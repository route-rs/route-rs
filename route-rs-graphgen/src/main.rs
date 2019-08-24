use std::fs::File;
use std::io::prelude::Write;
use std::io::BufReader;
use std::path::{Path, PathBuf};

extern crate clap;
use clap::{App, Arg};

extern crate xml;
use xml::reader::EventReader;

use crate::pipeline_graph::{EdgeData, NodeData, NodeKind, PipelineGraph, XmlNodeId};
use std::collections::HashMap;

mod codegen;
mod pipeline_graph;

enum Link {
    Input,
    Output(pipeline_graph::XmlNodeId),
    Sync(pipeline_graph::XmlNodeId, pipeline_graph::XmlNodeId),
}

fn gen_source_imports(modules: Vec<&str>) -> String {
    let local_imports = modules
        .iter()
        .map(|m| format!("crate::{}::*", m))
        .collect::<Vec<String>>();
    let external_imports = vec![
        "futures::lazy",
        "route_rs_runtime::element::*",
        "route_rs_runtime::link::*",
        "route_rs_runtime::pipeline::{InputChannelLink, OutputChannelLink}",
    ];
    [
        codegen::import(local_imports),
        codegen::import(external_imports),
    ]
    .join("\n")
}

fn get_io_nodes(nodes: &[&NodeData], edges: &[&EdgeData]) -> (NodeData, NodeData) {
    let io_nodes: Vec<&NodeData> = nodes
        .iter()
        .cloned()
        .filter(|n| n.node_kind == NodeKind::IO)
        .collect();
    let input_types: Vec<&NodeData> = io_nodes
        .iter()
        .cloned()
        .filter(|n| edges.iter().any(|e| e.source == n.xml_node_id))
        .collect();
    assert_eq!(input_types.len(), 1);
    let output_types: Vec<&NodeData> = io_nodes
        .iter()
        .cloned()
        .filter(|n| edges.iter().any(|e| e.target == n.xml_node_id))
        .collect();
    assert_eq!(output_types.len(), 1);
    (input_types[0].to_owned(), output_types[0].to_owned())
}

fn gen_element_decls(elements: &[&&NodeData]) -> (String, HashMap<String, String>) {
    let mut decl_idx: usize = 1;
    let mut element_decls_map = HashMap::new();
    let decls: Vec<String> = elements
        .iter()
        .map(|e| {
            let symbol = format!("elem_{}", decl_idx);
            decl_idx += 1;
            element_decls_map.insert(e.xml_node_id.to_owned(), symbol.clone());
            codegen::let_new(symbol, &e.node_class, Vec::<&str>::new())
        })
        .collect();
    (decls.join("\n"), element_decls_map)
}

fn gen_link_decls(
    links: &[(&XmlNodeId, Link)],
    element_decls: HashMap<String, String>,
) -> (String, HashMap<String, String>) {
    let mut decl_idx: usize = 1;
    let mut link_decls_map = HashMap::new();
    let decls: Vec<String> = links
        .iter()
        .map(|(id, el)| {
            let symbol = format!("link_{}", decl_idx);
            decl_idx += 1;
            link_decls_map.insert(id.to_owned().to_owned(), symbol.clone());
            let (struct_name, args) = match el {
                Link::Input => ("InputChannelLink", vec!["input_channel".to_string()]),
                Link::Output(feeder) => (
                    "OutputChannelLink",
                    vec![
                        format!("Box::new({})", link_decls_map.get(feeder.as_str()).unwrap()),
                        "output_channel".to_string(),
                    ],
                ),
                Link::Sync(feeder, element) => (
                    "SyncLink",
                    vec![
                        format!("Box::new({})", link_decls_map.get(feeder.as_str()).unwrap()),
                        element_decls.get(element.as_str()).unwrap().to_owned(),
                    ],
                ),
            };
            codegen::let_new(symbol, struct_name, args)
        })
        .collect();
    (decls.join("\n"), link_decls_map)
}

fn gen_tokio_run(drivers: Vec<String>, link_decls: HashMap<String, String>) -> String {
    let spawns: Vec<String> = drivers
        .iter()
        .map(|d| format!("tokio::spawn({});", link_decls.get(d).unwrap()))
        .collect();
    [
        String::from("tokio::run(lazy (|| {"),
        codegen::indent("    ", spawns.join("\n")),
        codegen::indent("    ", "Ok(())"),
        String::from("}));"),
    ]
    .join("\n")
}

fn gen_run_body(
    nodes: &[&NodeData],
    edges: &[&EdgeData],
    input_node: &NodeData,
    output_node: &NodeData,
) -> String {
    let mut elements = vec![];
    let mut links = vec![];
    let mut drivers = vec![];

    for nd in nodes {
        match &nd.node_kind {
            NodeKind::IO => {
                if nd.xml_node_id == input_node.xml_node_id {
                    links.push((&nd.xml_node_id, Link::Input));
                } else if nd.xml_node_id == output_node.xml_node_id {
                    let feeders: Vec<&&EdgeData> = edges
                        .iter()
                        .filter(|e| e.target == nd.xml_node_id)
                        .collect();
                    assert_eq!(feeders.len(), 1);
                    links.push((&nd.xml_node_id, Link::Output(feeders[0].source.to_owned())));
                    drivers.push(nd.xml_node_id.to_owned());
                } else {
                    panic!("{:?} is IO but not input_node or output_node", nd)
                }
            }
            NodeKind::Element => {
                let feeders: Vec<&&EdgeData> = edges
                    .iter()
                    .filter(|e| e.target == nd.xml_node_id)
                    .collect();
                assert_eq!(feeders.len(), 1);
                elements.push(nd);
                links.push((
                    &nd.xml_node_id,
                    Link::Sync(feeders[0].source.to_owned(), nd.xml_node_id.to_owned()),
                ));
            }
        }
    }

    let (element_decls_str, element_decls_map) = gen_element_decls(&elements);
    let (link_decls_str, link_decls_map) = gen_link_decls(&links, element_decls_map);
    [
        element_decls_str,
        link_decls_str,
        gen_tokio_run(drivers, link_decls_map),
    ]
    .join("\n\n")
}

fn gen_source_pipeline(nodes: Vec<&NodeData>, edges: Vec<&EdgeData>) -> String {
    let (input_node, output_node) = get_io_nodes(&nodes, &edges);
    [
        String::from("pub struct Pipeline {}"),
        codegen::impl_struct(
            "route_rs_runtime::pipeline::Runner",
            "Pipeline",
            [
                codegen::typedef(vec![
                    ("Input", &input_node.node_class),
                    ("Output", &output_node.node_class),
                ]),
                codegen::function(
                    "run",
                    vec![
                        ("input_channel", "crossbeam::Receiver<Self::Input>"),
                        ("output_channel", "crossbeam::Sender<Self::Output>"),
                    ],
                    "",
                    gen_run_body(&nodes, &edges, &input_node, &output_node),
                ),
            ]
            .join("\n\n"),
        ),
    ]
    .join("\n\n")
}

fn generate_pipeline_source(
    source_graph_path: PathBuf,
    modules: Vec<&str>,
    nodes: Vec<&NodeData>,
    edges: Vec<&EdgeData>,
) -> String {
    [
        codegen::comment(format!(
            "Generated by route-rs-graphgen\n\
             Source graph: {}",
            source_graph_path.as_path().display()
        )),
        gen_source_imports(modules),
        gen_source_pipeline(nodes, edges),
    ]
    .join("\n\n")
        + "\n"
}

fn main() {
    let app = App::new("route-rs graphgen")
        .version("0.1.0")
        .about("Generates route-rs pipeline from a graph")
        .arg(
            Arg::with_name("format")
                .short("f")
                .long("format")
                .value_name("FORMAT")
                .help("Specify input graph format")
                .takes_value(true)
                .possible_values(&["drawio"])
                .default_value("drawio"),
        )
        .arg(
            Arg::with_name("graph")
                .short("g")
                .long("graph")
                .value_name("GRAPH_FILE")
                .takes_value(true)
                .required(true)
                .validator(|g| {
                    if Path::new(&g).is_file() {
                        Ok(())
                    } else {
                        Err(format!("Path {} is not a regular file", g))
                    }
                }),
        )
        .arg(
            Arg::with_name("output")
                .short("o")
                .long("output")
                .value_name("OUTPUT_FILE")
                .takes_value(true)
                .required(true)
                .validator(|g| {
                    if Path::new(&g).parent().unwrap().is_dir() {
                        Ok(())
                    } else {
                        Err(format!("Path {} is not a regular file", g))
                    }
                }),
        )
        .arg(
            Arg::with_name("rustfmt")
                .long("rustfmt")
                .help("Run rustfmt on output file"),
        )
        .arg(
            Arg::with_name("modules")
                .short("m")
                .long("modules")
                .value_name("MODULES")
                .takes_value(true)
                .default_value("packets,elements"), // TODO: Validate that the modules exist in the target crate
        )
        .get_matches();

    let graph_file_path = Path::new(&app.value_of("graph").unwrap()).to_path_buf();
    let graph_file = File::open(&graph_file_path).unwrap();
    let graph_xml = EventReader::new(BufReader::new(graph_file));
    let graph = PipelineGraph::new(graph_xml);

    let modules: Vec<&str> = app.value_of("modules").unwrap().split(',').collect();

    let ordered_nodes = graph.ordered_nodes();
    let edges = graph.edges();

    let output_file_path = Path::new(&app.value_of("output").unwrap()).to_path_buf();
    let pipeline_source = generate_pipeline_source(graph_file_path, modules, ordered_nodes, edges);
    let mut output_file = File::create(&output_file_path).unwrap();
    output_file.write_all(pipeline_source.as_bytes()).unwrap();
    if app.is_present("rustfmt") {
        let rustfmt = std::process::Command::new("rustfmt")
            .args(&[output_file_path])
            .status();
        assert!(rustfmt.unwrap().success())
    }
}

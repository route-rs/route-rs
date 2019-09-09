use std::fs::File;
use std::io::prelude::Write;
use std::io::BufReader;
use std::path::{Path, PathBuf};

extern crate clap;
use clap::{App, Arg, ArgMatches};

extern crate xml;
use xml::reader::EventReader;

use crate::pipeline_graph::{EdgeData, NodeData, NodeKind, PipelineGraph, XmlNodeId};
use std::collections::HashMap;
use std::fmt::Debug;
use std::hash::Hash;

mod codegen;
mod pipeline_graph;

enum Link {
    Input,
    Output((XmlNodeId, Option<String>)),
    Sync((XmlNodeId, Option<String>), XmlNodeId),
    Classify((XmlNodeId, Option<String>), XmlNodeId, Vec<String>),
    Join(Vec<(XmlNodeId, Option<String>)>),
}

fn gen_source_imports(local_modules: Vec<&str>, runtime_modules: Vec<&str>) -> String {
    let local_imports = local_modules
        .iter()
        .map(|m| format!("crate::{}::*", m))
        .collect::<Vec<String>>();
    let runtime_imports = runtime_modules
        .iter()
        .map(|m| format!("route_rs_runtime::{}::*", m))
        .collect::<Vec<String>>();
    let external_imports = vec![
        "futures::lazy",
        "route_rs_runtime::pipeline::{InputChannelLink, OutputChannelLink}",
    ];
    [
        codegen::import(local_imports),
        codegen::import(runtime_imports),
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
            let symbol = format!("elem_{}_{}", decl_idx, e.node_class.to_lowercase());
            decl_idx += 1;
            element_decls_map.insert(e.xml_node_id.to_owned(), symbol.clone());
            codegen::let_new(symbol, &e.node_class, Vec::<&str>::new())
        })
        .collect();
    (decls.join("\n"), element_decls_map)
}

fn map_get_with_panic<'a, A, B>(map: &'a HashMap<A, B>, key: &A) -> &'a B
where
    A: Eq + Hash + Debug + 'a,
    B: Debug + 'a,
{
    match map.get(key) {
        Some(x) => x,
        None => panic!("get({:?}) failed on {:?}", key, map),
    }
}

fn unspool_channels(symbol: &str, channels: &mut Vec<String>, index: usize, kind: &str) -> String {
    let instance_symbol = format!("{}_{}_{}", &symbol, kind, index);
    channels.push(format!(
        "let {} = {}_{}s.next().unwrap();",
        &instance_symbol, &symbol, kind,
    ));
    instance_symbol
}

fn gen_link_decls(
    links: &[(XmlNodeId, Link)],
    element_decls: HashMap<String, String>,
) -> (String, Vec<String>) {
    let mut decl_idx: usize = 1;
    let mut link_decls_map = HashMap::new();
    let mut drivers: Vec<String> = vec![];
    let decls: Vec<String> = links
        .iter()
        .map(|(id, el)| {
            let symbol = format!("link_{}", decl_idx);
            decl_idx += 1;
            match el {
                Link::Input => {
                    link_decls_map.insert((id.to_owned(), None), symbol.clone());
                    codegen::let_new(
                        symbol,
                        "InputChannelLink",
                        vec!["input_channel".to_string()],
                    )
                }
                Link::Output(feeder) => {
                    link_decls_map.insert((id.to_owned(), None), symbol.clone());
                    drivers.push(symbol.clone());
                    codegen::let_new(
                        symbol,
                        "OutputChannelLink",
                        vec![
                            codegen::box_expr(map_get_with_panic(&link_decls_map, &feeder)),
                            "output_channel".to_string(),
                        ],
                    )
                }
                Link::Sync(feeder, element) => {
                    link_decls_map.insert((id.to_owned(), None), symbol.clone());
                    codegen::let_new(
                        symbol,
                        "SyncLink",
                        vec![
                            codegen::box_expr(map_get_with_panic(&link_decls_map, &feeder)),
                            element_decls.get(element.as_str()).unwrap().to_owned(),
                        ],
                    )
                }
                Link::Classify(feeder, element, branches) => {
                    let mut match_branches = vec![];
                    let mut egressors = vec![];
                    for branch_index in 0..(branches.len()) {
                        match_branches.push((
                            branches.get(branch_index).unwrap(),
                            branch_index.to_string(),
                        ));
                        let egressor_symbol =
                            unspool_channels(&symbol, &mut egressors, branch_index, "egressor");
                        link_decls_map.insert(
                            (
                                id.to_owned(),
                                Some(branches.get(branch_index).unwrap().to_owned()),
                            ),
                            egressor_symbol.clone(),
                        );
                    }
                    drivers.push(format!("{}_ingressor", &symbol));
                    let mut classify_decls = vec![
                        codegen::let_new(
                            symbol.clone(),
                            "ClassifyLink",
                            vec![
                                codegen::box_expr(map_get_with_panic(&link_decls_map, &feeder)),
                                element_decls.get(element.as_str()).unwrap().to_owned(),
                                codegen::box_expr(format!(
                                    "|c| {}",
                                    codegen::match_expr("c", match_branches)
                                )),
                                String::from("10"),
                                branches.len().to_string(),
                            ],
                        ),
                        format!("let {}_ingressor = {}.ingressor;", &symbol, &symbol,),
                        format!(
                            "let mut {}_egressors = {}.egressors.into_iter();",
                            &symbol, &symbol,
                        ),
                    ];
                    classify_decls.append(&mut egressors);
                    classify_decls.join("\n")
                }
                Link::Join(feeders) => {
                    let egressor_symbol = format!("{}_egressor", &symbol);
                    link_decls_map.insert((id.to_owned(), None), egressor_symbol.clone());
                    let mut feeders_decls = vec![];
                    let mut ingressors = vec![];
                    for feeder_index in 0..(feeders.len()) {
                        feeders_decls.push(codegen::box_expr(map_get_with_panic(
                            &link_decls_map,
                            &feeders.get(feeder_index).unwrap(),
                        )));
                        let ingressor_symbol =
                            unspool_channels(&symbol, &mut ingressors, feeder_index, "ingressor");
                        drivers.push(ingressor_symbol.clone());
                    }
                    let mut join_decls = vec![
                        codegen::let_new(
                            symbol.clone(),
                            "JoinLink",
                            vec![
                                format!("vec![{}]", feeders_decls.join(", ")),
                                String::from("10"),
                            ],
                        ),
                        format!("let {}_egressor = {}.egressor;", &symbol, &symbol,),
                        format!(
                            "let mut {}_ingressors = {}.ingressors.into_iter();",
                            &symbol, &symbol,
                        ),
                    ];
                    join_decls.append(&mut ingressors);
                    join_decls.join("\n")
                }
            }
        })
        .collect();
    (decls.join("\n\n"), drivers)
}

fn gen_tokio_run(drivers: Vec<String>) -> String {
    let spawns: Vec<String> = drivers
        .iter()
        .map(|d| format!("tokio::spawn({});", d))
        .collect();
    [
        String::from("tokio::run(lazy (move || {"),
        codegen::indent("    ", spawns.join("\n")),
        codegen::indent("    ", "Ok(())"),
        String::from("}));"),
    ]
    .join("\n")
}

fn expand_join_link<'a>(
    feeders: &[&&EdgeData],
    links: &mut Vec<(String, Link)>,
    orig_xml_node_id: &str,
    link_builder: Box<dyn Fn(XmlNodeId, Option<String>) -> Link + 'a>,
) {
    if feeders.len() == 1 {
        links.push((
            orig_xml_node_id.to_owned(),
            link_builder(feeders[0].source.to_owned(), feeders[0].label.to_owned()),
        ))
    } else {
        let join_xml_node_id = ["join", &orig_xml_node_id].join("_");
        let join_feeders = feeders
            .iter()
            .map(|f| (f.source.to_owned(), f.label.to_owned()))
            .collect::<Vec<(XmlNodeId, Option<String>)>>();
        links.push((join_xml_node_id.to_owned(), Link::Join(join_feeders)));
        links.push((
            orig_xml_node_id.to_owned(),
            link_builder(join_xml_node_id, None),
        ));
    }
}

fn gen_run_body(
    nodes: &[&NodeData],
    edges: &[&EdgeData],
    input_node: &NodeData,
    output_node: &NodeData,
) -> String {
    let mut elements = vec![];
    let mut links = vec![];

    for nd in nodes {
        let feeders: Vec<&&EdgeData> = edges
            .iter()
            .filter(|e| e.target == nd.xml_node_id)
            .collect();
        match &nd.node_kind {
            NodeKind::IO => {
                if nd.xml_node_id == input_node.xml_node_id {
                    links.push((nd.xml_node_id.to_owned(), Link::Input));
                } else if nd.xml_node_id == output_node.xml_node_id {
                    expand_join_link(
                        &feeders,
                        &mut links,
                        &nd.xml_node_id,
                        Box::new(|xni, label| Link::Output((xni, label))),
                    );
                } else {
                    panic!("{:?} is IO but not input_node or output_node", nd)
                }
            }
            NodeKind::Element => {
                elements.push(nd);
                expand_join_link(
                    &feeders,
                    &mut links,
                    &nd.xml_node_id,
                    Box::new(|xni, label| Link::Sync((xni, label), nd.xml_node_id.to_owned())),
                );
            }
            NodeKind::Classifier => {
                let outlets: Vec<String> = edges
                    .iter()
                    .filter(|e| e.source == nd.xml_node_id)
                    .map(|e| e.label.clone().unwrap())
                    .collect();
                elements.push(nd);
                expand_join_link(
                    &feeders,
                    &mut links,
                    &nd.xml_node_id,
                    Box::new(|xni, label| {
                        Link::Classify((xni, label), nd.xml_node_id.to_owned(), outlets.to_owned())
                    }),
                );
            }
        }
    }

    let (element_decls_str, element_decls_map) = gen_element_decls(&elements);
    let (link_decls_str, drivers) = gen_link_decls(&links, element_decls_map);
    [element_decls_str, link_decls_str, gen_tokio_run(drivers)].join("\n\n")
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
    local_modules: Vec<&str>,
    runtime_modules: Vec<&str>,
    nodes: Vec<&NodeData>,
    edges: Vec<&EdgeData>,
) -> String {
    [
        codegen::comment(format!(
            "Generated by route-rs-graphgen\n\
             Source graph: {}",
            source_graph_path.as_path().display()
        )),
        gen_source_imports(local_modules, runtime_modules),
        gen_source_pipeline(nodes, edges),
    ]
    .join("\n\n")
        + "\n"
}

fn get_array_arg<'a>(arg_matches: &'a ArgMatches, name: &str) -> Vec<&'a str> {
    arg_matches.value_of(name).unwrap().split(',').collect()
}

fn get_pathbuf_arg(arg_matches: &ArgMatches, name: &str) -> PathBuf {
    Path::new(arg_matches.value_of(name).unwrap()).to_path_buf()
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
            Arg::with_name("local-modules")
                .short("m")
                .long("local-modules")
                .value_name("LOCAL_MODULES")
                .takes_value(true)
                .default_value("packets,elements"), // TODO: Validate that the modules exist in the target crate
        )
        .arg(
            Arg::with_name("runtime-modules")
                .short("r")
                .long("runtime-modules")
                .value_name("RUNTIME_MODULES")
                .takes_value(true)
                .default_value("element,link"), // TODO: Validate that the modules exist in our crate
        )
        .get_matches();

    let graph_file_path = get_pathbuf_arg(&app, "graph");
    let graph_file = File::open(&graph_file_path).unwrap();
    let graph_xml = EventReader::new(BufReader::new(graph_file));
    let graph = PipelineGraph::new(graph_xml);

    let local_modules: Vec<&str> = get_array_arg(&app, "local-modules");
    let runtime_modules: Vec<&str> = get_array_arg(&app, "runtime-modules");

    let ordered_nodes = graph.ordered_nodes();
    let edges = graph.edges();

    let output_file_path = get_pathbuf_arg(&app, "output");
    let pipeline_source = generate_pipeline_source(
        graph_file_path,
        local_modules,
        runtime_modules,
        ordered_nodes,
        edges,
    );
    let mut output_file = File::create(&output_file_path).unwrap();
    output_file.write_all(pipeline_source.as_bytes()).unwrap();
    if app.is_present("rustfmt") {
        let rustfmt = std::process::Command::new("rustfmt")
            .args(&[output_file_path])
            .status();
        assert!(rustfmt.unwrap().success())
    }
}

use std::fs::File;
use std::io::prelude::Write;
use std::io::BufReader;
use std::path::{Path, PathBuf};

extern crate clap;
use clap::{App, Arg, ArgMatches};

extern crate xml;
use xml::reader::EventReader;

use crate::codegen::magic_newline_stmt;
use crate::pipeline_graph::{EdgeData, NodeData, NodeKind, PipelineGraph, XmlNodeId};
use std::collections::HashMap;
use std::fmt::Debug;
use std::hash::Hash;
use std::iter::FromIterator;
use syn::export::ToTokens;

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
    let mut imports = vec![];
    for lm in local_modules {
        imports.push(syn::UseTree::Path(codegen::use_path(
            "crate",
            syn::UseTree::Path(codegen::use_path(
                lm,
                syn::UseTree::Glob(codegen::use_glob()),
            )),
        )))
    }
    imports.push(syn::UseTree::Path(codegen::use_path(
        "route_rs_runtime",
        syn::UseTree::Path(codegen::use_path(
            "link",
            syn::UseTree::Glob(codegen::use_glob()),
        )),
    )));
    imports.push(syn::UseTree::Path(codegen::use_path(
        "route_rs_runtime",
        syn::UseTree::Path(codegen::use_path(
            "link",
            syn::UseTree::Path(codegen::use_path(
                "primitive",
                syn::UseTree::Glob(codegen::use_glob()),
            )),
        )),
    )));
    for rm in runtime_modules {
        imports.push(syn::UseTree::Path(codegen::use_path(
            "route_rs_runtime",
            syn::UseTree::Path(codegen::use_path(
                rm,
                syn::UseTree::Glob(codegen::use_glob()),
            )),
        )))
    }
    imports.push(syn::UseTree::Path(codegen::use_path(
        "tokio",
        syn::UseTree::Name(syn::UseName {
            ident: codegen::ident("runtime"),
        }),
    )));
    imports.push(syn::UseTree::Path(codegen::use_path(
        "tokio",
        syn::UseTree::Path(codegen::use_path(
            "task",
            syn::UseTree::Name(syn::UseName {
                ident: codegen::ident("JoinHandle"),
            }),
        )),
    )));

    codegen::import(&imports)
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

fn gen_processor_decls(processors: &[&&NodeData]) -> (Vec<syn::Stmt>, HashMap<String, String>) {
    let mut decl_idx: usize = 1;
    let mut processor_decls_map = HashMap::new();
    let decls: Vec<syn::Stmt> = processors
        .iter()
        .map(|e| {
            let symbol = format!("elem_{}_{}", decl_idx, e.node_class.to_lowercase());
            decl_idx += 1;
            processor_decls_map.insert(e.xml_node_id.to_owned(), symbol.clone());
            syn::Stmt::Local(codegen::let_simple(
                codegen::ident(symbol.as_str()),
                None,
                syn::Expr::Call(syn::ExprCall {
                    attrs: vec![],
                    func: Box::new(syn::Expr::Path(syn::ExprPath {
                        attrs: vec![],
                        qself: None,
                        path: codegen::simple_path(
                            vec![codegen::ident(&e.node_class), codegen::ident("new")],
                            false,
                        ),
                    })),
                    paren_token: syn::token::Paren {
                        span: proc_macro2::Span::call_site(),
                    },
                    args: syn::punctuated::Punctuated::new(),
                }),
                false,
            ))
        })
        .collect();
    (decls, processor_decls_map)
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

fn gen_link_decls(
    links: &[(XmlNodeId, Link)],
    processor_decls: HashMap<String, String>,
) -> Vec<syn::Stmt> {
    let mut decl_idx: usize = 0;
    let mut link_decls_map = HashMap::new();
    let decls: Vec<Vec<syn::Stmt>> = links
        .iter()
        .map(|(id, el)| {
            decl_idx += 1;
            match el {
                Link::Input => {
                    link_decls_map.insert((id.to_owned(), None), format!("link_{}_egress_{}", decl_idx, 0));
                    codegen::build_link(
                        decl_idx,
                        "InputChannelLink",
                        vec![
                            (codegen::ident("channel"), vec![codegen::expr_path_ident("input_channel")]),
                        ],
                        1
                    )
                }
                Link::Output(feeder) => {
                    codegen::build_link(
                        decl_idx,
                        "OutputChannelLink",
                        vec![
                            (codegen::ident("ingressor"), vec![codegen::expr_path_ident(map_get_with_panic(&link_decls_map, &feeder).as_str())]),
                            (codegen::ident("channel"), vec![codegen::expr_path_ident("output_channel")]),
                        ],
                        0
                    )
                }
                Link::Sync(feeder, processor) => {
                    link_decls_map.insert((id.to_owned(), None), format!("link_{}_egress_{}", decl_idx, 0));
                    codegen::build_link(
                        decl_idx,
                        "ProcessLink",
                        vec![
                            (codegen::ident("ingressor"), vec![codegen::expr_path_ident(map_get_with_panic(&link_decls_map, &feeder).as_str())]),
                            (codegen::ident("processor"), vec![codegen::expr_path_ident(processor_decls.get(processor.as_str()).unwrap())])
                        ],
                        1
                    )
                }
                Link::Classify(feeder, processor, branches) => {
                    let mut match_branches = vec![];
                    for branch_index in 0..(branches.len()) {
                        match_branches.push((
                            branches.get(branch_index).unwrap(),
                            branch_index.to_string(),
                        ));
                        link_decls_map.insert(
                            (
                                id.to_owned(),
                                Some(branches.get(branch_index).unwrap().to_owned()),
                            ),
                            format!("link_{}_egress_{}", decl_idx, branch_index),
                        );
                    }
                    codegen::build_link(
                        decl_idx,
                        "ClassifyLink",
                        vec![
                            (codegen::ident("ingressor"), vec![codegen::expr_path_ident(map_get_with_panic(&link_decls_map, &feeder).as_str())]),
                            (codegen::ident("classifier"), vec![codegen::expr_path_ident(processor_decls.get(processor.as_str()).unwrap())]),
                            (codegen::ident("dispatcher"), vec![syn::Expr::Call(syn::ExprCall {
                                attrs: vec![],
                                func: Box::new(syn::Expr::Path(syn::ExprPath {
                                    attrs: vec![],
                                    qself: None,
                                    path: codegen::simple_path(
                                        vec![codegen::ident("Box"), codegen::ident("new")],
                                        false,
                                    ),
                                })),
                                paren_token: syn::token::Paren {
                                    span: proc_macro2::Span::call_site(),
                                },
                                args: syn::punctuated::Punctuated::from_iter(vec![codegen::closure(
                                    false,
                                    false,
                                    false,
                                    vec![syn::Pat::Ident(syn::PatIdent {
                                        attrs: vec![],
                                        by_ref: None,
                                        mutability: None,
                                        ident: codegen::ident("c"),
                                        subpat: None
                                    })],
                                    syn::ReturnType::Default,
                                    vec![syn::Stmt::Expr(syn::Expr::Match(syn::ExprMatch {
                                        attrs: vec![],
                                        match_token: syn::token::Match { span: proc_macro2::Span::call_site() },
                                        expr: Box::new(codegen::expr_path_ident("c")),
                                        brace_token: syn::token::Brace { span: proc_macro2::Span::call_site() },
                                        arms: match_branches.into_iter().map(|(p, b)| syn::Arm {
                                            attrs: vec![],
                                            pat: syn::parse_str::<syn::Pat>(p).unwrap(),
                                            guard: None,
                                            fat_arrow_token: syn::token::FatArrow { spans: [proc_macro2::Span::call_site(), proc_macro2::Span::call_site()] },
                                            body: Box::new(syn::Expr::Lit(syn::ExprLit { attrs: vec![], lit: syn::Lit::Int(syn::LitInt::new(b.as_str(), proc_macro2::Span::call_site())) })),
                                            comma: Some(syn::token::Comma { spans: [proc_macro2::Span::call_site()] })
                                        }).collect::<Vec<syn::Arm>>()
                                    }))]
                                )].into_iter()),
                            })]),
                            (codegen::ident("num_egressors"), vec![syn::Expr::Lit(syn::ExprLit { attrs: vec![], lit: syn::Lit::Int(syn::LitInt::new(branches.len().to_string().as_str(), proc_macro2::Span::call_site())) })]),
                        ],
                        branches.len()
                    )
                }
                Link::Join(feeders) => {
                    let egressor_symbol = format!("link_{}_egress_{}", decl_idx, 0);
                    link_decls_map.insert((id.to_owned(), None), egressor_symbol);
                    let mut feeders_decls = vec![];
                    for feeder_index in 0..(feeders.len()) {
                        feeders_decls.push(map_get_with_panic(
                            &link_decls_map,
                            &feeders.get(feeder_index).unwrap(),
                        ));
                    }
                    codegen::build_link(
                        decl_idx,
                        "JoinLink",
                        vec![
                            (codegen::ident("ingressors"), vec![codegen::vec(feeders_decls.into_iter().map(|d| codegen::expr_path_ident(d)).collect::<Vec<syn::Expr>>())])
                        ],
                        1
                    )
                }
            }
        })
        .collect();
    decls
        .into_iter()
        .map(|mut ss| {
            // Add magic newlines between each link section. These will be replaced with real newlines
            // right before we write out the source, since syn doesn't have a way to generate newlines.
            ss.push(magic_newline_stmt());
            ss
        })
        .flatten()
        .collect()
}

fn gen_tokio_run() -> Vec<syn::Stmt> {
    vec![
        syn::Stmt::Local(codegen::let_simple(
            codegen::ident("rt"),
            None,
            codegen::call_chain(
                codegen::call_function(
                    syn::Expr::Path(syn::ExprPath {
                        attrs: vec![],
                        qself: None,
                        path: codegen::path(vec![
                            (codegen::ident("runtime"), None),
                            (codegen::ident("Builder"), None),
                            (codegen::ident("new"), None),
                        ]),
                    }),
                    vec![],
                ),
                vec![
                    ("threaded_scheduler", vec![]),
                    ("enable_all", vec![]),
                    ("build", vec![]),
                    ("unwrap", vec![]),
                ],
            ),
            true,
        )),
        syn::Stmt::Semi(
            codegen::call_function(
                syn::Expr::Path(syn::ExprPath {
                    attrs: vec![],
                    qself: None,
                    path: codegen::simple_path(
                        vec![codegen::ident("tokio"), codegen::ident("run")],
                        false,
                    ),
                }),
                vec![codegen::call_function(
                    codegen::expr_path_ident("lazy"),
                    vec![codegen::closure(
                        false,
                        false,
                        true,
                        vec![],
                        syn::ReturnType::Default,
                        vec![
                            codegen::for_loop(
                                syn::Pat::Ident(syn::PatIdent {
                                    attrs: vec![],
                                    by_ref: None,
                                    mutability: None,
                                    ident: codegen::ident("r"),
                                    subpat: None,
                                }),
                                codegen::expr_path_ident("all_runnables"),
                                vec![syn::Stmt::Semi(
                                    codegen::call_function(
                                        syn::Expr::Path(syn::ExprPath {
                                            attrs: vec![],
                                            qself: None,
                                            path: codegen::simple_path(
                                                vec![
                                                    codegen::ident("tokio"),
                                                    codegen::ident("spawn"),
                                                ],
                                                false,
                                            ),
                                        }),
                                        vec![codegen::expr_path_ident("r")],
                                    ),
                                    syn::token::Semi {
                                        spans: [proc_macro2::Span::call_site()],
                                    },
                                )],
                            ),
                            syn::Stmt::Expr(codegen::call_function(
                                codegen::expr_path_ident("Ok"),
                                vec![syn::Expr::Tuple(syn::ExprTuple {
                                    attrs: vec![],
                                    paren_token: syn::token::Paren {
                                        span: proc_macro2::Span::call_site(),
                                    },
                                    elems: syn::punctuated::Punctuated::new(),
                                })],
                            )),
                        ],
                    )],
                )],
            ),
            syn::token::Semi {
                spans: [proc_macro2::Span::call_site()],
            },
        ),
    ]
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
) -> Vec<syn::Stmt> {
    let mut processors = vec![];
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
            NodeKind::Processor => {
                processors.push(nd);
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
                processors.push(nd);
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

    let all_runnables_stmt = syn::Stmt::Local(codegen::let_simple(
        codegen::ident("all_runnables"),
        Some(syn::Type::Path(syn::TypePath {
            qself: None,
            path: syn::Path {
                leading_colon: None,
                segments: syn::punctuated::Punctuated::from_iter(vec![syn::PathSegment {
                    ident: codegen::ident("Vec"),
                    arguments: syn::PathArguments::AngleBracketed(codegen::angle_bracketed_types(
                        vec![syn::Type::Path(syn::TypePath {
                            qself: None,
                            path: codegen::simple_path(
                                vec![codegen::ident("TokioRunnable")],
                                false,
                            ),
                        })],
                    )),
                }]),
            },
        })),
        syn::Expr::Macro(syn::ExprMacro {
            attrs: vec![],
            mac: syn::Macro {
                path: codegen::simple_path(vec![codegen::ident("vec")], false),
                bang_token: syn::token::Bang {
                    spans: [proc_macro2::Span::call_site()],
                },
                delimiter: syn::MacroDelimiter::Bracket(syn::token::Bracket {
                    span: proc_macro2::Span::call_site(),
                }),
                tokens: proc_macro2::TokenStream::new(),
            },
        }),
        true,
    ));
    let (mut processor_decls_stmts, processor_decls_map) = gen_processor_decls(&processors);
    processor_decls_stmts.push(magic_newline_stmt());
    let mut stmts = vec![];
    stmts.push(all_runnables_stmt);
    stmts.push(magic_newline_stmt());
    stmts.append(&mut processor_decls_stmts);
    stmts.append(&mut gen_link_decls(&links, processor_decls_map));
    stmts.append(&mut gen_tokio_run());
    stmts
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
                    (
                        codegen::ident("Input"),
                        syn::parse_str::<syn::Type>(&input_node.node_class).unwrap(),
                    ),
                    (
                        codegen::ident("Output"),
                        syn::parse_str::<syn::Type>(&output_node.node_class).unwrap(),
                    ),
                ]),
                codegen::function_def(
                    codegen::ident("run"),
                    vec![
                        (
                            "input_channel",
                            syn::Type::Path(syn::TypePath {
                                qself: None,
                                path: codegen::path(vec![
                                    (codegen::ident("crossbeam"), None),
                                    (
                                        codegen::ident("Receiver"),
                                        Some(vec![syn::GenericArgument::Type(syn::Type::Path(
                                            syn::TypePath {
                                                qself: None,
                                                path: codegen::path(vec![
                                                    (codegen::ident("Self"), None),
                                                    (codegen::ident("Input"), None),
                                                ]),
                                            },
                                        ))]),
                                    ),
                                ]),
                            }),
                        ),
                        (
                            "output_channel",
                            syn::Type::Path(syn::TypePath {
                                qself: None,
                                path: codegen::path(vec![
                                    (codegen::ident("crossbeam"), None),
                                    (
                                        codegen::ident("Sender"),
                                        Some(vec![syn::GenericArgument::Type(syn::Type::Path(
                                            syn::TypePath {
                                                qself: None,
                                                path: codegen::path(vec![
                                                    (codegen::ident("Self"), None),
                                                    (codegen::ident("Output"), None),
                                                ]),
                                            },
                                        ))]),
                                    ),
                                ]),
                            }),
                        ),
                    ],
                    gen_run_body(&nodes, &edges, &input_node, &output_node),
                    syn::ReturnType::Default,
                )
                .to_token_stream()
                .to_string(),
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
    let args: Vec<&str> = arg_matches.value_of(name).unwrap().split(',').collect();
    if args.eq(&[""]) {
        vec![]
    } else {
        args
    }
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
                .default_value("packets,processors"), // TODO: Validate that the modules exist in the target crate
        )
        .arg(
            Arg::with_name("runtime-modules")
                .short("r")
                .long("runtime-modules")
                .value_name("RUNTIME_MODULES")
                .takes_value(true)
                .default_value(""), // TODO: Validate that the modules exist in our crate
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
    output_file
        .write_all(codegen::unmagic_newlines(pipeline_source).as_bytes())
        .unwrap();
    if app.is_present("rustfmt") {
        let rustfmt = std::process::Command::new("rustfmt")
            .args(&[output_file_path])
            .status();
        assert!(rustfmt.unwrap().success())
    }
}

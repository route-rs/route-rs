use std::collections::HashMap;
use std::path::PathBuf;

use crate::ast;
use crate::fs::{GRAPH_SUFFIX, RUST_SUFFIX};
use crate::graph;
use std::io::{ErrorKind, Write};
use syn::export::ToTokens;
use std::convert::TryFrom;

pub(crate) enum GenerateError {
    IO(std::io::Error),
}

fn report_result(
    results: &mut HashMap<PathBuf, Result<PathBuf, GenerateError>>,
    graph: PathBuf,
    result: Result<PathBuf, GenerateError>,
) {
    let message: String = match &result {
        Ok(path) => path.to_string_lossy().to_string(),
        Err(GenerateError::IO(err)) => format!("IO Error: {}", err),
    };

    println!("  {} => {}", graph.to_string_lossy(), message);
    results.insert(graph, result);
}

pub(crate) fn generate_files(
    src_dir: &PathBuf,
    dst_dir: &PathBuf,
    graph_files: Vec<PathBuf>,
) -> HashMap<PathBuf, Result<PathBuf, GenerateError>> {
    let mut results = HashMap::new();

    println!("Generating links:");
    for graph in graph_files {
        let src = src_dir.join(&graph);
        let parent = graph.parent().unwrap().to_path_buf();
        let file_string = graph.file_name().unwrap().to_str().unwrap();
        let dst = dst_dir.join(parent).join(PathBuf::from(
            file_string.replace(GRAPH_SUFFIX, RUST_SUFFIX),
        ));

        match std::fs::create_dir_all(dst.parent().unwrap()) {
            Ok(()) => match std::fs::File::create(&dst) {
                Ok(mut file) => {
                    let ast = generate_ast(&src);
                    let raw_source: String = generate_source(ast);
                    match file.write_all(raw_source.as_bytes()) {
                        Ok(()) => {
                            // Need to close the file before running rustfmt
                            drop(file);
                            // Syn generates source as one big single-line blob, so we want to
                            // format it with rustfmt for niceness.
                            rustfmt(&mut results, src, dst)
                        }
                        Err(e) => report_result(&mut results, src, Err(GenerateError::IO(e))),
                    }
                }
                Err(e) => report_result(&mut results, src, Err(GenerateError::IO(e))),
            },
            Err(e) => report_result(&mut results, src, Err(GenerateError::IO(e))),
        }
    }

    results
}

fn generate_ast(xml_file: &PathBuf) -> syn::File {
    let gr = graph::Graph::try_from(xml_file).unwrap();
    syn::File {
        shebang: None,
        attrs: vec![],
        items: vec![
            ast::magic_comment("Generated by route-rs-graphgen"),
            ast::magic_comment(format!("Source file: {}", xml_file.to_string_lossy()).as_str()),
            ast::def_struct(ast::vis_pub_crate(), gr.name.as_str(), vec![])
        ],
    }
}

fn generate_source(ast: syn::File) -> String {
    let raw = ast.to_token_stream().to_string();
    let fixed_comments = ast::unmagic_comments(&raw);

    fixed_comments
}

fn rustfmt(
    results: &mut HashMap<PathBuf, Result<PathBuf, GenerateError>>,
    src: PathBuf,
    dst: PathBuf,
) {
    let result = std::process::Command::new("rustfmt")
        .args(&[&dst])
        .args(&["--edition", "2018"])
        .status();

    match result {
        Ok(status) => {
            if status.success() {
                report_result(results, src, Ok(dst))
            } else {
                report_result(
                    results,
                    src,
                    Err(GenerateError::IO(std::io::Error::new(
                        ErrorKind::Other,
                        format!("rustfmt exited with code {}", status.code().unwrap()),
                    ))),
                )
            }
        }
        Err(e) => report_result(results, src, Err(GenerateError::IO(e))),
    }
}

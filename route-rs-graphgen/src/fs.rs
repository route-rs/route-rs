use std::collections::HashMap;
use std::path::PathBuf;

pub(crate) const GRAPH_SUFFIX: &str = ".graphgen.xml";
pub(crate) const RUST_SUFFIX: &str = ".rs";

pub(crate) fn find_files_with_suffix(dir: &PathBuf, suffix: &str) -> Vec<PathBuf> {
    let mut files_with_suffix = vec![];
    for entry in dir.read_dir().unwrap() {
        let path = entry.unwrap().path();
        if path.is_dir() {
            files_with_suffix.append(&mut find_files_with_suffix(&path, suffix))
        } else if path.is_file() {
            if let Some(filename) = path.to_str() {
                if filename.ends_with(suffix) {
                    files_with_suffix.push(path);
                }
            }
        }
    }
    files_with_suffix
}

pub(crate) fn generate_files(
    src_dir: &PathBuf,
    dst_dir: &PathBuf,
    graph_files: Vec<PathBuf>,
) -> HashMap<PathBuf, PathBuf> {
    let mut mapping = HashMap::new();

    for graph in graph_files {
        let src = src_dir.join(&graph);
        let parent = graph.parent().unwrap().to_path_buf();
        let file_string = graph.file_name().unwrap().to_str().unwrap();
        let dst = dst_dir.join(parent).join(PathBuf::from(
            file_string.replace(GRAPH_SUFFIX, RUST_SUFFIX),
        ));

        match std::fs::create_dir_all(dst.parent().unwrap()) {
            Ok(()) => match std::fs::File::create(&dst) {
                Ok(file) => {
                    mapping.insert(src, dst);
                }
                Err(e) => panic!(e),
            },
            Err(e) => panic!(e),
        }
    }

    mapping
}

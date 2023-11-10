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

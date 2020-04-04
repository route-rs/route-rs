use std::path::{Path, PathBuf};
use std::process::Command;

pub enum GraphgenVersion {
    V1,
    V2,
}

impl std::fmt::Display for GraphgenVersion {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", match self {
            GraphgenVersion::V1 => "v1",
            GraphgenVersion::V2 => "v2",
        })
    }
}

pub struct TestHelper {
    pub root: PathBuf,
    pub tmpdir: PathBuf,
    pub example_crate: String,
    pub test_name: String,
    pub extra_args: Vec<String>,
    pub version: GraphgenVersion,
}

impl TestHelper {
    pub fn new<S, T>(version: GraphgenVersion, example_crate: S, extra_args: Vec<T>) -> Self
    where
        S: Into<String>,
        T: Into<String>,
    {
        let root = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join(Path::new(".."))
            .canonicalize()
            .unwrap();
        let global_tmpdir = Path::new(&std::env::temp_dir()).canonicalize().unwrap();
        let example_crate_string = example_crate.into();
        let mut test_name = example_crate_string.clone();
        test_name.retain(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_');
        test_name = test_name.replace('-', "_");

        TestHelper {
            root,
            tmpdir: global_tmpdir.join("integration-test-route-rs-graphgen"),
            example_crate: example_crate_string,
            test_name,
            extra_args: extra_args
                .into_iter()
                .map(|s| s.into())
                .collect::<Vec<String>>(),
            version,
        }
        .initialize()
    }

    pub fn initialize(self) -> Self {
        self.setup_tmpdir();

        assert!(self.graphgen_binary().is_file());
        assert!(self.test_tmpdir().is_dir());
        assert!(self.crate_dir().is_dir());

        self
    }

    fn setup_tmpdir(&self) {
        if !self.tmpdir.exists() {
            std::fs::create_dir(&self.tmpdir).unwrap();
        }
        if self.test_tmpdir().exists() {
            std::fs::remove_dir_all(self.test_tmpdir()).unwrap();
        }
        std::fs::create_dir(self.test_tmpdir()).unwrap();
    }

    pub fn graphgen_binary(&self) -> PathBuf {
        self.root.join(Path::new("target/debug/route-rs-graphgen"))
    }

    pub fn test_tmpdir(&self) -> PathBuf {
        self.tmpdir.join(&self.test_name)
    }

    pub fn output_file(&self) -> PathBuf {
        self.test_tmpdir().join("pipeline.rs")
    }

    pub fn crate_dir(&self) -> PathBuf {
        self.root.join("examples").join(&self.example_crate)
    }

    pub fn graph_file(&self) -> PathBuf {
        self.crate_dir().join("src/pipeline.xml")
    }

    pub fn pipeline_file(&self) -> PathBuf {
        self.crate_dir().join("src/pipeline.rs")
    }

    pub fn run_graphgen(&self) {
        let graphgen_cmd = Command::new(self.graphgen_binary())
            .args(&[self.version.to_string()])
            .args(&["--graph", self.graph_file().to_str().unwrap()])
            .args(&["--output", self.output_file().to_str().unwrap()])
            .args(&self.extra_args)
            .output()
            .expect("Failed to execute graphgen");
        assert!(
            graphgen_cmd.status.success(),
            "Output:\n{}\n\nError:\n{}",
            String::from_utf8(graphgen_cmd.stdout).unwrap(),
            String::from_utf8(graphgen_cmd.stderr).unwrap(),
        );
    }

    pub fn run_diff(&self) {
        let diff_cmd = Command::new("diff")
            .arg("-u")
            // Excluded because we're building to a different location so relative paths will differ
            .args(&["-I", "// Source graph:"])
            .arg(self.pipeline_file())
            .arg(self.output_file())
            .output()
            .expect("Failed to execute diff");
        assert!(
            diff_cmd.status.success(),
            "Found differences:\n\n{}",
            String::from_utf8(diff_cmd.stdout).unwrap()
        );
    }
}

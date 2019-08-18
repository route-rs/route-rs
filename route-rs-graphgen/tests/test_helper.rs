use std::path::{Path, PathBuf};

pub struct TestHelper {
    pub root: PathBuf,
    pub tmpdir: PathBuf,
    pub example_crate: String,
    pub test_name: String,
}

impl TestHelper {
    pub fn new<S>(example_crate: S) -> Self
    where
        S: Into<String>,
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

    pub fn crate_dir(&self) -> PathBuf {
        self.root.join("examples").join(&self.example_crate)
    }
}

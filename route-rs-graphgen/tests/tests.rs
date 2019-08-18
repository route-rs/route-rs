use std::process::Command;

mod test_helper;

#[test]
fn trivial_identity() {
    let test_helper = test_helper::TestHelper::new("trivial-identity");
    let graph_file = test_helper.crate_dir().join("src/pipeline.xml");
    let pipeline_file = test_helper.crate_dir().join("src/pipeline.rs");
    let output_file = test_helper.test_tmpdir().join("pipeline.rs");
    let extra_args = vec!["--rustfmt", "--modules", "packets"];

    let graphgen_cmd = Command::new(test_helper.graphgen_binary())
        .args(&["--graph", graph_file.to_str().unwrap()])
        .args(&["--output", output_file.to_str().unwrap()])
        .args(extra_args)
        .output()
        .expect("Failed to execute graphgen");
    assert!(
        graphgen_cmd.status.success(),
        "Output:\n\n{}",
        String::from_utf8(graphgen_cmd.stdout).unwrap()
    );

    let diff_cmd = Command::new("diff")
        .arg("-u")
        .args(&["-I", "// Source graph:"]) // Exclude this line because we're building to a different location so the relative paths will be different
        .arg(pipeline_file)
        .arg(output_file)
        .output()
        .expect("Failed to execute diff");
    assert!(
        diff_cmd.status.success(),
        "Found differences:\n\n{}",
        String::from_utf8(diff_cmd.stdout).unwrap()
    );
}

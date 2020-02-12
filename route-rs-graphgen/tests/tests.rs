#[allow(dead_code)]
mod test_helper;

#[test]
#[ignore]
fn trivial_identity() {
    let test_helper = test_helper::TestHelper::new(
        "trivial-identity",
        vec![
            "--rustfmt",
            "--local-modules",
            "packets",
            "--runtime-modules",
            "processor",
        ],
    );

    test_helper.run_graphgen();
    test_helper.run_diff();
}

#[test]
#[ignore]
fn dns_interceptor() {
    let test_helper = test_helper::TestHelper::new("dns-interceptor", vec!["--rustfmt"]);

    test_helper.run_graphgen();
    test_helper.run_diff();
}

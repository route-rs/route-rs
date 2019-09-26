#[allow(dead_code)]
mod test_helper;

// TODO: re-enable after Link API is stabilized
//#[test]
//fn trivial_identity() {
//    let test_helper = test_helper::TestHelper::new(
//        "trivial-identity",
//        vec!["--rustfmt", "--local-modules", "packets"],
//    );
//
//    test_helper.run_graphgen();
//    test_helper.run_diff();
//}
//
//#[test]
//fn dns_interceptor() {
//    let test_helper = test_helper::TestHelper::new(
//        "dns-interceptor",
//        vec!["--rustfmt", "--runtime-modules", "link"],
//    );
//
//    test_helper.run_graphgen();
//    test_helper.run_diff();
//}

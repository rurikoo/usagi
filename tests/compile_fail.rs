#[test]
#[ignore = "run explicitly, snapshots depend on the rustc version"]
fn compile_fail() {
    let t = trybuild::TestCases::new();
    t.compile_fail("tests/ui/*.rs");
}

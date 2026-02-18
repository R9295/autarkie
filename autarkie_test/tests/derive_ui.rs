#[test]
fn derive_pass() {
    let t = trybuild::TestCases::new();
    t.pass("tests/ui/pass/*.rs");
}

#[test]
fn derive_fail() {
    let t = trybuild::TestCases::new();
    t.compile_fail("tests/ui/fail/*.rs");
}

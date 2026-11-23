//! Compiler-facing tests for the public derive macro.

#[test]
fn reflect_derive_compiler_cases() {
    let cases = trybuild::TestCases::new();
    cases.pass("tests/ui/pass/*.rs");
    cases.compile_fail("tests/ui/fail/*.rs");
}

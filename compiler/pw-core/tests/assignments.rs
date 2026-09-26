//! **Which bindings may be assigned, and with what** (PW0611, PW0607;
//! ADR-0051).
//!
//! `x = e` compiles since 2026-09-25. Until then nothing said which bindings
//! an assignment may change: a parameter, or a `let` without `mut`, was
//! assigned and checked clean, and so was a value of another type.

use pw_core::check::check_sources;

fn codes(src: &str) -> Vec<&'static str> {
    check_sources(&[("t.pw".to_string(), src.to_string())])[0]
        .1
        .iter()
        .map(|d| d.code)
        .collect()
}

#[test]
fn a_let_mut_binding_is_assigned() {
    let src = "module m\n\nfn f() -> Int !{} {\n    let mut x = 0\n    x = 1\n    x\n}\n";
    assert_eq!(codes(src), Vec::<&str>::new());
}

#[test]
fn a_binding_without_mut_is_not() {
    let src = "module m\n\nfn f() -> Int !{} {\n    let x = 0\n    x = 1\n    x\n}\n";
    assert_eq!(codes(src), ["PW0611"]);
}

#[test]
fn a_parameter_and_a_loop_variable_are_not() {
    let param = "module m\n\nfn f(n: Int) -> Int !{} {\n    n = 1\n    n\n}\n";
    assert_eq!(codes(param), ["PW0611"]);
    let each = "module m\n\nfn f(xs: List<Int>) -> Int !{} {\n    for x in xs {\n        x = 1\n    }\n    0\n}\n";
    assert_eq!(codes(each), ["PW0611"]);
}

#[test]
fn a_module_level_let_mut_is_assigned_and_a_let_is_not() {
    let src = "module m\n\nlet mut seen: Int = 0\n\nfn f() -> () !{} {\n    seen = 1\n}\n";
    assert_eq!(codes(src), Vec::<&str>::new());
    let fixed = src.replace("let mut seen", "let seen");
    assert_eq!(codes(&fixed), ["PW0611"]);
}

#[test]
fn the_innermost_binding_decides() {
    // `let x` rebinds `x`, and the new binding is not `mut`.
    let src =
        "module m\n\nfn f() -> Int !{} {\n    let mut x = 0\n    let x = 1\n    x = 2\n    x\n}\n";
    assert_eq!(codes(src), ["PW0611"]);
}

#[test]
fn an_assignment_keeps_the_bindings_type() {
    let src = "module m\n\nfn f() -> Int !{} {\n    let mut x = 0\n    x = \"a\"\n    x\n}\n";
    assert_eq!(codes(src), ["PW0607"]);
    let fine = src.replace("x = \"a\"", "x = 2");
    assert_eq!(codes(&fine), Vec::<&str>::new());
}

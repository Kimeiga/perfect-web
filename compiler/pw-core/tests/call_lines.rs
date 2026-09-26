//! **A call's arguments open on the callee's line** (ADR-0083).
//!
//! A `(` at the start of a line begins a new statement, as `<`, `-` and `!`
//! do. Until 2026-09-26 it continued the expression before it, so `g(n)`
//! followed by `()` on the next line parsed as the one call `g(n)()`, and a
//! correct program was refused for "`` does not resolve": the callee of that
//! call is a call, which names nothing. Each test states one case, with a
//! control.

use pw_core::check::check_sources;

fn reported(src: &str) -> Vec<String> {
    check_sources(&[("t.pw".to_string(), src.to_string())])
        .into_iter()
        .flat_map(|(_, ds)| ds.into_iter().map(|d| format!("{} {}", d.code, d.message)))
        .collect()
}

fn clean(src: &str) {
    let found = reported(src);
    assert!(found.is_empty(), "{src}: {found:#?}");
}

#[test]
fn a_parenthesis_on_the_next_line_begins_a_statement() {
    // A call, then the unit value.
    clean(
        "module t\n\nfn g(n: Int) -> () !{} { () }\n\nfn f(n: Int) -> () !{} {\n    g(n)\n    ()\n}\n",
    );
    // A binding holding a function, then a parenthesised result.
    clean(
        "module t\n\nfn add(a: Int) -> fn(Int) -> Int !{} { (b) => a + b }\n\n\
         fn f() -> Int !{} {\n    let h = add(1)\n    (2 + 3) * 4\n}\n",
    );
}

#[test]
fn a_call_still_breaks_inside_its_arguments() {
    clean(
        "module t\n\nfn g(a: Int, b: Int) -> Int !{} { a + b }\n\n\
         fn f() -> Int !{} {\n    g(\n        1,\n        2\n    )\n}\n",
    );
    // And a chain continues on a line that starts with `.`.
    clean("module t\n\ntype P = P { x: Int }\n\nfn f(p: P) -> Int !{} {\n    p\n        .x\n}\n");
}

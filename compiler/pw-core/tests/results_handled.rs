//! **A failure is handled** (ADR-0099).
//!
//! A `Result` carries a failure, and a value nothing uses handles neither of
//! its cases. Until 2026-09-26 a statement's `Result` was dropped without a
//! word: of two `Carts.add(..)` statements, the first could fail and the
//! command went on. Nine corpus fixtures dropped a rollback's, a clear's or an
//! add's, one of them `@expect: clean`. Each test states one case, with a
//! control.

use pw_core::check::check_sources;

const SAVE: &str = "module t\n\nfn save(n: Int) -> Result<Int, String> !{} { Ok(n) }\n\n";

fn reported(decl: &str) -> Vec<String> {
    check_sources(&[("t.pw".to_string(), format!("{SAVE}{decl}\n"))])
        .into_iter()
        .flat_map(|(_, ds)| ds.into_iter().map(|d| format!("{} {}", d.code, d.message)))
        .collect()
}

fn one(decl: &str, says: &str) {
    let found = reported(decl);
    assert_eq!(found.len(), 1, "{decl}\n{found:#?}");
    assert!(found[0].contains(says), "{decl}\n{found:#?}");
}

fn clean(decl: &str) {
    let found = reported(decl);
    assert!(found.is_empty(), "{decl}\n{found:#?}");
}

#[test]
fn a_statements_result_is_handled() {
    one(
        "fn f() -> Result<Int, String> !{} {\n    save(1)\n    save(2)\n}",
        "PW0618 this `Result<Int, String>` is dropped, and its failure with it",
    );
    // Passed on, handled, or discarded by a name that says so.
    clean("fn f() -> Result<Int, String> !{} {\n    save(1)?\n    save(2)\n}");
    clean(
        "fn f() -> Result<Int, String> !{} {\n    match save(1) {\n        Ok(_) => save(2),\n        \
         Err(e) => Err(e),\n    }\n}",
    );
    clean("fn f() -> Result<Int, String> !{} {\n    let _ignored = save(1)\n    save(2)\n}");
}

/// A body declared `-> ()` discards its last value, and a loop's body is
/// discarded every time round.
#[test]
fn a_discarded_value_is_not_a_result() {
    one(
        "fn f() -> () !{} {\n    save(1)\n}",
        "PW0618 this `Result<Int, String>` is dropped",
    );
    one(
        "fn f(xs: List<Int>) -> () !{} {\n    for x in xs {\n        save(x)\n    }\n    ()\n}",
        "PW0618 this `Result<Int, String>` is dropped",
    );
}

/// What is returned is used: a body's last value, a `return`'s, and a
/// lambda's, which is its caller's to use.
#[test]
fn a_returned_result_is_used() {
    clean("fn f() -> Result<Int, String> !{} { save(1) }");
    clean(
        "fn f(b: Bool) -> Result<Int, String> !{} {\n    if b {\n        return save(1)\n    }\n    save(2)\n}",
    );
    clean("fn f() -> Result<Int, String> !{} {\n    let g = fn(n: Int) save(n)\n    g(1)\n}");
    // An `acquire` clause's value is the handle the platform takes.
    clean(
        "component Widget(n: Int) {\n    placement browser\n\n    resource handle when true {\n        \
         scope component\n        acquire { save(n) }\n        release(h) { () }\n    }\n\n    \
         view { <section aria-label=\"w\" /> }\n}",
    );
}

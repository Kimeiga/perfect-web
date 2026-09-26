//! **A call through a field holding a function is checked** (ADR-0077).
//!
//! `r.f(x)`, where `f` is a field of `r`'s type, calls the value the field
//! holds, as a call through a binding does (ADR-0068). Until 2026-09-26 the
//! call resolved to nothing, so a wrong argument, a wrong number of them and
//! a wrong use of the result all passed `pw check`; the backend refused the
//! call by name. Each test states one case, with a control.

use pw_core::check::check_sources;

fn reported(src: &str) -> Vec<String> {
    check_sources(&[("t.pw".to_string(), src.to_string())])
        .into_iter()
        .flat_map(|(_, ds)| ds.into_iter().map(|d| format!("{} {}", d.code, d.message)))
        .collect()
}

fn program(body: &str) -> String {
    format!(
        "module t\n\ntype R = R {{ f: fn(Int) -> Int, x: Int }}\n\n\
         fn g(r: R, n: Int) -> Int !{{}} {{ n }}\n\n{body}\n"
    )
}

fn refused(body: &str, code: &str) {
    let found = reported(&program(body));
    assert_eq!(found.len(), 1, "{body}: {found:#?}");
    assert!(found[0].starts_with(code), "{body}: {found:#?}");
}

fn clean(body: &str) {
    let found = reported(&program(body));
    assert!(found.is_empty(), "{body}: {found:#?}");
}

#[test]
fn a_call_through_a_field_is_checked() {
    refused("fn h(r: R) -> Int !{} { r.f(\"a\") }", "PW0605");
    refused("fn h(r: R) -> String !{} { r.f(1) }", "PW0606");
    refused("fn h(r: R) -> Int !{} { r.f(1, 2) }", "PW0604");
    clean("fn h(r: R) -> Int !{} { r.f(1) }");
}

#[test]
fn a_field_that_is_not_a_function_is_not_called() {
    refused("fn h(r: R) -> Int !{} { r.x(1) }", "PW0614");
    clean("fn h(r: R) -> Int !{} { r.x }");
}

/// A declaration that takes the value first is its member, as before.
#[test]
fn a_declared_member_is_still_a_declaration() {
    refused("fn h(r: R) -> Int !{} { r.g(\"a\") }", "PW0605");
    clean("fn h(r: R) -> Int !{} { r.g(1) }");
}

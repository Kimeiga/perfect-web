//! **A resumable handler does not capture a function** (ADR-0086).
//!
//! A handler's captures are written onto the element as data, and read back
//! when it runs. A function is code, not data. Until 2026-09-26 a handler
//! that captured one checked, and the build refused the handler for a name
//! that "names no declaration". Each test states one case, with a control.

use pw_core::check::check_sources;

fn reported(view: &str) -> Vec<String> {
    let src = format!("module t\n\nfn go(n: Int) -> () !{{}} {{ () }}\n\n{view}\n");
    check_sources(&[("t.pw".to_string(), src)])
        .into_iter()
        .flat_map(|(_, ds)| ds.into_iter().map(|d| format!("{} {}", d.code, d.message)))
        .collect()
}

fn refused(view: &str) {
    let found = reported(view);
    assert_eq!(found.len(), 1, "{view}: {found:#?}");
    assert!(
        found[0]
            .starts_with("PW5008 handler captures `f: fn(Int) -> ()`, which is not serializable"),
        "{view}: {found:#?}"
    );
}

#[test]
fn a_handler_that_captures_a_function_is_refused() {
    // A parameter of function type, and a local declared one. Each handler
    // captures the `n` it reads too (ADR-0110), so the function is the one
    // defect.
    refused(
        "view V(n: Int, f: fn(Int) -> ()) !{} {\n    <button type=\"button\" on:press={resumable(captures = { f, n }) => f(n)}>Go</button>\n}",
    );
    refused(
        "view V(n: Int) !{} {\n    let f: fn(Int) -> () = go\n    <button type=\"button\" on:press={resumable(captures = { f, n }) => f(n)}>Go</button>\n}",
    );
    // The data it would have been given, captured, and the function called by name.
    let found = reported(
        "view V(n: Int) !{} {\n    <button type=\"button\" on:press={resumable(captures = { n }) => go(n)}>Go</button>\n}",
    );
    assert!(found.is_empty(), "{found:#?}");
}

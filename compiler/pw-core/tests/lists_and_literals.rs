//! **A list's items share one type, and an `Int` literal fits an `Int`**
//! (ADR-0069).
//!
//! Until 2026-09-26 a list's items were joined, and where two disagreed the
//! element type was a hole, so `[1, "a"]` passed where a `List<Int>` is
//! declared. An `Int` literal past 64 bits was an `Int` to the checker, and
//! the backend was the first to refuse it. Each test states one case, with a
//! control that what is right is not refused.

use pw_core::check::check_sources;

fn std_lib() -> Vec<(String, String)> {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../packages/pw-std");
    let mut out = Vec::new();
    for e in std::fs::read_dir(root).expect("pw-std") {
        let p = e.expect("entry").path();
        if p.extension().is_some_and(|x| x == "pw") {
            out.push((
                p.display().to_string(),
                std::fs::read_to_string(&p).expect("read"),
            ));
        }
    }
    out.sort();
    out
}

fn reported(src: &str) -> Vec<String> {
    let mut all = std_lib();
    all.push(("t.pw".to_string(), src.to_string()));
    check_sources(&all)
        .into_iter()
        .filter(|(n, _)| n == "t.pw")
        .flat_map(|(_, ds)| ds.into_iter().map(|d| format!("{} {}", d.code, d.message)))
        .collect()
}

fn refused(src: &str, code: &str) {
    let found = reported(src);
    assert!(!found.is_empty(), "nothing reported");
    assert!(found.iter().any(|d| d.starts_with(code)), "{found:#?}");
}

fn none(src: &str) {
    let found = reported(src);
    assert!(found.is_empty(), "{found:#?}");
}

fn program(body: &str) -> String {
    format!("module t\n\nimport List\n\n{body}\n")
}

#[test]
fn a_lists_items_share_one_type() {
    refused(&program("fn f() -> List<Int> { [1, \"a\"] }"), "PW0615");
    refused(
        &program("fn f() -> Int { List.length([1, \"a\", true]) }"),
        "PW0615",
    );
    none(&program("fn f() -> List<Int> { [1, 2, 3] }"));
    // An item of any type is an item of the others' type.
    none(&program("fn f() -> List<Option<Int>> { [None, Some(1)] }"));
}

#[test]
fn an_int_literal_fits_an_int() {
    refused(&program("fn f() -> Int { 99999999999999999999 }"), "PW0616");
    none(&program("fn f() -> Int { 9223372036854775807 }"));
}

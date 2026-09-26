//! **A record is built with each of its fields once, and no other**, and an
//! `if` without `else` is no value (ADR-0067).
//!
//! Until 2026-09-26 a record built by its fields' names was checked field by
//! field against the fields it gave, and nothing else: a field it left out,
//! a field its type does not declare, and a field given twice each passed
//! `pw check`, and the backend was the first to refuse them. An `if` without
//! `else` was a value of no stated type, so `fn f(n: Int) -> Int { if n > 0
//! { 1 } }` passed too, though the backend gives it the unit value when its
//! condition is false. Each test states one case, with a control that what is
//! right is not refused.

use pw_core::check::check_sources;

fn files(dirs: &[&str]) -> Vec<(String, String)> {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut out = Vec::new();
    for d in dirs {
        let mut paths: Vec<std::path::PathBuf> = std::fs::read_dir(root.join(d))
            .unwrap_or_else(|e| panic!("{d}: {e}"))
            .map(|e| e.expect("entry").path())
            .filter(|p| p.extension().is_some_and(|x| x == "pw"))
            .collect();
        paths.sort();
        for p in paths {
            out.push((
                p.display().to_string(),
                std::fs::read_to_string(&p).expect("read"),
            ));
        }
    }
    out
}

fn reported(src: &str) -> Vec<String> {
    let mut all = files(&["packages/pw-std"]);
    all.push(("t.pw".to_string(), src.to_string()));
    check_sources(&all)
        .into_iter()
        .filter(|(n, _)| n == "t.pw")
        .flat_map(|(_, ds)| ds.into_iter().map(|d| format!("{} {}", d.code, d.message)))
        .collect()
}

fn refused(src: &str, code: &str, says: &str) {
    let found = reported(src);
    assert!(!found.is_empty(), "nothing reported");
    assert!(found.iter().all(|d| d.starts_with(code)), "{found:#?}");
    assert!(found.iter().any(|d| d.contains(says)), "{found:#?}");
}

fn none(src: &str) {
    let found = reported(src);
    assert!(found.is_empty(), "{found:#?}");
}

fn program(body: &str) -> String {
    format!("module t\n\ntype Box = Box {{ value: Int, label: String }}\n\n{body}\n")
}

#[test]
fn a_field_left_out_is_refused() {
    refused(
        &program("fn f(n: Int) -> Box { Box { value: n } }"),
        "PW0612",
        "`label`",
    );
    none(&program(
        "fn f(n: Int) -> Box { Box { value: n, label: \"a\" } }",
    ));
}

#[test]
fn a_field_the_type_does_not_declare_is_refused() {
    refused(
        &program("fn f(n: Int) -> Box { Box { value: n, label: \"a\", colour: \"red\" } }"),
        "PW0612",
        "`colour`",
    );
}

#[test]
fn a_field_given_twice_is_refused() {
    refused(
        &program("fn f(n: Int) -> Box { Box { value: n, value: 2, label: \"a\" } }"),
        "PW0612",
        "`value`",
    );
}

#[test]
fn a_shorthand_field_is_given() {
    none(&program(
        "fn f(n: Int) -> Box {\n    let label = \"a\"\n    Box { value: n, label }\n}",
    ));
}

#[test]
fn an_if_without_else_is_no_value() {
    refused(
        &program("fn f(n: Int) -> Int {\n    if n > 0 { 1 }\n}"),
        "PW0606",
        "`Unit`",
    );
    none(&program(
        "fn f(n: Int) -> Int {\n    if n > 0 { 1 } else { 0 }\n}",
    ));
    // As a statement it is what it always was.
    none(&program(
        "fn f(n: Int) -> Int {\n    let mut t = 0\n    if n > 0 { t = 1 }\n    t\n}",
    ));
}

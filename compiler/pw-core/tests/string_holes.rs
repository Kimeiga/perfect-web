//! **What a string interpolates has a text form** (ADR-0084).
//!
//! A string's holes are written as text, as a template's are (ADR-0074): a
//! `String`, an `Int` or a `Bool`, or an opaque type over one. Until
//! 2026-09-26 nothing related them, so `"{xs}"` over a list, a record or an
//! `Option` checked, and the backend was the first to refuse it ("has no text
//! form here"). Two fixtures logged a `Result` that way and are corrected.
//! Each test states one case, with a control.

use pw_core::check::check_sources;

fn reported(decls: &str) -> Vec<String> {
    let src = format!(
        "module t\n\ntype P = P {{ x: Int }}\n\nopaque type Code = String\n\n\
         view V(r: P) !{{}} {{\n    <a href=\"/x/{{r}}\">x</a>\n}}\n\n{decls}\n"
    );
    check_sources(&[("t.pw".to_string(), src)])
        .into_iter()
        .flat_map(|(_, ds)| ds.into_iter().map(|d| format!("{} {}", d.code, d.message)))
        .filter(|d| !d.contains("`href`"))
        .collect()
}

fn one(decls: &str, says: &str) {
    let found = reported(decls);
    assert_eq!(found.len(), 1, "{decls}: {found:#?}");
    assert!(found[0].contains(says), "{decls}: {found:#?}");
}

fn clean(decls: &str) {
    let found = reported(decls);
    assert!(found.is_empty(), "{decls}: {found:#?}");
}

#[test]
fn a_strings_hole_has_a_text_form() {
    one(
        "fn f(n: Int) -> String !{} { \"{[n, 2]}\" }",
        "PW0609 `\"{..}\"` is `List<Int>`, which has no text form",
    );
    one(
        "fn f(p: P) -> String !{} { \"{p}\" }",
        "PW0609 `\"{..}\"` is `t.P`, which has no text form",
    );
    one(
        "fn f(x: Float) -> String !{} { \"{x}\" }",
        "PW0609 `\"{..}\"` is `Float`, which has no text form",
    );
    for hole in ["s", "n", "b", "c", "p.x"] {
        clean(&format!(
            "fn f(s: String, n: Int, b: Bool, c: Code, p: P) -> String !{{}} {{ \"{{{hole}}}\" }}"
        ));
    }
}

#[test]
fn a_value_that_may_be_absent_is_taken_apart_first() {
    one(
        "fn f(o: Option<Int>) -> String !{} { \"{o}\" }",
        "PW0600 `\"{..}\"` is `Option<Int>`, a value that may be absent; `match` takes it apart",
    );
    clean(
        "fn f(o: Option<Int>) -> String !{} {\n    match o {\n        Some(n) => \"{n}\",\n        None => \"none\",\n    }\n}",
    );
}

/// A template attribute's string is the template's (ADR-0074): reported
/// once, naming the attribute.
#[test]
fn an_attributes_string_is_the_templates() {
    let src = "module t\n\ntype P = P { x: Int }\n\nview V(r: P) !{} {\n    <a href=\"/x/{r}\">x</a>\n}\n";
    let found: Vec<String> = check_sources(&[("t.pw".to_string(), src.to_string())])
        .into_iter()
        .flat_map(|(_, ds)| ds.into_iter().map(|d| format!("{} {}", d.code, d.message)))
        .collect();
    assert_eq!(
        found,
        vec!["PW0609 `href` is `t.P`, which has no text form"],
        "{found:#?}"
    );
}

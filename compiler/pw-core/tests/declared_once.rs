//! **A name is written once where it is declared** (ADR-0098).
//!
//! A module declares each name once in a namespace (PW0024). Inside a
//! declaration nothing held a name to that until 2026-09-26, and each of
//! these checked:
//! - a parameter taken twice, `fn f(x: Int, x: String)`;
//! - a record's field declared twice, and a sum type's case;
//! - a policy written twice, `cache private` then `cache shared`, where every
//!   reader takes the first, so the order decided whether a session's data
//!   was refused a shared cache;
//! - an attribute given twice, `<a href="/a" href="/b">`, where HTML keeps
//!   the first.
//!
//! Each test states one case, with a control.

use pw_core::check::check_sources;

fn reported(src: &str) -> Vec<String> {
    check_sources(&[("t.pw".to_string(), src.to_string())])
        .into_iter()
        .flat_map(|(_, ds)| ds.into_iter().map(|d| format!("{} {}", d.code, d.message)))
        .collect()
}

fn one(src: &str, says: &str) {
    let found = reported(src);
    assert_eq!(found.len(), 1, "{src}\n{found:#?}");
    assert!(found[0].contains(says), "{src}\n{found:#?}");
}

fn clean(src: &str) {
    let found = reported(src);
    assert!(found.is_empty(), "{src}\n{found:#?}");
}

#[test]
fn a_parameter_is_taken_once() {
    one(
        "module t\n\nfn f(x: Int, x: Int) -> Int !{} { x }\n",
        "PW0028 `f` takes `x` twice",
    );
    one(
        "module t\n\nfn f<T, T>(x: T) -> T !{} { x }\n",
        "PW0028 `f` takes `T` twice",
    );
    clean("module t\n\nfn f(x: Int, y: Int) -> Int !{} { x + y }\n");
}

#[test]
fn a_field_and_a_case_are_declared_once() {
    one(
        "module t\n\ntype P = P { x: Int, x: Int }\n",
        "PW0028 `P` declares `x` twice",
    );
    one(
        "module t\n\ntype S =\n    | A\n    | A\n",
        "PW0028 `S` declares `A` twice",
    );
    clean("module t\n\ntype P = P { x: Int, y: Int }\n\ntype S =\n    | A\n    | B\n");
}

#[test]
fn a_policy_is_written_once() {
    let query = |policies: &str| {
        format!("module t\n\nsession query Mine(id: Int) -> Int !{{}}\n{policies}{{\n    id\n}}\n")
    };
    one(
        &query("    freshness 0.seconds\n    cache private\n    cache shared\n"),
        "PW0028 `Mine` declares `cache` twice",
    );
    clean(&query("    freshness 0.seconds\n    cache private\n"));
    // An effect has as many impacts as it has facets, as the platform's
    // `dom.mutate` writes them.
    clean(
        "module t\n\nprelude Effect\n\neffect paint {\n    placement browser\n    \
         capability paint\n    impact dom_write\n    impact paint_write\n}\n",
    );
}

#[test]
fn an_attribute_is_given_once() {
    let view = |attrs: &str| format!("module t\n\nview V() !{{}} {{\n    <a {attrs}>x</a>\n}}\n");
    one(
        &view("href=\"/a\" href=\"/b\""),
        "PW0028 `<a>` is given `href` twice",
    );
    // HTML reads an attribute's name in any case (ADR-0095).
    one(
        &view("href=\"/a\" HREF=\"/b\""),
        "PW0028 `<a>` is given `HREF` twice",
    );
    clean(&view("href=\"/a\" title=\"b\""));
}

/// A pattern binds a name once: `Pair(a, a)` bound `a` as an `Int` and again
/// as a `String`, and a body saw the second.
#[test]
fn a_pattern_binds_a_name_once() {
    let src = |arm: &str, lambda: &str| {
        format!(
            "module t\n\ntype P =\n    | Pair(Int, String)\n    | Other\n\n\
             fn f(p: P) -> Int !{{}} {{\n    let g = {lambda}\n    \
             match p {{\n        {arm} => g(1, 2),\n        P.Other => 0,\n    }}\n}}\n"
        )
    };
    one(
        &src("P.Pair(a, a)", "fn(acc: Int, x: Int) acc + x"),
        "PW0028 `a` is bound twice in one pattern",
    );
    one(
        &src("P.Pair(a, b)", "fn(acc: Int, acc: Int) acc"),
        "PW0028 `acc` is bound twice in one pattern",
    );
    clean(&src("P.Pair(a, b)", "fn(acc: Int, x: Int) acc + x"));
}

//! **What each construct takes, checked** (ADR-0068).
//!
//! A probe on 2026-09-26 found constructs whose operands were related to
//! nothing, so a wrong program passed `pw check`:
//! - a call through a function value, whatever it was given, and whatever it
//!   answered;
//! - a value called that is not a function;
//! - a call to a local function value that shares a declaration's name,
//!   checked against the declaration;
//! - `for` over a value that is not a list;
//! - `?` on a value with no failure;
//! - an `if` or a `match` whose branches produce two types, where its value
//!   is bound or passed.
//!
//! The last found a silent miscompile: an `elif` chain lowered as its first
//! branch and, for its `else`, the next condition. Each test states one case,
//! with a control that what is right is not refused.

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

fn refused(src: &str, code: &str) -> Vec<String> {
    let found = reported(src);
    assert!(!found.is_empty(), "nothing reported");
    assert!(found.iter().any(|d| d.starts_with(code)), "{found:#?}");
    found
}

fn none(src: &str) {
    let found = reported(src);
    assert!(found.is_empty(), "{found:#?}");
}

fn program(body: &str) -> String {
    format!("module t\n\nimport List\n\n{body}\n")
}

#[test]
fn a_call_through_a_function_value_is_checked() {
    // Its arguments, bound by a `let` or a parameter.
    let wrong =
        program("fn f(n: Int) -> Int {\n    let g: fn(Int) -> Int = x => x + 1\n    g(\"a\")\n}");
    refused(&wrong, "PW0605");
    none(&wrong.replace("g(\"a\")", "g(n)"));
    refused(
        &program("fn f(g: fn(Int) -> Int) -> Int { g(\"a\") }"),
        "PW0605",
    );
    // Its arity.
    refused(
        &program("fn f(g: fn(Int) -> Int) -> Int { g(1, 2) }"),
        "PW0604",
    );
    // And its result.
    refused(
        &program("fn f(g: fn(Int) -> Int) -> String { g(1) }"),
        "PW0606",
    );
}

#[test]
fn a_value_called_must_be_a_function() {
    let wrong = program("fn f(n: Int) -> Int {\n    let k = 1\n    k(n)\n}");
    let found = refused(&wrong, "PW0614");
    assert!(found.iter().any(|d| d.contains("`k`")), "{found:#?}");
    none(&program(
        "fn f(n: Int) -> Int {\n    let k: fn(Int) -> Int = x => x\n    k(n)\n}",
    ));
}

#[test]
fn a_local_function_value_is_not_the_declaration_it_shadows() {
    // `double` here is the local, a function of a `String`: the declaration
    // of the name takes an `Int`.
    none(&program(
        "fn double(n: Int) -> Int { n * 2 }\n\nfn f(s: String) -> String {\n    let double: fn(String) -> String = w => \"{w}{w}\"\n    double(s)\n}",
    ));
}

#[test]
fn a_for_loop_runs_over_a_list() {
    let wrong = program(
        "fn f(n: Int) -> Int {\n    let mut t = 0\n    for x in n {\n        t = t + 1\n    }\n    t\n}",
    );
    refused(&wrong, "PW0609");
    none(&program(
        "fn f(xs: List<Int>) -> Int {\n    let mut t = 0\n    for x in xs {\n        t = t + x\n    }\n    t\n}",
    ));
}

#[test]
fn a_try_takes_an_option_or_a_result() {
    refused(
        &program("fn f(b: Bool) -> Option<Int> {\n    let v = b?\n    Some(1)\n}"),
        "PW0609",
    );
    none(&program(
        "fn f(o: Option<Int>) -> Option<Int> {\n    let v = o?\n    Some(v)\n}",
    ));
}

#[test]
fn branches_whose_value_is_used_produce_one_type() {
    refused(
        &program("fn f(c: Bool) -> Int {\n    let x = if c { 1 } else { \"a\" }\n    7\n}"),
        "PW0613",
    );
    refused(
        &program(
            "fn f(o: Option<Int>) -> Int {\n    let x = match o {\n        Some(n) => n,\n        None => \"none\",\n    }\n    7\n}",
        ),
        "PW0613",
    );
    none(&program(
        "fn f(c: Bool) -> Int {\n    let x = if c { 1 } else { 2 }\n    x\n}",
    ));
    // A statement's value is discarded, and not checked here.
    none(&program(
        "fn f(c: Bool) -> Int {\n    if c { 1 } else { \"a\" }\n    7\n}",
    ));
}

#[test]
fn an_elif_chain_is_one_if_nested_in_another() {
    // Lowered as `if a { 1 } else b`, this chain's value was a `Bool` when
    // `a` was false: its branches disagreed.
    none(&program(
        "fn f(n: Int) -> Int {\n    let x = if n > 10 { 1 } elif n > 5 { 2 } else { 3 }\n    x\n}",
    ));
    none(&program(
        "fn f(n: Int) -> Int {\n    let x = if n > 10 { 1 } else if n > 5 { 2 } else { 3 }\n    x\n}",
    ));
    refused(
        &program(
            "fn f(n: Int) -> Int {\n    let x = if n > 10 { 1 } elif n > 5 { \"two\" } else { 3 }\n    x\n}",
        ),
        "PW0613",
    );
}

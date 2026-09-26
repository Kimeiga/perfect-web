//! **Nested and literal patterns are analysed** (ADR-0060).
//!
//! Until 2026-09-26 the exhaustiveness analysis read the payload of `Some`,
//! `Ok` and `Err` as one opaque type, read a declared case's fields by their
//! spelling, and did not read a literal at all. Each such match was Blocked:
//! neither proven nor refused, so `match o { Some(Empty) => 1, None => 0 }`
//! over an `Option<Shape>` passed `pw check`, and the backend compiled
//! `Empty` there as a binding that took every payload. A `Bool`, an `Int` or
//! a `String` could not be matched at all. Each test states one of those,
//! and a control.

use pw_core::check::check_sources;

fn reported(src: &str) -> Vec<String> {
    check_sources(&[("t.pw".to_string(), src.to_string())])
        .into_iter()
        .flat_map(|(_, ds)| ds.into_iter().map(|d| format!("{} {}", d.code, d.message)))
        .collect()
}

fn repairs(src: &str) -> Vec<String> {
    check_sources(&[("t.pw".to_string(), src.to_string())])
        .into_iter()
        .flat_map(|(_, ds)| ds.into_iter().flat_map(|d| d.repairs))
        .map(|r| r.description)
        .collect()
}

fn program(body: &str) -> String {
    format!(
        "module t\n\ntype Shape =\n    | Circle(Int)\n    | Label(String)\n    | Empty\n\n\
         type Box =\n    | Full(Option<Int>)\n    | Void\n\n{body}\n"
    )
}

const CLEAN: [&str; 0] = [];

#[test]
fn a_case_nested_in_an_option_is_analysed() {
    // `Empty` under `Some` is `Shape`'s case, not a binding.
    let partial = program(
        "fn f(o: Option<Shape>) -> Int {\n    match o {\n        Some(Empty) => 1,\n        None => 0,\n    }\n}",
    );
    assert_eq!(
        reported(&partial),
        ["PW0305 match on `Option<Shape>` is not exhaustive"]
    );
    assert!(
        repairs(&partial).contains(&"add an arm for `Some(Circle(Int))`".to_string()),
        "{:?}",
        repairs(&partial)
    );
    // Control.
    let whole = partial.replace(
        "        None => 0,",
        "        Some(_) => 2,\n        None => 0,",
    );
    assert_eq!(reported(&whole), CLEAN);
}

#[test]
fn an_option_nested_in_a_declared_case_is_analysed() {
    // A declared case's field is its resolved type, not its spelling.
    let partial = program(
        "fn f(b: Box) -> Int {\n    match b {\n        Full(Some(n)) => n,\n        Void => -1,\n    }\n}",
    );
    assert_eq!(repairs(&partial)[0], "add an arm for `Full(None)`");
    let whole = partial.replace(
        "        Void => -1,",
        "        Full(None) => 0,\n        Void => -1,",
    );
    assert_eq!(reported(&whole), CLEAN);
}

#[test]
fn a_bool_is_matched_by_true_and_false() {
    let partial = program("fn f(b: Bool) -> Int {\n    match b {\n        true => 1,\n    }\n}");
    assert_eq!(
        reported(&partial),
        ["PW0305 match on `Bool` is not exhaustive"]
    );
    assert_eq!(repairs(&partial)[0], "add an arm for `false`");
    let whole = partial.replace(
        "        true => 1,",
        "        true => 1,\n        false => 0,",
    );
    assert_eq!(reported(&whole), CLEAN);
}

#[test]
fn no_list_of_literals_is_every_int() {
    let partial = program(
        "fn f(n: Int) -> String {\n    match n {\n        -1 => \"minus one\",\n        0 => \"zero\",\n    }\n}",
    );
    assert_eq!(
        reported(&partial),
        ["PW0305 match on `Int` is not exhaustive"]
    );
    assert_eq!(
        repairs(&partial),
        ["add an arm `_ =>` for the values no literal names"]
    );
    let whole = partial.replace(
        "        0 => \"zero\",",
        "        0 => \"zero\",\n        _ => \"many\",",
    );
    assert_eq!(reported(&whole), CLEAN);
}

#[test]
fn a_literal_of_another_type_is_refused() {
    let wrong = program(
        "fn f(n: Int) -> Int {\n    match n {\n        \"a\" => 1,\n        _ => 0,\n    }\n}",
    );
    assert_eq!(
        reported(&wrong),
        ["PW0608 `\"a\"` is not a constructor of `Int`"]
    );
    assert_eq!(
        repairs(&wrong),
        ["a pattern against `Int` is one of its literals, or `_`"]
    );
    let right = wrong.replace("\"a\" => 1", "7 => 1");
    assert_eq!(reported(&right), CLEAN);
}

#[test]
fn a_name_nested_in_a_pattern_is_typed_in_its_arm() {
    let wrong = program(
        "fn f(o: Option<Option<Int>>) -> Int {\n    match o {\n        Some(Some(x)) => x + \"a\",\n        _ => 0,\n    }\n}",
    );
    assert_eq!(
        reported(&wrong),
        ["PW0609 the right side of `+`, like its left, must be `Int`, and this is `String`"]
    );
    let right = wrong.replace("x + \"a\"", "x + 1");
    assert_eq!(reported(&right), CLEAN);
}

#[test]
fn a_name_alone_binds_the_whole_value() {
    // `other` is the scrutinee, typed; until ADR-0060 it was unknown.
    let wrong = program(
        "fn f(s: String) -> Int {\n    match s {\n        \"a\" => 1,\n        other => other + 1,\n    }\n}",
    );
    assert_eq!(
        reported(&wrong),
        [
            "PW0609 the left side of `+` must be `Int or Float`, and this is `String`",
            "PW0609 the right side of `+`, like its left, must be `String`, and this is `Int`"
        ]
    );
}

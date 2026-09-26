//! **A declared sum type's cases are typed where they are written**
//! (ADR-0059).
//!
//! Until 2026-09-26 a case had no type. `Shape.Circle("x")` passed `pw check`
//! though `Circle` holds an `Int`, `Shape.Bogus(1)` named a case nothing
//! declares and passed too, and a payload bound in an arm (`Rect(w, h)`) was
//! unknown to every relation in the arm's body. A pattern written through its
//! type, `Shape.Empty`, parsed as a binding of that dotted name, so it matched
//! everything and proved a match exhaustive that was not. Each test below
//! states one of those, and a control that the rule does not refuse what is
//! right.

use pw_core::check::check_sources;

fn std_lib() -> Vec<(String, String)> {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../packages/pw-std");
    let mut out = Vec::new();
    for e in std::fs::read_dir(root).expect("pw-std") {
        let p = e.expect("entry").path();
        if p.extension().is_some_and(|x| x == "pw") {
            out.push((
                p.file_name().unwrap().to_string_lossy().to_string(),
                std::fs::read_to_string(&p).expect("read"),
            ));
        }
    }
    out.sort();
    out
}

/// `code message` for each diagnostic on `t.pw`, checked with the standard
/// library.
fn reported(src: &str) -> Vec<String> {
    let mut files = std_lib();
    files.push(("t.pw".to_string(), src.to_string()));
    check_sources(&files)
        .into_iter()
        .filter(|(n, _)| n == "t.pw")
        .flat_map(|(_, ds)| ds.into_iter().map(|d| format!("{} {}", d.code, d.message)))
        .collect()
}

/// The repairs each diagnostic on `t.pw` offers.
fn repairs(src: &str) -> Vec<String> {
    let mut files = std_lib();
    files.push(("t.pw".to_string(), src.to_string()));
    check_sources(&files)
        .into_iter()
        .filter(|(n, _)| n == "t.pw")
        .flat_map(|(_, ds)| ds.into_iter().flat_map(|d| d.repairs))
        .map(|r| r.description)
        .collect()
}

fn program(decls: &str) -> String {
    format!(
        "module t\n\nimport List\nimport String\n\n\
         type Shape =\n    | Circle(Int)\n    | Rect(Int, Int)\n    | Label(String)\n    | Empty\n\n\
         {decls}\n"
    )
}

const CLEAN: [&str; 0] = [];

#[test]
fn a_cases_fields_are_its_arguments() {
    let wrong = program("fn f() -> Shape { Shape.Circle(\"x\") }");
    assert_eq!(
        reported(&wrong),
        ["PW0605 argument 1 of `t.Shape.Circle` is declared `Int` and this is `String`"]
    );
    let few = program("fn f() -> Shape { Shape.Rect(1) }");
    assert_eq!(
        reported(&few),
        ["PW0604 `t.Shape.Rect` declares 2 arguments and this call passes 1"]
    );
    // Control.
    let right = program("fn f() -> Shape { Shape.Rect(1, 2) }");
    assert_eq!(reported(&right), CLEAN);
}

#[test]
fn a_case_its_type_does_not_declare_is_refused() {
    for body in ["Shape.Bogus(1)", "Shape.Bogus"] {
        let src = program(&format!("fn f() -> Shape {{ {body} }}"));
        assert_eq!(
            reported(&src),
            ["PW0608 `Bogus` is not a constructor of `t.Shape`"],
            "{body}"
        );
        assert!(
            repairs(&src)
                .iter()
                .any(|r| r.contains("`Circle`, `Rect`, `Label`, `Empty`")),
            "the repair names the cases: {:?}",
            repairs(&src)
        );
    }
    // Control: each declared case.
    let right =
        program("fn f(b: Bool) -> Shape { if b { Shape.Empty } else { Shape.Label(\"a\") } }");
    assert_eq!(reported(&right), CLEAN);
}

#[test]
fn a_case_is_its_types_value() {
    // A case without a payload is its type; one with a payload is the
    // function that builds it.
    let wrong = program("fn f() -> Int { Shape.Empty }");
    assert_eq!(
        reported(&wrong),
        ["PW0606 `t.f` declares its result `Int` and this produces `t.Shape`"]
    );
    let passed =
        program("fn f(names: List<String>) -> List<Shape> { List.map(names, Shape.Circle) }");
    assert_eq!(
        reported(&passed),
        [
            "PW0605 argument 2 of `List.map` is declared `fn(String) -> ?` and this is \
             `fn(Int) -> t.Shape`"
        ]
    );
    // Control.
    let right = program("fn f(radii: List<Int>) -> List<Shape> { List.map(radii, Shape.Circle) }");
    assert_eq!(reported(&right), CLEAN);
}

#[test]
fn a_bare_case_is_the_one_type_that_has_it() {
    // `Empty` alone is `Shape.Empty`: typed, so a wrong use is seen.
    let wrong = program("fn f() -> Int { Empty }");
    assert_eq!(
        reported(&wrong),
        ["PW0606 `t.f` declares its result `Int` and this produces `t.Shape`"]
    );
    // Two types that have it: neither is meant.
    let two = program("type Box =\n    | Empty\n    | Full(Int)\n\nfn f() -> Shape { Empty }");
    assert_eq!(
        reported(&two),
        ["PW0022 `Empty` is a case of `Shape` and `Box`"]
    );
    assert!(
        repairs(&two)
            .iter()
            .any(|r| r.contains("`Shape.Empty` or `Box.Empty`")),
        "{:?}",
        repairs(&two)
    );
    // Control: written through its type, either is fine.
    let qualified = two.replace(
        "fn f() -> Shape { Empty }",
        "fn f() -> Shape { Shape.Empty }",
    );
    assert_eq!(reported(&qualified), CLEAN);
    let right = program("fn f() -> Shape { Empty }");
    assert_eq!(reported(&right), CLEAN);
}

#[test]
fn a_bare_case_with_a_payload_names_its_qualified_form() {
    let src = program("fn f() -> Shape { Circle(3) }");
    assert_eq!(reported(&src), ["PW0021 `Circle` does not resolve"]);
    assert_eq!(repairs(&src), ["write `Shape.Circle(..)`"]);
}

#[test]
fn a_pattern_through_its_type_is_a_case_not_a_binding() {
    // Until 2026-09-26 `Shape.Empty` bound a name, matched everything, and
    // this match was proven exhaustive.
    let partial =
        program("fn f(s: Shape) -> Int {\n    match s {\n        Shape.Empty => 0,\n    }\n}");
    let got = reported(&partial);
    assert_eq!(got.len(), 1, "{got:?}");
    assert!(
        got[0].starts_with("PW0305 match on `Shape` is not exhaustive"),
        "{got:?}"
    );
    // Control: every case, through its type and alone, with arguments.
    let whole = program(
        "fn f(s: Shape) -> Int {\n    match s {\n        Shape.Circle(r) => r,\n        \
         Rect(w, h) => w * h,\n        Shape.Label(_) => 0,\n        Shape.Empty => 0,\n    }\n}",
    );
    assert_eq!(reported(&whole), CLEAN);
}

#[test]
fn a_patterns_qualifier_must_name_the_type_matched() {
    let src = program(
        "type Box =\n    | Empty\n    | Full(Int)\n\nfn f(s: Shape) -> Int {\n    match s {\n        \
         Box.Empty => 0,\n        _ => 1,\n    }\n}",
    );
    assert_eq!(
        reported(&src),
        ["PW0608 `Box.Empty` is not a constructor of `Shape`"]
    );
    // Control.
    let right = src.replace("Box.Empty => 0", "Shape.Empty => 0");
    assert_eq!(reported(&right), CLEAN);
}

#[test]
fn an_arms_fields_are_typed_in_its_body() {
    let wrong = program(
        "fn f(s: Shape) -> Int {\n    match s {\n        Rect(w, label) => w + label,\n        \
         Label(t) => String.length(t),\n        _ => 0,\n    }\n}",
    )
    .replace("Rect(Int, Int)", "Rect(Int, String)");
    assert_eq!(
        reported(&wrong),
        ["PW0609 the right side of `+`, like its left, must be `Int`, and this is `String`"]
    );
    // Control: the same arm over the declared fields.
    let right = program(
        "fn f(s: Shape) -> Int {\n    match s {\n        Rect(w, h) => w + h,\n        \
         Label(t) => String.length(t),\n        _ => 0,\n    }\n}",
    );
    assert_eq!(reported(&right), CLEAN);
}

#[test]
fn an_options_payload_is_typed_in_its_arm() {
    // The relations walk an arm's body with its bindings; until 2026-09-26
    // `x` here was unknown and this passed.
    let wrong = program(
        "fn f(o: Option<Int>) -> Int {\n    match o {\n        Some(x) => x + \"a\",\n        None => 0,\n    }\n}",
    );
    assert_eq!(
        reported(&wrong),
        ["PW0609 the right side of `+`, like its left, must be `Int`, and this is `String`"]
    );
    let right = wrong.replace("x + \"a\"", "x + 1");
    assert_eq!(reported(&right), CLEAN);
}

#[test]
fn a_generic_sum_type_is_instantiated_where_it_is_used() {
    let decls = "type Maybe<T> =\n    | Nothing\n    | Just(T)\n\n";
    let built = program(&format!(
        "{decls}fn f() -> Maybe<String> {{ Maybe.Just(1) }}"
    ));
    assert_eq!(
        reported(&built),
        ["PW0606 `t.f` declares its result `t.Maybe<String>` and this produces `t.Maybe<Int>`"]
    );
    let bound = program(&format!(
        "{decls}fn f(m: Maybe<String>) -> Int {{\n    match m {{\n        Just(x) => x + 1,\n        \
         Nothing => 0,\n    }}\n}}"
    ));
    // Both sides, as ADR-0043 reports a `String` added to an `Int`.
    assert_eq!(
        reported(&bound),
        [
            "PW0609 the left side of `+` must be `Int or Float`, and this is `String`",
            "PW0609 the right side of `+`, like its left, must be `String`, and this is `Int`"
        ]
    );
    // Control.
    let right = program(&format!(
        "{decls}fn f(m: Maybe<Int>) -> Maybe<Int> {{\n    match m {{\n        Just(x) => Maybe.Just(x + 1),\n        \
         Nothing => Maybe.Nothing,\n    }}\n}}"
    ));
    assert_eq!(reported(&right), CLEAN);
}

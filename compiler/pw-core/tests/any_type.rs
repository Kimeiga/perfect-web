//! **A value that holds at every type** (ADR-0065).
//!
//! `None`, `[]`, `Err(e)`'s success, `todo`, and a generic value whose
//! parameter nothing fixes (`Maybe.Nothing`, `Secret("")`) each have a part
//! no declaration states. Until 2026-09-26 that part was a hole the value
//! relations could not decide against, so `fn f() -> Option<Int> { None }`
//! was undecided, and so was every relation through such a value: 20 of
//! kiokun's relations. Such a part holds at every type, and agrees with any.
//! A `let mut` stays one type: `let mut xs = []` then `xs = [1]` makes `xs` a
//! `List<Int>`, not a list of everything. Each test states one case, with a
//! control that a wrong program is still refused.

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

/// Every value relation over `t.pw`, as `kind outcome`, and its diagnostics.
fn relations(src: &str) -> (Vec<String>, Vec<String>) {
    let mut all = files(&["packages/pw-std"]);
    all.push(("t.pw".to_string(), src.to_string()));
    let units: Vec<pw_core::check::Unit> = all
        .iter()
        .map(|(p, s)| pw_core::check::Unit {
            path: p.clone(),
            src: s.clone(),
            hir: pw_core::lower::lower_file(s, &pw_syntax::parse_tree(s).green),
        })
        .collect();
    let rels = pw_core::values::analysis(&units)
        .into_iter()
        .filter(|(p, _)| p == "t.pw")
        .flat_map(|(_, rs)| rs)
        .filter(|r| r.kind != pw_core::values::RelationKind::Annotation)
        .map(|r| format!("{:?} {:?}", r.kind, r.outcome))
        .collect();
    let diags = check_sources(&all)
        .into_iter()
        .filter(|(n, _)| n == "t.pw")
        .flat_map(|(_, ds)| ds.into_iter().map(|d| format!("{} {}", d.code, d.message)))
        .collect();
    (rels, diags)
}

fn program(decls: &str) -> String {
    format!("module t\n\nimport List\n\n{decls}\n")
}

/// Every relation `src` has agrees, and there is at least one.
fn agrees(src: &str) {
    let (rels, diags) = relations(src);
    assert!(!rels.is_empty(), "no relation");
    assert!(rels.iter().all(|r| r.ends_with("Agree")), "{rels:#?}");
    assert!(diags.is_empty(), "{diags:#?}");
}

fn refused(src: &str, code: &str) {
    let (_, diags) = relations(src);
    assert!(!diags.is_empty(), "nothing reported");
    assert!(diags.iter().all(|d| d.starts_with(code)), "{diags:#?}");
}

#[test]
fn none_is_an_option_of_anything() {
    agrees(&program("fn f() -> Option<Int> { None }"));
    agrees(&program(
        "fn f(n: Int) -> Option<Int> {\n    let best: Option<Int> = None\n    best\n}",
    ));
    refused(&program("fn f() -> Int { None }"), "PW0606");
}

#[test]
fn an_empty_list_is_a_list_of_anything() {
    agrees(&program("fn f() -> List<String> { [] }"));
    refused(&program("fn f() -> Int { [] }"), "PW0606");
    // Through a generic call whose `T` only the empty list meets.
    agrees(&program("fn f() -> Option<Int> { List.get([], 0) }"));
    // And its element is any number an operator takes.
    agrees(&program(
        "fn f() -> Int { List.fold([], 0, (t, x) => x + t) }",
    ));
}

#[test]
fn a_failure_is_a_result_of_any_success() {
    agrees(&program(
        "type Oops = Oops { why: String }\n\nfn f() -> Result<Int, Oops> { Err(Oops { why: \"no\" }) }",
    ));
    // The failure's own type is still checked.
    refused(
        &program("type Oops = Oops { why: String }\n\nfn f() -> Result<Int, Oops> { Err(1) }"),
        "PW0606",
    );
}

#[test]
fn todo_is_any_value() {
    agrees(&program("fn f() -> Int { todo }"));
}

#[test]
fn a_generic_case_nothing_fixes_is_any_instance() {
    agrees(&program(
        "type Maybe<T> =\n    | Nothing\n    | Just(T)\n\nfn f() -> Maybe<Int> { Maybe.Nothing }",
    ));
    refused(
        &program(
            "type Maybe<T> =\n    | Nothing\n    | Just(T)\n\nfn f() -> Int { Maybe.Nothing }",
        ),
        "PW0606",
    );
}

#[test]
fn a_mutable_binding_is_one_type() {
    // `xs` holds what is assigned to it: a list of everything would let a
    // `List<Int>` be read as a `List<String>`.
    refused(
        &program("fn f(n: Int) -> List<String> {\n    let mut xs = []\n    xs = [n]\n    xs\n}"),
        "PW0606",
    );
    agrees(&program(
        "fn f(n: Int) -> List<Int> {\n    let mut xs = []\n    xs = [n]\n    xs\n}",
    ));
}

#[test]
fn a_value_of_any_type_fixes_no_variable() {
    // `concat`'s `T` is the `String` the second list fixes, whichever list
    // comes first: `[]` agrees with it and does not decide it.
    agrees(&program(
        "fn f() -> List<String> { List.concat([], [\"a\"]) }",
    ));
    refused(
        &program("fn f() -> List<Int> { List.concat([], [\"a\"]) }"),
        "PW0606",
    );
    refused(
        &program("fn f() -> List<Int> { List.concat([\"a\"], []) }"),
        "PW0606",
    );
}

#[test]
fn a_case_passed_as_a_function_keeps_its_link() {
    // `Maybe.Just` is a function from a `T` to a `Maybe<T>`: its `T` is not
    // any type, or `List.map([1], Maybe.Just)` would agree with a list of
    // `Maybe<String>`.
    let (rels, diags) = relations(&program(
        "type Maybe<T> =\n    | Nothing\n    | Just(T)\n\nfn f() -> List<Maybe<String>> { List.map([1], Maybe.Just) }",
    ));
    assert!(
        diags.is_empty() || diags.iter().all(|d| d.starts_with("PW0606")),
        "{diags:#?}"
    );
    assert!(
        !rels
            .iter()
            .any(|r| r.starts_with("Return") && r.ends_with("Agree")),
        "{rels:#?}"
    );
}

#[test]
fn a_parameter_no_field_mentions_is_any_type() {
    let either = "type Either<A, B> =\n    | Left(A)\n    | Right(B)\n\n";
    agrees(&program(&format!(
        "{either}fn f() -> Either<Int, String> {{ Either.Left(1) }}"
    )));
    refused(
        &program(&format!(
            "{either}fn f() -> Either<Int, String> {{ Either.Left(\"x\") }}"
        )),
        "PW0606",
    );
    // A record built by its fields' names, and a phantom parameter of an
    // opaque type, which its representation does not mention.
    agrees(&program(
        "type Box<T, U> = Box { value: T }\n\nfn f() -> Box<Int, String> { Box { value: 1 } }",
    ));
    agrees(&program(
        "opaque type Tag<C> = String\n\nfn f() -> Tag<Int> { Tag(\"a\") }",
    ));
}

#[test]
fn an_early_return_is_any_success() {
    agrees(&program(
        "type Oops = Oops { why: String }\n\nfn f(r: Result<Int, Oops>) -> Result<String, Oops> {\n    let n = r?\n    Ok(\"{n}\")\n}",
    ));
    // The failure it returns is still checked.
    refused(
        &program(
            "type Oops = Oops { why: String }\n\ntype Other = Other { why: String }\n\nfn f(r: Result<Int, Other>) -> Result<String, Oops> {\n    let n = r?\n    Ok(\"{n}\")\n}",
        ),
        "PW0606",
    );
}

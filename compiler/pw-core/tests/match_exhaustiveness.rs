//! **Every match is checked for exhaustiveness, and each pattern is read
//! against the type it matches.**
//!
//! Until 2026-09-25, each of these made a match read as exhaustive when it
//! was not:
//! - the analysis read only a scrutinee that was a parameter annotated with a
//!   sum type the program declares, so `match Entries.get(word) { Some(e) => .. }`,
//!   with its `None` arm missing, passed `pw check`;
//! - every pattern, at any depth, was read against the scrutinee's
//!   constructors, and one it did not find there read as a wildcard;
//! - a parameter's type was read for a name an arm had bound again;
//! - `=> return e` parsed as an arm ending at `return`, then a phantom arm
//!   whose pattern was `e`.
//!
//! ADR-0011 requires `pw` to reject an incomplete match "regardless of the
//! function's effect row", because Koka accepts one in a function that may
//! raise `exn`, and it fails at runtime. The kiokun slice's backend refused a
//! match the checker had passed, which is how the first was found. The others
//! were found while fixing it.

use pw_core::check::{MatchOutcome, Unit, check_sources, match_analysis};
use pw_core::diagnostics::Diagnostic;

fn library() -> Vec<(String, String)> {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut out = Vec::new();
    for dir in ["packages/pw-std", "packages/pw-platform-web"] {
        let mut ps: Vec<std::path::PathBuf> = std::fs::read_dir(root.join(dir))
            .unwrap()
            .map(|e| e.unwrap().path())
            .filter(|p| p.extension().is_some_and(|x| x == "pw"))
            .collect();
        ps.sort();
        for p in ps {
            out.push((
                p.display().to_string(),
                std::fs::read_to_string(&p).unwrap(),
            ));
        }
    }
    out
}

const PRELUDE: &str = "module m\n\ntype Doc = Doc {\n    s: String,\n}\n\ntype Status =\n    | Draft\n    | Sent\n\n\
fn find(w: String) -> Option<String> !{ database.read<Doc> }\n    host \"m:d/e#find\"\n\n\
fn fetch(w: String) -> Result<String, String> !{ database.read<Doc> }\n    host \"m:d/e#fetch\"\n\n\
fn status(w: String) -> Status !{ database.read<Doc> }\n    host \"m:d/e#status\"\n\n\
type Colour =\n    | Red\n    | Green\n\ntype Shape =\n    | Circle(Status)\n    | Square\n\n\
fn paint(w: String) -> Option<Colour> !{ database.read<Doc> }\n    host \"m:d/e#paint\"\n\n";

fn diagnostics(body: &str) -> Vec<Diagnostic> {
    let mut files = library();
    files.push(("m.pw".to_string(), format!("{PRELUDE}{body}")));
    check_sources(&files)
        .into_iter()
        .filter(|(p, _)| p == "m.pw")
        .flat_map(|(_, ds)| ds)
        .collect()
}

fn refused(body: &str) -> Diagnostic {
    let ds = diagnostics(body);
    let found: Vec<&Diagnostic> = ds.iter().filter(|d| d.code == "PW0305").collect();
    assert_eq!(found.len(), 1, "one PW0305 for:\n{body}\ngot {ds:#?}");
    println!(
        "{}  [{}]",
        found[0].message, found[0].repairs[0].description
    );
    found[0].clone()
}

/// Every diagnostic, as `code message`.
fn reported(body: &str) -> Vec<String> {
    diagnostics(body)
        .iter()
        .map(|d| format!("{} {}", d.code, d.message))
        .collect()
}

/// What the match audit concluded for each match in `decl`.
fn outcomes(body: &str, decl: &str) -> Vec<MatchOutcome> {
    let mut files = library();
    files.push(("m.pw".to_string(), format!("{PRELUDE}{body}")));
    let units: Vec<Unit> = files
        .iter()
        .map(|(p, s)| Unit {
            path: p.clone(),
            hir: pw_core::lower::lower_file(s, &pw_syntax::parse_tree(s).green),
            src: s.clone(),
        })
        .collect();
    match_analysis(&units)
        .into_iter()
        .filter(|a| a.declaration == decl)
        .map(|a| a.outcome)
        .collect()
}

fn blocked_because(outcome: &MatchOutcome, why: &str) -> bool {
    matches!(outcome, MatchOutcome::Blocked { reason } if reason.contains(why))
}

fn accepted(body: &str) {
    let ds = diagnostics(body);
    assert!(
        ds.iter().all(|d| d.code != "PW0305"),
        "no PW0305 for:\n{body}\ngot {ds:#?}"
    );
}

#[test]
fn an_option_match_without_none_is_refused() {
    let d = refused(
        "public query Q(w: String) -> Option<String> {\n    match find(w) {\n        Some(x) => Some(x),\n    }\n}\n",
    );
    assert!(d.message.contains("Option"), "{}", d.message);
    assert_eq!(d.repairs[0].description, "add an arm for `None`");
}

#[test]
fn a_result_match_without_err_is_refused() {
    let d = refused(
        "public query Q(w: String) -> Option<String> {\n    match fetch(w) {\n        Ok(x) => Some(x),\n    }\n}\n",
    );
    assert_eq!(d.repairs[0].description, "add an arm for `Err(_)`");
}

#[test]
fn a_call_returning_a_declared_sum_type_is_checked() {
    // The declared type was always checkable; the CALL was not, because the
    // analysis read only a bare name with an annotation.
    let d = refused(
        "public query Q(w: String) -> Int {\n    match status(w) {\n        Draft => 1,\n    }\n}\n",
    );
    assert_eq!(d.repairs[0].description, "add an arm for `Sent`");
}

#[test]
fn a_declared_option_parameter_is_checked_too() {
    refused("fn f(o: Option<String>) -> Int !{} {\n    match o {\n        None => 0,\n    }\n}\n");
}

#[test]
fn exhaustive_matches_pass() {
    accepted(
        "public query Q(w: String) -> Option<String> {\n    match find(w) {\n        Some(x) => Some(x),\n        None => None,\n    }\n}\n",
    );
    accepted(
        "public query R(w: String) -> Option<String> {\n    match fetch(w) {\n        Ok(x) => Some(x),\n        Err(_) => None,\n    }\n}\n",
    );
    accepted(
        "public query S(w: String) -> Option<String> {\n    match find(w) {\n        _ => None,\n    }\n}\n",
    );
}

#[test]
fn a_pattern_nested_under_some_is_blocked_not_proven() {
    // The payload is opaque to the analysis, so it cannot say whether
    // `Some(Some(x))` covers `Some`. It says nothing, and the audit says why.
    let body = "fn f(o: Option<Option<String>>) -> Int !{} {\n    match o {\n        Some(Some(x)) => 1,\n        None => 0,\n    }\n}\n";
    accepted(body);
    let mut files = library();
    files.push(("m.pw".to_string(), format!("{PRELUDE}{body}")));
    let units: Vec<Unit> = files
        .iter()
        .map(|(p, s)| Unit {
            path: p.clone(),
            hir: pw_core::lower::lower_file(s, &pw_syntax::parse_tree(s).green),
            src: s.clone(),
        })
        .collect();
    let f: Vec<_> = match_analysis(&units)
        .into_iter()
        .filter(|a| a.declaration == "f")
        .collect();
    assert_eq!(f.len(), 1);
    assert!(
        matches!(&f[0].outcome, pw_core::check::MatchOutcome::Blocked { reason } if reason.contains("nested under a built-in variant")),
        "{:?}",
        f[0].outcome
    );
}

#[test]
fn the_kiokun_slices_matches_are_proven_now() {
    // Both of `Lookup`'s matches are over calls and over `Option`: Blocked
    // before, Proven now.
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut files = library();
    for f in ["dictionary", "Entries", "Index", "app"] {
        let p = root.join(format!("examples/kiokun/{f}.pw"));
        files.push((
            p.display().to_string(),
            std::fs::read_to_string(&p).unwrap(),
        ));
    }
    let units: Vec<Unit> = files
        .iter()
        .map(|(p, s)| Unit {
            path: p.clone(),
            hir: pw_core::lower::lower_file(s, &pw_syntax::parse_tree(s).green),
            src: s.clone(),
        })
        .collect();
    let lookup: Vec<_> = match_analysis(&units)
        .into_iter()
        .filter(|a| a.declaration == "Lookup")
        .collect();
    println!(
        "{:?}",
        lookup
            .iter()
            .map(|a| (&a.scrutinee_type, &a.outcome))
            .collect::<Vec<_>>()
    );
    assert_eq!(lookup.len(), 2);
    assert!(
        lookup
            .iter()
            .all(|a| matches!(a.outcome, pw_core::check::MatchOutcome::Proven))
    );
}

#[test]
fn a_constructor_its_type_lacks_is_refused_not_read_as_a_wildcard() {
    // Each of these was Proven: the foreign constructor read as `_`.
    let both = "public query Q(w: String) -> Int {\n    match find(w) {\n        Ok(x) => 1,\n        Err(e) => 2,\n    }\n}\n";
    assert_eq!(
        reported(both),
        [
            "PW0608 `Ok` is not a constructor of `Option<String>`",
            "PW0608 `Err` is not a constructor of `Option<String>`",
        ]
    );
    assert_eq!(
        diagnostics(both)[0].repairs[0].description,
        "`Option<String>`'s constructors are `Some`, `None`"
    );
    assert!(blocked_because(
        &outcomes(both, "Q")[0],
        "`Ok` is not a constructor"
    ));

    let one = "public query Q(w: String) -> Int {\n    match find(w) {\n        Some(x) => 1,\n        Err(e) => 2,\n    }\n}\n";
    assert_eq!(
        reported(one),
        ["PW0608 `Err` is not a constructor of `Option<String>`"]
    );

    let bogus = "public query Q(w: String) -> Int {\n    match status(w) {\n        Draft => 1,\n        Bogus(x) => 2,\n    }\n}\n";
    assert_eq!(
        reported(bogus),
        ["PW0608 `Bogus` is not a constructor of `m.Status`"]
    );

    // A bare name is a constructor where some type has one of that name, and
    // a fresh binding otherwise.
    let other = "public query Q(w: String) -> Int {\n    match status(w) {\n        Draft => 1,\n        Red => 2,\n    }\n}\n";
    assert_eq!(
        reported(other),
        ["PW0608 `Red` is not a constructor of `m.Status`"]
    );
    let binding = "public query Q(w: String) -> Int {\n    match status(w) {\n        Draft => 1,\n        other => 2,\n    }\n}\n";
    assert!(reported(binding).is_empty(), "{:?}", reported(binding));
    assert_eq!(outcomes(binding, "Q"), [MatchOutcome::Proven]);
}

#[test]
fn a_nested_pattern_is_read_against_its_fields_type() {
    // `Circle(Draft)` was read as `Circle(_)`: `Draft` was looked up among
    // `Shape`'s constructors, not found, and read as a binding.
    let d = refused(
        "fn f(s: Shape) -> Int !{} {\n    match s {\n        Circle(Draft) => 1,\n        Square => 2,\n    }\n}\n",
    );
    assert_eq!(d.repairs[0].description, "add an arm for `Circle(Sent)`");

    let complete = "fn f(s: Shape) -> Int !{} {\n    match s {\n        Circle(Draft) => 1,\n        Circle(Sent) => 3,\n        Square => 2,\n    }\n}\n";
    assert!(reported(complete).is_empty(), "{:?}", reported(complete));
    assert_eq!(outcomes(complete, "f"), [MatchOutcome::Proven]);

    let foreign = "fn f(s: Shape) -> Int !{} {\n    match s {\n        Circle(None) => 1,\n        Square => 2,\n    }\n}\n";
    assert_eq!(
        reported(foreign),
        ["PW0608 `None` is not a constructor of `Status`"]
    );
}

#[test]
fn what_the_analysis_cannot_read_is_blocked_not_proven() {
    // A literal covers one value. Read as a wildcard, `Some("a")` covered
    // every `Some`.
    let literal = "fn f(o: Option<String>) -> Int !{} {\n    match o {\n        Some(\"a\") => 1,\n        None => 0,\n    }\n}\n";
    assert!(reported(literal).is_empty(), "{:?}", reported(literal));
    assert!(blocked_because(
        &outcomes(literal, "f")[0],
        "a literal pattern"
    ));

    // `None` under `Some` is another type's constructor, where the analysis
    // does not know the payload's type.
    let nested = "fn f(o: Option<Option<String>>) -> Int !{} {\n    match o {\n        Some(None) => 1,\n        None => 0,\n    }\n}\n";
    assert!(reported(nested).is_empty(), "{:?}", reported(nested));
    assert!(blocked_because(
        &outcomes(nested, "f")[0],
        "nested under a built-in variant"
    ));
}

#[test]
fn a_name_an_arm_binds_again_is_not_read_as_the_parameter() {
    // `x` in the inner match is the arm's `Colour`, not the parameter's
    // `Status`. The parameter's type proved `match x { Red => 1 }`
    // exhaustive against `Status`, where `Red` read as a binding.
    let shadowed = "public query Q(x: Status, w: String) -> Int {\n    match paint(w) {\n        Some(x) => match x {\n            Red => 1,\n        },\n        None => 0,\n    }\n}\n";
    let found = outcomes(shadowed, "Q");
    assert_eq!(found.len(), 2);
    assert_eq!(
        found[0],
        MatchOutcome::Proven,
        "the outer match, over `Option<Colour>`"
    );
    assert!(blocked_because(&found[1], "unknown here"), "{found:?}");

    // Bound once, it is typed, and the missing case is found.
    let d = refused(
        "public query Q(w: String) -> Int {\n    match paint(w) {\n        Some(c) => match c {\n            Red => 1,\n        },\n        None => 0,\n    }\n}\n",
    );
    assert_eq!(d.repairs[0].description, "add an arm for `Green`");
}

#[test]
fn a_return_in_an_arm_is_not_a_phantom_arm() {
    // `=> return Ok(1)` parsed as `=> return`, then an arm whose pattern was
    // `Ok(1)`. That arm read as a wildcard, so this match was Proven.
    let d = refused(
        "fn f(s: Status) -> Result<Int, String> !{} {\n    match s {\n        Draft => return Ok(1)\n    }\n}\n",
    );
    assert_eq!(d.repairs[0].description, "add an arm for `Sent`");

    // And the value is returned, so the return type is checked against it.
    let wrong = "fn g(s: Status) -> Int !{} {\n    match s {\n        Draft => 1\n        Sent => return \"x\"\n    }\n}\n";
    assert!(
        reported(wrong).iter().any(|d| d.starts_with("PW0606")),
        "{:?}",
        reported(wrong)
    );
}

#[test]
fn a_field_count_is_checked_at_any_depth_and_in_every_arm() {
    // Nested: `Draft` carries no field. This was blocked, never reported.
    let nested = "fn f(s: Shape) -> Int !{} {\n    match s {\n        Circle(Draft(x)) => 1,\n        Circle(Sent) => 2,\n        Square => 3,\n    }\n}\n";
    assert_eq!(
        reported(nested),
        ["PW0603 `Draft` binds 1 field(s) but declares 0"]
    );

    // An arm the analysis cannot read does not hide another arm's defects.
    let mixed = "fn f(o: Option<String>) -> Int !{} {\n    match o {\n        Some(\"a\") => 1,\n        Some => 2,\n        Ok(x) => 3,\n        None => 0,\n    }\n}\n";
    assert_eq!(
        reported(mixed),
        [
            "PW0603 `Some` binds 0 field(s) but declares 1",
            "PW0608 `Ok` is not a constructor of `Option<String>`",
        ]
    );
    assert!(blocked_because(
        &outcomes(mixed, "f")[0],
        "`Ok` is not a constructor"
    ));
}

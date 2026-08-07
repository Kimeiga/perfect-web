//! E8 step 2 — a capability that names nothing is a build error.
//!
//! Architect ruling, 2026-08-07:
//!
//! > Deployment is far too late to tell the developer "the node doesn't provide
//! > `database.read<Stroes>`". The actual problem is: `Stroes` does not name a
//! > type. That is a source-program error with a precise source span and an
//! > obvious repair. […] Not a warning. Authority requirements are not
//! > something we should "best effort" through.
//!
//! # What this rule may NOT do
//!
//! Outlaw the convention the corpus already uses. `database.read<Stores>` names
//! a MODULE — the domain being read — and the first version of this rule
//! accepted only types, reporting `R-002` and `R-008` as defective. A rule that
//! makes correct programs fail is worse than the deployment-time message it
//! replaces, so half these tests are about what must stay legal.

use pw_core::check::check_sources;
use pw_core::diagnostics::Diagnostic;

fn check(files: &[(&str, &str)]) -> Vec<Diagnostic> {
    let owned: Vec<(String, String)> = files
        .iter()
        .map(|(p, s)| (p.to_string(), s.to_string()))
        .collect();
    check_sources(&owned)
        .into_iter()
        .flat_map(|(_, d)| d)
        .collect()
}

fn codes(files: &[(&str, &str)]) -> Vec<&'static str> {
    check(files).into_iter().map(|d| d.code).collect()
}

const DOMAIN: (&str, &str) = (
    "domain.pw",
    "module Stores\n\ntype Store = Store { id: String }\n\nopaque type StoreId = String\n",
);

#[test]
fn an_argument_that_names_nothing_is_an_error() {
    let f = (
        "t.pw",
        "module t\n\nimport Stores\n\nfn read() -> Int !{ database.read<Stroes> } { 0 }\n",
    );
    let found = check(&[DOMAIN, f]);
    let ours: Vec<&Diagnostic> = found.iter().filter(|d| d.code == "PW5200").collect();
    assert_eq!(ours.len(), 1, "{:?}", codes(&[DOMAIN, f]));

    let d = ours[0];
    assert!(d.message.contains("Stroes"), "{}", d.message);
    assert!(
        d.message.contains("did you mean `Stores`?"),
        "the repair is the point: {}",
        d.message
    );
    assert!(!d.repairs.is_empty(), "a legal alternative is offered");
    assert!(d.explanation.is_some());

    // The span points at the effect, not at the declaration.
    let src = f.1;
    assert!(
        src[d.primary_span.start..d.primary_span.end].contains("Stroes"),
        "the span covers what is wrong, got {:?}",
        &src[d.primary_span.start..d.primary_span.end]
    );
}

// --- what must stay legal ---------------------------------------------------

#[test]
fn a_module_is_a_legitimate_capability_argument() {
    // `database.read<Stores>` names the DOMAIN being read. This is what the
    // corpus writes, and the first version of this rule rejected all of it.
    let f = (
        "t.pw",
        "module t\n\nimport Stores\n\nfn read() -> Int !{ database.read<Stores> } { 0 }\n",
    );
    assert!(!codes(&[DOMAIN, f]).contains(&"PW5200"));
}

#[test]
fn a_declared_type_is_a_legitimate_capability_argument() {
    let f = (
        "t.pw",
        "module t\n\nimport Stores\n\nfn read() -> Int !{ database.read<Store> } { 0 }\n",
    );
    assert!(!codes(&[DOMAIN, f]).contains(&"PW5200"));
}

#[test]
fn an_opaque_type_is_a_legitimate_capability_argument() {
    // The ruling's own carve-out: "unless the name refers to a properly
    // declared external/opaque contract".
    let f = (
        "t.pw",
        "module t\n\nimport Stores\n\nfn read() -> Int !{ database.read<StoreId> } { 0 }\n",
    );
    assert!(!codes(&[DOMAIN, f]).contains(&"PW5200"));
}

#[test]
fn an_effect_with_no_argument_is_not_examined() {
    let f = ("t.pw", "module t\n\nfn log() -> Int !{ log } { 0 }\n");
    assert!(!codes(&[f]).contains(&"PW5200"));
}

// --- the discriminating controls -------------------------------------------
//
// The rule adopted after R-037: an invariant that depends on a chain must have
// a control that breaks one link and makes the diagnostic disappear.

#[test]
fn declaring_the_missing_name_removes_the_diagnostic() {
    // The link: argument -> declared name. Break it by ADDING the declaration
    // and the error must go, or the rule is reporting something else.
    let broken = (
        "t.pw",
        "module t\n\nimport Stores\n\nfn read() -> Int !{ database.read<Ledger> } { 0 }\n",
    );
    assert!(codes(&[DOMAIN, broken]).contains(&"PW5200"));

    let repaired = (
        "domain.pw",
        "module Stores\n\ntype Store = Store { id: String }\n\ntype Ledger = Ledger { id: String }\n",
    );
    assert!(!codes(&[repaired, broken]).contains(&"PW5200"));
}

#[test]
fn the_suggestion_is_absent_when_nothing_is_close() {
    // A suggestion that named the alphabetically first declaration would be
    // worse than none — it sends the reader to an unrelated declaration. So
    // the distance is bounded, and a name nothing resembles gets no guess.
    let f = (
        "t.pw",
        "module t\n\nimport Stores\n\nfn read() -> Int !{ database.read<Wxyzzy> } { 0 }\n",
    );
    let d = check(&[DOMAIN, f])
        .into_iter()
        .find(|d| d.code == "PW5200")
        .expect("still an error");
    assert!(
        !d.message.contains("did you mean"),
        "no guess when nothing is close: {}",
        d.message
    );
}

#[test]
fn the_rule_reads_the_whole_program_not_one_file() {
    // `database.read<Stores>` in one file names a module declared in another.
    // A per-file rule would report every real program.
    let f = (
        "t.pw",
        "module t\n\nimport Stores\n\nfn read() -> Int !{ database.read<Stores> } { 0 }\n",
    );
    assert!(
        !codes(&[DOMAIN, f]).contains(&"PW5200"),
        "the other file declares it"
    );
}

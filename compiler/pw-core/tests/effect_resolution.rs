//! E8 — effect ontology slice 2: a written effect resolves to a declaration.
//!
//! The list the architect's ruling of 2026-08-07 requires before this is
//! "landed":
//!
//! ```text
//! effect log                      parses/resolves          slice 1 ✓
//! effect database.read<T>         parses/resolves          slice 1 ✓
//! database.read<Stores>           resolves decl + argument   here
//! database.read<Stroes>           PW5200                   done
//! databse.read<Stores>            unknown effect             here
//! database.reed<Stores>           unknown effect             here
//! database.read                   wrong generic arity        here
//! database.read<A, B>             wrong generic arity        here
//! foo.read<T> vs database.read<T> never confused           slice 1 ✓
//! database.read vs database.write distinct EffectDefIds    slice 1 ✓
//! ```
//!
//! plus the structural one, which is what ADR-0022 asks of the first
//! production consumer of semantic provenance:
//!
//! > No downstream semantic analysis discovers an effect by splitting or
//! > comparing its dotted spelling.
//!
//! Asserted here as evidence rather than as a verdict: the resolution must
//! have travelled by `Route::Qualified`, and `Route::ProgramWideName` — the
//! route every instance of coincidental correctness so far has taken — must
//! not appear at all.
//!
//! # Where the structural half lives
//!
//! A behavioural test cannot prove the ABSENCE of a last-segment lookup — it
//! can only prove that the paths it happens to exercise do not use one. So the
//! structural guarantee is `tests/last_segment.rs`: it enumerates every
//! `rsplit('.')` in the crate and requires each to be classified, with the
//! `forbidden` count ratcheted at zero.
//!
//! Both halves were checked against a deliberate mutation — resolving an
//! effect by its final segment, which would make `databse.read` find
//! `database.read`. `last_segment.rs` reports the new site as unclassified;
//! and `an_effect_out_of_scope_records_no_fact..` below caught the ordering
//! defect the mutation exposed on the way, where a resolution that later
//! failed had already recorded an edge claiming it succeeded.

use pw_core::hir::{EffectRef, Hir};
use pw_core::lower::lower_file;
use pw_core::ontology::{EffectError, Ontology, Resolved, TypeArgument};
use pw_core::provenance::{Evidence, Route};
use pw_core::resolve::Workspace;
use pw_syntax::parse_tree;

/// The platform declarations these tests resolve against.
const EFFECTS: &str = "module effects\n\n\
     effect log {\n    capability none\n}\n\n\
     effect database.read<T> {\n    capability database.read<T>\n    \
     host \"pw:host/database#read\"\n}\n\n\
     effect database.write<T> {\n    capability database.write<T>\n}\n\n\
     effect cache.read {\n    capability none\n}\n";

/// A module declaring the things a type argument may name.
const DOMAIN: &str = "module Stores\n\ntype Store = Store { id: Int }\n";

/// The unit that writes the rows, importing both. Unit 2 throughout.
const USER: &str = "module app\n\nimport effects\nimport Stores\n";

fn program(sources: &[&str]) -> (Vec<Hir>, Ontology) {
    let hirs: Vec<Hir> = sources
        .iter()
        .map(|s| lower_file(s, &parse_tree(s).green))
        .collect();
    let refs: Vec<&Hir> = hirs.iter().collect();
    let ontology = Ontology::build(&refs);
    (hirs, ontology)
}

/// Resolve one written row entry as `app` — a unit that imports the effect
/// declarations and the domain, which is what a real caller looks like.
fn resolve(written: &str) -> (Result<Resolved, EffectError>, Evidence) {
    let src = user_with(written);
    let (hirs, ontology) = program(&[EFFECTS, DOMAIN, &src]);
    let refs: Vec<&Hir> = hirs.iter().collect();
    let ws = Workspace::build(&refs);
    let mut evidence = Evidence::default();
    let entry = row_entry(&hirs[2]);
    let result = ontology.resolve(&ws, 2, &entry, &mut evidence);
    (result, evidence)
}

fn user_with(written: &str) -> String {
    format!("{USER}\nfn f() -> Int !{{ {written} }} {{ 0 }}\n")
}

/// A row entry, lowered by the REAL grammar.
///
/// Not hand-built. The point of `EffectRef::args` is that the parser found the
/// arguments, and a test that constructed them itself would be asserting on
/// its own arithmetic rather than on the compiler's.
fn row_entry(hir: &Hir) -> EffectRef {
    let (_, decl) = hir.all_decls().find(|(_, d)| d.name == "f").expect("f");
    decl.declared_effects
        .as_ref()
        .expect("a row")
        .first()
        .expect("one entry")
        .clone()
}

// --- the pipeline ------------------------------------------------------------

#[test]
fn a_written_effect_resolves_to_its_declaration_and_its_argument() {
    let (result, evidence) = resolve("database.read<Stores>");
    let Ok(Resolved::Operation(instance)) = result else {
        panic!("expected an operation, got {result:?}");
    };

    let (_hirs, ontology) = program(&[EFFECTS, DOMAIN]);
    let decl = ontology
        .declaration(instance.effect)
        .expect("a declaration");
    assert_eq!(decl.path.text(), "database.read");
    assert_eq!(decl.arity, 1);
    assert_eq!(decl.host.as_deref(), Some("pw:host/database#read"));

    // `Stores` is a MODULE — the domain being read. `capability.rs` records
    // why that counts: a first version accepted only types and reported two
    // working corpus files as defective.
    assert_eq!(
        instance.args,
        vec![TypeArgument::Module {
            written: "Stores".into()
        }]
    );

    // ADR-0022. The required edge, and the forbidden source.
    assert_eq!(
        evidence.effect_resolved("database.read"),
        Some(instance.effect),
        "the resolution must be recorded, not just returned"
    );
    assert!(evidence.routes().contains(&Route::Qualified));
    assert!(
        !evidence.routes().contains(&Route::ProgramWideName),
        "an effect must never be found by matching a spelling against the program"
    );
}

#[test]
fn a_type_argument_that_names_a_declared_type_resolves_to_it() {
    let (result, _) = resolve("database.read<Store>");
    let Ok(Resolved::Operation(instance)) = result else {
        panic!("expected an operation, got {result:?}");
    };
    assert!(
        matches!(&instance.args[0], TypeArgument::Type { written, .. } if written == "Store"),
        "{:?}",
        instance.args
    );
}

#[test]
fn an_effect_with_no_argument_resolves_with_no_argument() {
    let (result, _) = resolve("log");
    let Ok(Resolved::Operation(instance)) = result else {
        panic!("expected an operation, got {result:?}");
    };
    assert!(instance.args.is_empty());
}

#[test]
fn a_bare_family_resolves_to_the_family_and_not_to_an_operation() {
    // `!{ database }` is legitimate: declaring a family accepts its members.
    // It is not an effect anything performs, and the two must not collapse —
    // a family standing in for an operation would grant `database.write` to
    // anything that declared `database`.
    let (result, _) = resolve("database");
    assert!(
        matches!(&result, Ok(Resolved::Family { name, .. }) if name == "database"),
        "{result:?}"
    );
}

// --- the failures ------------------------------------------------------------

#[test]
fn a_misspelled_family_is_unknown_and_says_which_half_is_wrong() {
    let (result, _) = resolve("databse.read<Stores>");
    let Err(EffectError::UnknownFamily {
        family, nearest, ..
    }) = result
    else {
        panic!("expected an unknown family, got {result:?}");
    };
    assert_eq!(family, "databse");
    assert_eq!(nearest.as_deref(), Some("database"));
}

#[test]
fn a_misspelled_operation_is_unknown_and_lists_what_the_family_declares() {
    // The distinction that needs the declarations to exist. `databse.read` and
    // `database.reed` are one typo each and send a reader to two different
    // places; "unknown effect" would send them to neither.
    let (result, _) = resolve("database.reed<Stores>");
    let Err(EffectError::UnknownOperation {
        family,
        operation,
        declared,
        nearest,
        ..
    }) = result
    else {
        panic!("expected an unknown operation, got {result:?}");
    };
    assert_eq!(family, "database");
    assert_eq!(operation, "reed");
    assert_eq!(nearest.as_deref(), Some("read"));
    assert_eq!(declared, vec!["read".to_string(), "write".to_string()]);
}

#[test]
fn an_effect_written_with_no_argument_where_one_is_declared_is_wrong_arity() {
    let (result, _) = resolve("database.read");
    assert!(
        matches!(
            result,
            Err(EffectError::WrongArity {
                expected: 1,
                found: 0,
                ..
            })
        ),
        "{result:?}"
    );
}

#[test]
fn an_effect_written_with_two_arguments_where_one_is_declared_is_wrong_arity() {
    let (result, _) = resolve("database.read<Store, Stores>");
    assert!(
        matches!(
            result,
            Err(EffectError::WrongArity {
                expected: 1,
                found: 2,
                ..
            })
        ),
        "{result:?}"
    );
}

#[test]
fn an_effect_declaring_no_parameters_written_with_one_is_wrong_arity() {
    // The other direction. Without it, "wrong arity" could mean "fewer than
    // declared" and an effect given an argument it does not take would pass.
    let (result, _) = resolve("log<Public>");
    assert!(
        matches!(
            result,
            Err(EffectError::WrongArity {
                expected: 0,
                found: 1,
                ..
            })
        ),
        "{result:?}"
    );
}

// --- what a failure must NOT do ---------------------------------------------

#[test]
fn a_failed_resolution_records_no_effect_fact() {
    // The control on the provenance itself. If an unresolvable effect still
    // recorded a resolution, every assertion about a chain above would be
    // measuring nothing — a graph that always contains the edge cannot be
    // used to prove the edge was travelled.
    let (result, evidence) = resolve("databse.read<Stores>");
    assert!(result.is_err());
    assert_eq!(evidence.effect_resolved("databse.read"), None);
    assert!(evidence.routes().is_empty(), "{:?}", evidence.routes());
}

#[test]
fn an_effect_out_of_scope_records_no_fact_and_a_wrong_arity_one_records_the_name() {
    // Two failures, two different answers, and the difference is the point.
    //
    // Out of scope: the path named nothing THIS UNIT can reach, so there is no
    // resolution to record. This is the ordering the last-segment mutation
    // exposed — recorded before the visibility check, a failed resolution left
    // an edge behind claiming it had succeeded.
    let src = "module app\n\nfn f() -> Int !{ database.read<Stores> } { 0 }\n";
    let (hirs, ontology) = program(&[EFFECTS, DOMAIN, src]);
    let refs: Vec<&Hir> = hirs.iter().collect();
    let ws = Workspace::build(&refs);
    let mut evidence = Evidence::default();
    let result = ontology.resolve(&ws, 2, &row_entry(&hirs[2]), &mut evidence);
    assert!(result.is_err(), "{result:?}");
    assert_eq!(evidence.effect_resolved("database.read"), None);

    // Wrong arity: the path DID name a declaration, and a separate rule
    // rejected how it was written. Recording the resolution is the truth —
    // the name resolved, the use is wrong — and a diagnostic that wants to
    // point at the declaration needs it.
    let (result, evidence) = resolve("database.read");
    assert!(matches!(result, Err(EffectError::WrongArity { .. })));
    assert!(
        evidence.effect_resolved("database.read").is_some(),
        "the name resolved; only the argument count is wrong"
    );
}

#[test]
fn two_operations_of_one_family_get_distinct_identities() {
    // `docs/RISK_QUEUE.md` 34's shape at the ontology layer, asserted on the
    // resolved identity rather than on the spelling.
    let read = resolve("database.read<Stores>").0;
    let write = resolve("database.write<Stores>").0;
    let (Ok(Resolved::Operation(r)), Ok(Resolved::Operation(w))) = (read, write) else {
        panic!("both must resolve");
    };
    assert_ne!(r.effect, w.effect);
}

#[test]
fn two_families_sharing_a_final_segment_are_never_confused() {
    // `database.read` and `cache.read`. A resolver that matched the last
    // segment would give one the other's declaration — and `cache.read`
    // declares `capability none` while `database.read` declares real host
    // authority, so the confusion would be an authority mistake.
    let (a, _) = resolve("database.read<Stores>");
    let (b, _) = resolve("cache.read");
    let (Ok(Resolved::Operation(a)), Ok(Resolved::Operation(b))) = (a, b) else {
        panic!("both must resolve");
    };
    assert_ne!(a.effect, b.effect);

    let (_hirs, ontology) = program(&[EFFECTS, DOMAIN]);
    assert!(ontology.declaration(a.effect).unwrap().host.is_some());
    assert!(ontology.declaration(b.effect).unwrap().host.is_none());
}

// --- visibility --------------------------------------------------------------

#[test]
fn an_effect_the_unit_does_not_import_is_out_of_scope() {
    // Assumption A-009. "This program declares it somewhere" is the ambient
    // union E2B removed, and an effect is not exempt from it.
    let user = "module app\n\nfn f() -> Int !{ database.read<Stores> } { 0 }\n";
    let (hirs, ontology) = program(&[EFFECTS, DOMAIN, user]);
    let refs: Vec<&Hir> = hirs.iter().collect();
    let ws = Workspace::build(&refs);
    let entry = row_entry(&hirs[2]);

    let mut evidence = Evidence::default();
    let unimported = ontology.resolve(&ws, 2, &entry, &mut evidence);
    assert!(unimported.is_err(), "`app` imports nothing: {unimported:?}");

    // The neighbour: the same row, in a unit that imports the declarations.
    // Without it, "is_err" would also be satisfied by a resolver that never
    // resolves anything.
    let imported = resolve("database.read<Stores>").0;
    assert!(
        matches!(imported, Ok(Resolved::Operation(_))),
        "importing `effects` brings the effect into scope: {imported:?}"
    );
}

#[test]
fn an_argument_the_unit_cannot_see_does_not_resolve() {
    // Architect ruling, 2026-08-07: effect names come from the prelude, their
    // arguments do not.
    //
    // > If `Payments` isn't imported or otherwise in scope, that's a source
    // > error.
    //
    // This was `TypeArgument::Unscoped` — "the program declares it, this unit
    // does not import it" — kept because 31 corpus rows relied on it. All 31
    // were repaired, and A-017 was retired the day after it was written. What
    // remains is the two-way answer: resolved from here, or not resolved.
    let src = "module app\n\nimport effects\n\nfn f() -> Int !{ database.read<Store> } { 0 }\n";
    let (hirs, ontology) = program(&[EFFECTS, DOMAIN, src]);
    let refs: Vec<&Hir> = hirs.iter().collect();
    let ws = Workspace::build(&refs);
    let mut evidence = Evidence::default();
    let result = ontology.resolve(&ws, 2, &row_entry(&hirs[2]), &mut evidence);

    let Ok(Resolved::Operation(instance)) = result else {
        panic!("the effect itself resolves through the prelude: {result:?}");
    };
    assert_eq!(
        instance.args,
        vec![TypeArgument::Unresolved {
            written: "Store".into()
        }],
        "`Store` is declared in `Stores`, which this file does not import"
    );
    assert!(!instance.args[0].is_resolved());

    // The neighbour, so this is measuring visibility rather than a resolver
    // that never resolves anything.
    let (ok, _) = resolve("database.read<Store>");
    let Ok(Resolved::Operation(ok)) = ok else {
        panic!("importing `Stores` resolves it");
    };
    assert!(ok.args[0].is_resolved());
}

// --- the ontology itself -----------------------------------------------------

#[test]
fn a_family_exists_because_something_was_declared_into_it() {
    let (_hirs, ontology) = program(&[EFFECTS, DOMAIN]);
    assert!(ontology.has_family("database"));
    assert!(ontology.has_family("cache"));
    assert!(ontology.has_family("log"));
    assert!(
        !ontology.has_family("databse"),
        "no checker holds a list of valid family spellings"
    );

    // The control: with no declarations there are no families, so the checks
    // above are measuring the declarations rather than a built-in table.
    let (_hirs, empty) = program(&[DOMAIN]);
    assert!(empty.is_empty());
    assert!(!empty.has_family("database"));
}

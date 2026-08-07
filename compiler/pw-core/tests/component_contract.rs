//! E8-0 — the compiler→host contract, and the one rule that governs it.
//!
//! Architect ruling, 2026-08-07:
//!
//! > Freeze the compiler→host `ComponentContract` […] Rule: actual Wasm
//! > imports ⊆ statically allowed set, never a superset. E8 consumes
//! > capabilities; it does not define effect semantics.
//!
//! and the principle the whole milestone hangs on:
//!
//! > The compiler decides what authority code needs. The host decides whether
//! > that authority physically exists. Neither should reconstruct the other's
//! > answer.
//!
//! # What each test is defending against
//!
//! Every one of these has a plausible implementation that passes a naive test.
//! A contract derived from *declared* placements rather than solved ones looks
//! identical until a declaration is wrong. A capability that drops its type
//! argument looks identical until two arguments exist. An audit that reports
//! "satisfied" for an unparseable import looks identical until one appears.
//! So each test names the thing that would still work.

use pw_core::contract::{Audit, Capability, ComponentContract, audit, contracts};
use pw_core::hir::Hir;
use pw_core::lower::lower_file;
use pw_core::resolve::Workspace;
use pw_core::signatures::Signatures;
use pw_syntax::parse_tree;

mod support;

/// A program with three components whose authority differs in every dimension
/// that matters: family, operation, type argument, and none at all.
const PROGRAM: &str = "\
module shop.origin

fn read_stores() -> Int !{ database.read<Stores> } { 0 }
fn write_stores() -> Int !{ database.write<Stores> } { 0 }

public query Menu(id: Int) -> Int
    freshness   5.minutes
    consistency snapshot
    cache       shared
    key         id
{
    read_stores()
}

command Restock(id: Int) -> Int
    requires SignedIn
{
    write_stores()
}
";

const BROWSER_ONLY: &str = "\
module shop.view

fn paint(x: Int) -> Int !{ dom.mutate } { x }

component Badge() {
    paint(1)
}
";

const PURE: &str = "\
module shop.pure

component Label() {
    1
}
";

fn build(sources: &[&str]) -> Vec<ComponentContract> {
    // The effect vocabulary, selected the way a real program selects one.
    // Since `World::worlds_for` was deleted an effect nothing declares has no
    // capability and no placement, so a fixture that performs `database.read`
    // has to say what `database.read` is — see `tests/support/mod.rs`.
    let hirs: Vec<Hir> = std::iter::once(&support::VOCABULARY)
        .chain(sources.iter())
        .map(|s| lower_file(s, &parse_tree(s).green))
        .collect();
    let refs: Vec<&Hir> = hirs.iter().collect();
    let ws = Workspace::build(&refs);
    let sigs = Signatures::build(&ws, &refs);
    contracts(&refs, &sigs, &ws)
}

fn one(sources: &[&str], module: &str) -> ComponentContract {
    build(sources)
        .into_iter()
        .find(|c| c.component_id == module)
        .unwrap_or_else(|| panic!("no contract for {module}"))
}

#[test]
fn a_contract_carries_the_six_semantic_fields_and_no_others() {
    // Frozen means frozen. Serialized and read back as a map, so a seventh
    // SEMANTIC field added later fails here rather than being discovered by a
    // host that silently ignores it.
    //
    // `capability_mapping` is not a seventh: it says which mapping produced the
    // other six, so a host can tell a representation change from an authority
    // change. A contract carrying a mapping version it does not know must be
    // refused rather than interpreted under its own.
    let c = one(&[PROGRAM], "shop.origin.Menu");
    let json = serde_json::to_value(&c).expect("serialize");
    let mut keys: Vec<&str> = json
        .as_object()
        .expect("object")
        .keys()
        .map(|k| k.as_str())
        .collect();
    keys.sort();
    assert_eq!(
        keys,
        [
            "abi_schema",
            "allowed_placements",
            "capability_mapping",
            "component_id",
            "exports",
            "imports",
            "required_capabilities",
        ]
    );
    assert_eq!(c.capability_mapping, pw_core::contract::CAPABILITY_MAPPING);
}

#[test]
fn a_capabilitys_argument_is_resolved_against_the_programs_types() {
    use pw_core::contract::{Capability as Cap, NotACapability};
    use pw_core::ontology::Ontology;
    use std::collections::BTreeSet;

    let declared: BTreeSet<String> = ["Stores".to_string()].into_iter().collect();
    let hir = lower_file(support::VOCABULARY, &parse_tree(support::VOCABULARY).green);
    let ws = Workspace::build(&[&hir]);
    let ontology = Ontology::build_with(&[&hir], &ws);

    // Resolved.
    assert_eq!(
        Cap::resolve("database.read<Stores>", &declared, &ontology)
            .expect("resolves")
            .name(),
        "database.read<Stores>"
    );

    // An effect declared with `capability none` needs no host authority.
    assert_eq!(
        Cap::resolve("trace", &declared, &ontology),
        Err(NotACapability::Unrestricted {
            family: "trace".into()
        })
    );

    // A misspelled argument is REPORTED, not accepted silently. Without this,
    // `database.read<Stroes>` becomes a capability nothing will ever grant and
    // the failure appears at deployment rather than at build.
    assert_eq!(
        Cap::resolve("database.read<Stroes>", &declared, &ontology),
        Err(NotACapability::UnknownArgument {
            family: "database".into(),
            argument: "Stroes".into()
        })
    );

    // And an effect the program never declared is neither of those. It used to
    // be `Unrestricted` whenever the deleted `worlds_for` table had no row for
    // its family, which is the same answer `trace` gets above — so a typo and
    // a deliberately authority-free effect were indistinguishable.
    assert_eq!(
        Cap::resolve("databse.read<Stores>", &declared, &ontology),
        Err(NotACapability::Undeclared {
            effect: "databse.read<Stores>".into()
        })
    );
}

#[test]
fn an_unresolvable_argument_does_not_remove_authority() {
    // The direction that matters. Over-stating a capability is refused work;
    // under-stating it is authority nobody approved — so a capability whose
    // argument does not resolve is KEPT, and the mistake surfaces as a refusal
    // rather than as a silent grant.
    let program = "\
module shop.typo

fn read() -> Int !{ database.read<Stroes> } { 0 }

public query Menu(id: Int) -> Int
    freshness   5.minutes
    consistency snapshot
    cache       shared
    key         id
{
    read()
}
";
    let c = one(&[program], "shop.typo.Menu");
    assert_eq!(
        c.required_capabilities
            .iter()
            .map(Capability::name)
            .collect::<Vec<_>>(),
        ["database.read<Stroes>"],
        "the capability survives even though nothing will grant it"
    );
}

#[test]
fn required_capabilities_come_from_the_effect_row() {
    // Per DECLARATION, so the query that reads does not also get the write the
    // command in the same module needs. Aggregating by module would give both
    // both — over-granting, and then E5's solver would place neither, because
    // the union of two demands can be unsatisfiable when each alone is not.
    let menu = one(&[PROGRAM], "shop.origin.Menu");
    let restock = one(&[PROGRAM], "shop.origin.Restock");
    let names = |c: &ComponentContract| -> Vec<String> {
        c.required_capabilities
            .iter()
            .map(Capability::name)
            .collect()
    };
    assert_eq!(names(&menu), ["database.read<Stores>"]);
    assert_eq!(names(&restock), ["database.write<Stores>"]);
}

#[test]
fn a_component_with_no_effects_needs_no_capabilities_and_may_import_nothing() {
    // The boundary case that makes the rule bite hardest: an empty allowed set
    // means EVERY import is undeclared. A contract that treated "no
    // capabilities" as "no constraint" would give the most trusted-looking
    // component unlimited authority.
    let c = one(&[PURE], "shop.pure.Label");
    assert!(c.required_capabilities.is_empty());
    assert!(c.imports.is_empty());
    assert_eq!(
        audit(&c, &["pw:host/database#read".into()]),
        Audit::Undeclared(vec!["pw:host/database#read".into()]),
    );
    assert_eq!(audit(&c, &[]), Audit::Satisfied);
}

#[test]
fn allowed_placements_are_solved_rather_than_declared() {
    // E5's solver, not a table here. A component that reads the database can
    // only run at the origin; one that mutates the DOM only in the browser;
    // one that does neither can run anywhere.
    assert_eq!(
        one(&[PROGRAM], "shop.origin.Menu").allowed_placements,
        ["origin"]
    );
    assert_eq!(
        one(&[BROWSER_ONLY], "shop.view.Badge").allowed_placements,
        ["browser"]
    );
    assert_eq!(
        one(&[PURE], "shop.pure.Label").allowed_placements,
        ["build", "browser", "edge", "origin"],
        "a component with no restricted capability runs anywhere"
    );
}

#[test]
fn placement_is_not_a_capability() {
    // The architect's distinction, as a fact about the data. Two components
    // with the SAME single placement need different capabilities, and two with
    // the same capability family need the same placement — so neither field is
    // recoverable from the other, and a host that inferred one from the other
    // would be wrong in one of these two cases.
    let reader = one(&[PROGRAM], "shop.origin.Menu");
    let writer = one(&[PROGRAM], "shop.origin.Restock");
    let browser = one(&[BROWSER_ONLY], "shop.view.Badge");

    assert_eq!(reader.allowed_placements, ["origin"]);
    assert_eq!(browser.allowed_placements, ["browser"]);
    assert_ne!(reader.required_capabilities, browser.required_capabilities);

    // And the reverse direction: `database.read` and `database.write` are
    // different capabilities at the SAME placement, so a host cannot infer one
    // field from the other in either direction.
    assert_eq!(reader.allowed_placements, writer.allowed_placements);
    assert_ne!(reader.required_capabilities, writer.required_capabilities);
    assert_eq!(
        reader.required_capabilities[0].family,
        writer.required_capabilities[0].family
    );
}

// --- the rule ------------------------------------------------------------

#[test]
fn actual_imports_may_be_a_subset() {
    // Fewer is fine: dead code, or a branch never compiled in.
    let c = one(&[PROGRAM], "shop.origin.Menu");
    assert_eq!(c.imports.len(), 1);
    assert_eq!(audit(&c, &[c.imports[0].key()]), Audit::Satisfied);
    assert_eq!(audit(&c, &[]), Audit::Satisfied);
}

#[test]
fn actual_imports_may_never_be_a_superset() {
    let c = one(&[PROGRAM], "shop.origin.Menu");
    let mut actual: Vec<String> = c.imports.iter().map(|i| i.key()).collect();
    actual.push("pw:host/secret#read".into());

    assert_eq!(
        audit(&c, &actual),
        Audit::Undeclared(vec!["pw:host/secret#read".into()]),
        "one import outside the allowed set is a refusal, not a warning"
    );
}

#[test]
fn every_undeclared_import_is_reported_not_only_the_first() {
    // A build that fixes the first and rediscovers the second is a slow way to
    // learn the shape of a problem.
    let c = one(&[PURE], "shop.pure.Label");
    let actual = vec![
        "pw:host/secret#read".to_string(),
        "pw:host/database#write".to_string(),
    ];
    assert_eq!(
        audit(&c, &actual),
        Audit::Undeclared(vec![
            "pw:host/database#write".into(),
            "pw:host/secret#read".into(),
        ]),
    );
}

#[test]
fn an_unrecognisable_import_is_undeclared_rather_than_ignored() {
    // Fail-closed. "I could not tell what this asks for" and "this asks for
    // nothing" must never produce the same verdict — the audit is membership
    // in a set, so there is no parse to fail and no unknown-interface branch
    // to forget.
    let c = one(&[PROGRAM], "shop.origin.Menu");
    for weird in ["", "not-an-interface", "pw:host/database", "#read", "\u{0}"] {
        assert!(
            !audit(&c, &[weird.to_string()]).is_satisfied(),
            "`{weird}` must not be treated as satisfied"
        );
    }
}

#[test]
fn the_operation_distinguishes_two_imports_of_one_family() {
    // A host that granted `pw:host/database` wholesale would satisfy a
    // component that only ever asked to read.
    let read = one(&[PROGRAM], "shop.origin.Menu");
    let write = one(&[PROGRAM], "shop.origin.Restock");
    assert_eq!(
        read.imports.iter().map(|i| i.key()).collect::<Vec<_>>(),
        ["pw:host/database#read"]
    );
    assert_eq!(
        write.imports.iter().map(|i| i.key()).collect::<Vec<_>>(),
        ["pw:host/database#write"]
    );
    assert_eq!(
        audit(&read, &["pw:host/database#write".into()]),
        Audit::Undeclared(vec!["pw:host/database#write".into()]),
        "reading does not authorise writing, however close the two look"
    );

    // A read-only component may not import write.
    let read_only = "\
module shop.readonly

fn read_stores() -> Int !{ database.read<Stores> } { 0 }

public query Menu(id: Int) -> Int
    freshness   5.minutes
    consistency snapshot
    cache       shared
    key         id
{
    read_stores()
}
";
    let ro = one(&[read_only], "shop.readonly.Menu");
    assert_eq!(
        audit(&ro, &["pw:host/database#write".into()]),
        Audit::Undeclared(vec!["pw:host/database#write".into()]),
    );
}

#[test]
fn an_import_names_the_capability_that_authorises_it() {
    // So a refusal can say why. "This module imports `pw:host/database#read`
    // and nothing in its effect row asks for `database.read<Stores>`" is
    // actionable; "undeclared import" is not.
    let c = one(&[PROGRAM], "shop.origin.Menu");
    let read = c
        .imports
        .iter()
        .find(|i| i.name == "read")
        .expect("the read import");
    assert_eq!(read.capability, "database.read<Stores>");
}

// --- the schema ----------------------------------------------------------

#[test]
fn the_schema_ignores_formatting_and_notices_authority() {
    // Over the INTERFACE, not the source. A comment must not change it; a new
    // capability must. A schema over source text is `resume_artifacts` scheme
    // 1, which this project superseded for exactly this reason.
    let plain = one(&[PROGRAM], "shop.origin.Menu");

    let commented = PROGRAM.replace(
        "public query Menu",
        "// a comment that changes nothing about the interface\npublic query Menu",
    );
    assert_eq!(
        one(&[&commented], "shop.origin.Menu").abi_schema,
        plain.abi_schema,
        "a comment is not an interface change"
    );

    let widened = PROGRAM.replace(
        "fn read_stores() -> Int !{ database.read<Stores> } { 0 }",
        "fn read_stores() -> Int !{ database.read<Stores>, secret<Payments> } { 0 }",
    );
    let widened = one(&[&widened], "shop.origin.Menu");
    assert_ne!(
        widened.abi_schema, plain.abi_schema,
        "a new capability IS an interface change"
    );
    assert!(
        widened
            .required_capabilities
            .iter()
            .any(|c| c.family == "secret"),
        "and it reaches the contract"
    );
}

#[test]
fn a_new_sibling_adds_a_contract_and_perturbs_no_other() {
    // Per-declaration contracts have a property module-wide ones cannot: a new
    // declaration is a new contract, and every existing one is byte-identical.
    // A host can therefore reload exactly what changed, and "the schema
    // changed" carries information instead of tracking the file.
    let before = build(&[PROGRAM]);
    let extra = format!(
        "{PROGRAM}\ncommand Discount(id: Int) -> Int\n    requires SignedIn\n{{\n    id\n}}\n"
    );
    let after = build(&[&extra]);

    assert_eq!(after.len(), before.len() + 1);
    for old in &before {
        let same = after
            .iter()
            .find(|c| c.component_id == old.component_id)
            .expect("an existing component is still there");
        assert_eq!(same, old, "{} was perturbed by a sibling", old.component_id);
    }
    assert!(
        after
            .iter()
            .any(|c| c.component_id == "shop.origin.Discount")
    );
}

#[test]
fn renaming_an_export_changes_that_contract() {
    // The control for the test above. If a sibling's arrival changes nothing
    // and a rename changes nothing either, the schema is a constant.
    let before = one(&[PROGRAM], "shop.origin.Menu");
    let renamed = PROGRAM.replace("query Menu(", "query Catalogue(");
    let after = one(&[&renamed], "shop.origin.Catalogue");
    assert_ne!(after.abi_schema, before.abi_schema);
    assert_eq!(after.exports[0].name, "Catalogue");
}

#[test]
fn a_page_does_not_inherit_its_handlers_authority() {
    // Least authority, at the place it is easiest to lose.
    //
    // A page that renders `on:press={.. => add_to_cart(..)}` CONTAINS a call
    // to a command that writes the database. It does not perform that write:
    // the handler does, when someone presses the button, and E7-L already
    // makes the handler a separately loaded unit. A contract that walked the
    // whole body would require `database.write` to RENDER a page — and a host
    // granting it would hand the render path authority it never uses.
    let program = "\
module shop.ui

fn write_stores() -> Int !{ database.write<Stores> } { 0 }

command Buy(id: Int) -> Int
    requires SignedIn
{
    write_stores()
}

page Shop(id: Int) {
    view {
        <button on:press={resumable() => Buy(id)}>Buy</button>
    }
}
";
    let page = one(&[program], "shop.ui.Shop");
    let command = one(&[program], "shop.ui.Buy");

    assert!(
        page.required_capabilities.is_empty(),
        "rendering requires no authority, got {:?}",
        page.required_capabilities
    );
    assert!(!audit(&page, &["pw:host/database#write".into()]).is_satisfied());

    // And the authority is not LOST — it is recorded once, against the thing
    // that performs it.
    assert_eq!(
        command
            .required_capabilities
            .iter()
            .map(Capability::name)
            .collect::<Vec<_>>(),
        ["database.write<Stores>"]
    );
    assert_eq!(command.allowed_placements, ["origin"]);
    assert_eq!(
        page.allowed_placements,
        ["build", "browser", "edge", "origin"],
        "and the page can be rendered anywhere, which is the point"
    );
}

#[test]
fn a_body_change_that_changes_neither_authority_nor_interface_keeps_the_schema() {
    // The control for the two tests above: if the schema changed on every
    // edit, "it changed" would carry no information and a host would reload on
    // noise.
    let plain = one(&[PROGRAM], "shop.origin.Menu");
    let rebodied = PROGRAM.replace(
        "fn write_stores() -> Int !{ database.write<Stores> } { 0 }",
        "fn write_stores() -> Int !{ database.write<Stores> } { 1 }",
    );
    assert_eq!(
        one(&[&rebodied], "shop.origin.Menu").abi_schema,
        plain.abi_schema
    );
}

#[test]
fn the_contract_round_trips_through_json() {
    // ADR-0018's boundary: the host deserializes this, and a field that does
    // not survive the trip is a field the host never sees.
    let c = one(&[PROGRAM], "shop.origin.Menu");
    let text = serde_json::to_string(&c).expect("serialize");
    let back: ComponentContract = serde_json::from_str(&text).expect("deserialize");
    assert_eq!(back, c);
}

#[test]
fn two_declarations_sharing_a_name_do_not_share_authority() {
    // `Inference::known` is keyed by the BARE declaration name, so
    // `Resources.Cart` and `store.page.Cart` share one entry in it. Deriving a
    // contract from that map gave a query whose body is `todo` the database
    // capability of an unrelated query in another module — authority granted
    // for a body that performs nothing, and it looked entirely reasonable in
    // the output.
    //
    // Contracts therefore infer from the BODY. This pins that: the two `Read`
    // declarations below differ only in what they do.
    let empty = "\
module a

component Read() {
    1
}
";
    let reads = "\
module b

fn touch() -> Int !{ database.read<Stores> } { 0 }

component Read() {
    touch()
}
";
    let all = build(&[empty, reads]);
    let a = all
        .iter()
        .find(|c| c.component_id == "a.Read")
        .expect("a.Read");
    let b = all
        .iter()
        .find(|c| c.component_id == "b.Read")
        .expect("b.Read");

    assert!(
        a.required_capabilities.is_empty(),
        "a component that does nothing needs nothing, got {:?}",
        a.required_capabilities
    );
    assert_eq!(
        b.required_capabilities
            .iter()
            .map(Capability::name)
            .collect::<Vec<_>>(),
        ["database.read<Stores>"],
        "and the one that does still needs it"
    );
    assert_ne!(a.abi_schema, b.abi_schema);
}

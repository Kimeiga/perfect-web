//! Placement follows what a body DOES, not what its annotation permits.
//!
//! Architect ruling, 2026-08-07:
//!
//! > Make placement consume exactly the same `effective_effects(DefId)` that
//! > contract generation uses. […] No caller should independently decide
//! > declared-versus-inferred for itself.
//!
//! ```text
//! local definition with body    inferred effects = actual requirements
//! bodyless/external definition  declared effects = requirements
//! ```
//!
//! # Why one function rather than two agreeing ones
//!
//! If placement chose declarations and contract generation chose inference,
//! one semantic declaration would have two answers about where it can run —
//! and they would agree until the day they did not. This project has spent its
//! life deleting that pattern; `docs/RISK_QUEUE.md` is mostly instances of it.
//!
//! The two controls the ruling named are `over_declaration_does_not_narrow`
//! and `a_body_that_performs_it_does`. They are the discriminating pair: a
//! rule reading annotations passes the second and fails the first, and a rule
//! reading nothing passes the first and fails the second.

use pw_core::check::check_sources;
use pw_core::contract::{Capability, contracts};
use pw_core::hir::Hir;
use pw_core::lower::lower_file;
use pw_core::resolve::Workspace;
use pw_core::signatures::Signatures;
use pw_syntax::parse_tree;

const LIB: &str = "\
module Stores

type Store = Store { id: Int }

fn get(id: Int) -> Store !{ database.read<Stores> } { Store { id: id } }

fn note(x: Int) -> Int !{ trace } { x }
";

fn codes(src: &str) -> Vec<&'static str> {
    check_sources(&[("lib.pw".into(), LIB.into()), ("t.pw".into(), src.into())])
        .into_iter()
        .flat_map(|(_, d)| d)
        .map(|d| d.code)
        .collect()
}

fn required(src: &str, component: &str) -> Vec<String> {
    let hirs: Vec<Hir> = [LIB, src]
        .iter()
        .map(|s| lower_file(s, &parse_tree(s).green))
        .collect();
    let refs: Vec<&Hir> = hirs.iter().collect();
    let ws = Workspace::build(&refs);
    let sigs = Signatures::build(&ws, &refs);
    contracts(&refs, &sigs, &ws)
        .into_iter()
        .find(|c| c.component_id == component)
        .unwrap_or_else(|| panic!("no contract for {component}"))
        .required_capabilities
        .iter()
        .map(Capability::name)
        .collect()
}

/// A component whose row PERMITS a database read and whose body only traces.
const OVER_DECLARED: &str = "\
module app

import Stores

component Badge()
    placement browser
{
    Stores.note(1)
}
";

/// The same shape, performing the read it names.
const HONEST: &str = "\
module app

import Stores

component Badge()
    placement browser
{
    Stores.get(1)
}
";

#[test]
fn over_declaration_does_not_narrow_placement() {
    // The ruling's first control. `Badge` may only run in the browser and its
    // body performs `trace`, which every world grants. A placement rule
    // reading the ANNOTATION would see `database.read` — origin-only — and
    // report a browser component as unplaceable.
    //
    // The row is on the helper here rather than on `Badge` itself, which is the
    // realistic shape: a library declares broadly and a caller uses one part.
    let found = codes(OVER_DECLARED);
    assert!(
        !found.contains(&"PW5002"),
        "a browser component that only traces is placeable: {found:?}"
    );
    assert!(
        !found.contains(&"PW5005"),
        "and its declared placement is not contradicted: {found:?}"
    );
}

#[test]
fn a_body_that_performs_it_does_narrow_placement() {
    // The ruling's second control, and the reason the first is not simply
    // "never report anything". The same declaration, the same placement, a
    // body that actually reads the database — and now it is refused.
    let found = codes(HONEST);
    assert!(
        found.contains(&"PW5005") || found.contains(&"PW5002"),
        "a browser component that reads the database must be refused: {found:?}"
    );
}

#[test]
fn placement_and_the_contract_read_the_same_thing() {
    // The invariant behind both controls. If these two ever disagree, one
    // semantic declaration has two answers about what it needs.
    assert!(
        required(OVER_DECLARED, "app.Badge").is_empty(),
        "the contract sees no capability either"
    );
    assert_eq!(
        required(HONEST, "app.Badge"),
        ["database.read<Stores>"],
        "and sees it when the body performs it"
    );
}

#[test]
fn a_definition_with_no_body_falls_back_to_its_declaration() {
    // The other half of the rule. An interface or separately compiled
    // declaration has no body to infer from, so its row IS its requirement —
    // and a contract that inferred "nothing" from "no body" would hand
    // external code an empty capability set.
    let external = "\
module app

fn reach() -> Int !{ database.read<Stores> }
";
    let hirs: Vec<Hir> = [LIB, external]
        .iter()
        .map(|s| lower_file(s, &parse_tree(s).green))
        .collect();
    let refs: Vec<&Hir> = hirs.iter().collect();
    let ws = Workspace::build(&refs);
    let sigs = Signatures::build(&ws, &refs);
    let inference = {
        let mut i = pw_core::effects::Inference::new(&sigs, &ws);
        i.run(&refs);
        i
    };
    let hir = &hirs[1];
    let (id, _) = hir
        .all_decls()
        .find(|(_, d)| d.name == "reach")
        .expect("reach");
    assert_eq!(
        inference.effective_effects(1, hir, id),
        ["database.read<Stores>"],
        "no body means the row is the requirement"
    );
}

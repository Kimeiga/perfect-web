//! **The contract matrix, frozen before the ontology drives it.**
//!
//! Architect ruling, 2026-08-07, on making `interface_for` declaration-driven:
//!
//! > Before changing `interface_for`, make a contract matrix that freezes the
//! > intended semantics. […] Then switch `interface_for` to the ontology and
//! > require exactly those contract changes. That will distinguish "we
//! > accidentally removed the DOM imports" from "the ontology intentionally
//! > says this is a browser-placement effect rather than host authority."
//!
//! So this file is an INSTRUMENT, not a rule. It records what
//! `contract::contracts()` produces today for one declaration per interesting
//! effect shape. Step 7 changes the derivation; this test then fails with a
//! diff, and step 8 is the review of that diff.
//!
//! `docs/RISK_QUEUE.md`'s admissibility rule is why it lands first: a
//! measurement is not admissible until its instrument can detect the
//! corresponding failure, and an instrument written after the change would
//! only be able to record the result.
//!
//! # The rows
//!
//! ```text
//! pure                              no capability, placeable anywhere
//! layout.measure                    browser-semantic, no authority
//! style.mutate<LayoutAffect>        the same, with an argument
//! database.read<Stores>             real host authority
//! secret<Payments>                  real host authority, origin only
//! layout.measure + database.read    nowhere to run
//! ```
//!
//! The last row is the one that proves the matrix is measuring placement and
//! not only imports: a body that measures layout AND reads the database has no
//! single world, and that must stay true through step 7 — the ontology changes
//! what authority is asked for, never where code can run.

use pw_core::contract::{ComponentContract, ImportKind, contracts};
use pw_core::hir::Hir;
use pw_core::lower::lower_file;
use pw_core::resolve::Workspace;
use pw_core::signatures::Signatures;
use pw_syntax::parse_tree;

/// The library the rows call into. Effect rows live on these signatures, so
/// each `query` below acquires its effects the way any program does —
/// through inference, not through its own annotation.
const LIB: &str = "\
module Stores

type Store = Store { id: Int }

// **Host-bound, explicitly.** An effect row is not a host binding — architect
// ruling, 2026-08-20. `get` and `payment_key` are the two rows that must keep
// asking for real authority, so they say where their implementations come from
// rather than having it inferred from what they perform.
fn get(id: Int) -> Store !{ database.read<Stores> }
    host \"pw:host/database#read\"

fn measure_it(id: Int) -> Int !{ layout.measure } { 0 }

fn set_width(id: Int) -> Int !{ style.mutate<LayoutAffect> } { 0 }

fn payment_key() -> Int !{ secret<Payments> }
    host \"pw:host/secrets#get\"

type LayoutAffect = LayoutAffect {}

type Payments = Payments {}
";

const ROWS: &str = "\
module rows

import Stores

query Pure(id: Int) -> Int
    cache shared
{
    id
}

query Measured(id: Int) -> Int
    cache shared
{
    Stores.measure_it(id)
}

query Styled(id: Int) -> Int
    cache shared
{
    Stores.set_width(id)
}

query Read(id: Int) -> Int
    cache shared
{
    Stores.get(id).id
}

query Secret(id: Int) -> Int
    cache shared
{
    Stores.payment_key()
}

query Both(id: Int) -> Int
    cache shared
{
    Stores.measure_it(Stores.get(id).id)
}
";

/// The real effect declarations, so the ontology this exercises is the one the
/// compiler will use.
///
/// Read from `packages/` rather than restated here. A matrix built against a
/// hand-written copy of the vocabulary would freeze the copy, and step 8 would
/// then verify that the change did what the test file said instead of what the
/// platform says.
fn declarations() -> Vec<String> {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    [
        "packages/pw-std/effects.pw",
        "packages/pw-platform-web/effects.pw",
    ]
    .iter()
    .map(|p| std::fs::read_to_string(root.join(p)).expect("effect declarations"))
    .collect()
}

fn matrix() -> Vec<ComponentContract> {
    let mut sources = declarations();
    sources.push(LIB.to_string());
    sources.push(ROWS.to_string());
    let hirs: Vec<Hir> = sources
        .iter()
        .map(|s| lower_file(s, &parse_tree(s).green))
        .collect();
    let refs: Vec<&Hir> = hirs.iter().collect();
    let ws = Workspace::build(&refs);
    let sigs = Signatures::build(&ws, &refs);
    contracts(&refs, &sigs, &ws)
}

fn row(name: &str) -> ComponentContract {
    matrix()
        .into_iter()
        .find(|c| c.component_id == format!("rows.{name}"))
        .unwrap_or_else(|| panic!("no contract for `rows.{name}`"))
}

/// `capability`, `interface#name` for each HOST import, sorted.
fn host_imports(c: &ComponentContract) -> Vec<String> {
    let mut out: Vec<String> = c
        .imports
        .iter()
        .filter(|i| i.kind == ImportKind::HostCapability)
        .map(|i| format!("{} = {}", i.capability, i.key()))
        .collect();
    out.sort();
    out
}

fn capabilities(c: &ComponentContract) -> Vec<String> {
    let mut out: Vec<String> = c.required_capabilities.iter().map(|c| c.name()).collect();
    out.sort();
    out
}

#[test]
fn the_program_the_matrix_is_built_from_actually_produces_contracts() {
    // The control. Every row below asserts on one contract, and a harness that
    // silently produced none would make each of them vacuous — `row()` panics,
    // but only if the test bothers to call it.
    let all = matrix();
    assert_eq!(
        all.len(),
        6,
        "six queries, six contracts (the effect declarations are not \
         components and contribute none): {:?}",
        all.iter().map(|c| &c.component_id).collect::<Vec<_>>()
    );
}

#[test]
fn pure_needs_nothing_and_runs_anywhere() {
    let c = row("Pure");
    assert!(capabilities(&c).is_empty(), "{:?}", capabilities(&c));
    assert!(host_imports(&c).is_empty(), "{:?}", host_imports(&c));
    assert_eq!(c.allowed_placements, ["build", "browser", "edge", "origin"]);
}

#[test]
fn layout_measure_is_browser_placement_and_not_host_authority() {
    // **The row step 7 changed, and the reason the ruling wanted it.**
    //
    // Before: `capabilities {layout.measure}`, importing
    // `pw:host/layout#measure` — asking a host to grant the layout engine to
    // code whose only crime is being a user interface. `Capability::resolve`
    // read `World::worlds_for`'s restricted-family list as "needs a
    // capability", and that list answers a different question.
    //
    // After: no capability and no import. The declaration says
    // `placement browser, capability none`, and both facts survive: the
    // placement is unchanged.
    let c = row("Measured");
    assert!(capabilities(&c).is_empty(), "{:?}", capabilities(&c));
    assert!(host_imports(&c).is_empty(), "{:?}", host_imports(&c));
    assert_eq!(
        c.allowed_placements,
        ["browser"],
        "the ontology changes what authority is asked for, never where code runs"
    );
}

#[test]
fn style_mutate_is_browser_placement_even_carrying_an_argument() {
    // The same change, on an effect with a type argument — so the reason is
    // the DECLARATION and not "effects without arguments need no authority".
    let c = row("Styled");
    assert!(capabilities(&c).is_empty(), "{:?}", capabilities(&c));
    assert!(host_imports(&c).is_empty(), "{:?}", host_imports(&c));
    assert_eq!(c.allowed_placements, ["browser"]);
}

#[test]
fn database_read_asks_for_real_authority_and_must_keep_asking() {
    // The row step 7 must NOT change. If this moves, the change removed
    // authority rather than reclassifying it.
    let c = row("Read");
    assert_eq!(capabilities(&c), ["database.read<Stores>"]);
    assert_eq!(
        host_imports(&c),
        ["database.read<Stores> = pw:host/database#read"]
    );
    assert_eq!(c.allowed_placements, ["origin"]);
}

#[test]
fn a_secret_asks_for_real_authority_and_must_keep_asking() {
    // `secret<Payments>` as the corpus writes it. The architect's matrix said
    // `secret.read<Payments>`; the corpus declares `secret<C>` with an
    // argument and `secret.read` without one, and this uses what exists.
    //
    // The row where step 7 changed the INTERFACE and not the authority:
    // `pw:host/secret#use` was formatted from the family and an empty
    // operation, and the platform's module is `secrets` with a `get`. The
    // capability is identical, which is what makes this the control proving
    // the change is about naming rather than about granting.
    let c = row("Secret");
    assert_eq!(capabilities(&c), ["secret<Payments>"]);
    assert_eq!(host_imports(&c), ["secret<Payments> = pw:host/secrets#get"]);
    assert_eq!(c.allowed_placements, ["origin"]);
}

#[test]
fn measuring_layout_and_reading_the_database_has_nowhere_to_run() {
    // The row that proves the matrix measures PLACEMENT and not only imports.
    // Browser-only and origin-only in one body is a contradiction, and it must
    // stay one through step 7: the ontology changes what authority is asked
    // for, never where code can run.
    let c = row("Both");
    // `layout.measure` is no longer a capability — but it is still an EFFECT,
    // and placement solves over effects. This is the assertion that separates
    // the two: had the solver been reading the capability list, dropping
    // `layout.measure` from it would have made this body placeable at the
    // origin, and a browser-only measurement would be scheduled on a server.
    assert_eq!(capabilities(&c), ["database.read<Stores>"]);
    assert!(
        c.allowed_placements.is_empty(),
        "a body that measures layout and reads the database has no world: {:?}",
        c.allowed_placements
    );
}

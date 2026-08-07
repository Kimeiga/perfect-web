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

fn get(id: Int) -> Store !{ database.read<Stores> } { Store { id: id } }

fn measure_it(id: Int) -> Int !{ layout.measure } { 0 }

fn set_width(id: Int) -> Int !{ style.mutate<LayoutAffect> } { 0 }

fn payment_key() -> Int !{ secret<Payments> } { 0 }

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

fn matrix() -> Vec<ComponentContract> {
    let hirs: Vec<Hir> = [LIB, ROWS]
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
        "six queries, six contracts: {:?}",
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
fn layout_measure_asks_the_host_for_authority_today() {
    // **The row step 7 changes.** `layout.measure` is browser-semantic and
    // `packages/pw-platform-web/effects.pw` declares `capability none` for it —
    // but `Capability::resolve` reads `World::worlds_for`'s restricted-family
    // table as "needs a host capability", so the contract asks a host to grant
    // the layout engine to code whose only crime is being a user interface.
    let c = row("Measured");
    assert_eq!(capabilities(&c), ["layout.measure"]);
    assert_eq!(
        host_imports(&c),
        ["layout.measure = pw:host/layout#measure"]
    );
    // Placement is already right, and must not move.
    assert_eq!(c.allowed_placements, ["browser"]);
}

#[test]
fn style_mutate_carries_its_argument_into_the_capability_today() {
    let c = row("Styled");
    assert_eq!(capabilities(&c), ["style.mutate<LayoutAffect>"]);
    assert_eq!(
        host_imports(&c),
        ["style.mutate<LayoutAffect> = pw:host/style#mutate"]
    );
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
    // The interface name is the row step 7 changes WITHOUT changing authority:
    // `pw:host/secret#use` is formatted from the family and an empty
    // operation, while the declaration says `pw:host/secrets#get`.
    let c = row("Secret");
    assert_eq!(capabilities(&c), ["secret<Payments>"]);
    assert_eq!(host_imports(&c), ["secret<Payments> = pw:host/secret#use"]);
    assert_eq!(c.allowed_placements, ["origin"]);
}

#[test]
fn measuring_layout_and_reading_the_database_has_nowhere_to_run() {
    // The row that proves the matrix measures PLACEMENT and not only imports.
    // Browser-only and origin-only in one body is a contradiction, and it must
    // stay one through step 7: the ontology changes what authority is asked
    // for, never where code can run.
    let c = row("Both");
    assert_eq!(
        capabilities(&c),
        ["database.read<Stores>", "layout.measure"]
    );
    assert!(
        c.allowed_placements.is_empty(),
        "a body that measures layout and reads the database has no world: {:?}",
        c.allowed_placements
    );
}

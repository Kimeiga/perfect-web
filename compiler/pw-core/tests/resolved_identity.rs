//! Inferred facts belong to a declaration, not to a spelling.
//!
//! `docs/RISK_QUEUE.md` 34. `Inference::known` was keyed by the bare
//! declaration name, so `Resources.Cart` and `store.page.Cart` shared one
//! entry. A query whose body is `todo` was reported as requiring
//! `database.read` — which is exactly what a cart query *should* require, so
//! the wrongness was invisible in the output.
//!
//! Architect ruling, 2026-08-07:
//!
//! > It should be `DefId → inferred facts` […] never `"Cart" → inferred facts`.
//! > This is not "implementing the permanent E9 type system early". It is
//! > repairing the current compiler so every existing analysis consumes the
//! > name resolution machinery E2B already established.

use pw_core::contract::{Capability, ComponentContract, contracts};
use pw_core::hir::Hir;
use pw_core::lower::lower_file;
use pw_core::resolve::Workspace;
use pw_core::signatures::Signatures;
use pw_syntax::parse_tree;

/// Two modules, one spelling, different bodies, different effects.
const QUIET: &str = "\
module Resources

query Cart(session: Int) -> Int
    freshness   0.seconds
    consistency snapshot
    cache       private
    key         session
{
    todo
}
";

const BUSY: &str = "\
module store.page

fn read_carts(s: Int) -> Int !{ database.read<Carts> } { s }

query Cart(session: Int) -> Int
    freshness   0.seconds
    consistency snapshot
    cache       private
    key         session
{
    read_carts(session)
}
";

fn build(sources: &[&str]) -> Vec<ComponentContract> {
    let hirs: Vec<Hir> = sources
        .iter()
        .map(|s| lower_file(s, &parse_tree(s).green))
        .collect();
    let refs: Vec<&Hir> = hirs.iter().collect();
    let ws = Workspace::build(&refs);
    let sigs = Signatures::build(&ws, &refs);
    contracts(&refs, &sigs, &ws)
}

fn find<'a>(all: &'a [ComponentContract], id: &str) -> &'a ComponentContract {
    all.iter()
        .find(|c| c.component_id == id)
        .unwrap_or_else(|| panic!("no contract for {id}"))
}

#[test]
fn each_inferred_result_belongs_only_to_its_own_declaration() {
    // Both orders, because a map keyed by name gives whichever ran LAST — so a
    // one-order test passes half the time by construction.
    for sources in [[QUIET, BUSY], [BUSY, QUIET]] {
        let all = build(&sources);
        let quiet = find(&all, "Resources.Cart");
        let busy = find(&all, "store.page.Cart");

        assert!(
            quiet.required_capabilities.is_empty(),
            "a body of `todo` performs nothing, got {:?} (order {:?})",
            quiet.required_capabilities,
            sources.iter().map(|s| &s[..20]).collect::<Vec<_>>()
        );
        assert_eq!(
            busy.required_capabilities
                .iter()
                .map(Capability::name)
                .collect::<Vec<_>>(),
            ["database.read<Carts>"],
            "and the one that reads still does"
        );
        assert_ne!(quiet.abi_schema, busy.abi_schema);
    }
}

#[test]
fn the_two_declarations_would_be_indistinguishable_by_name() {
    // The control. If the two declarations differed in spelling, this file
    // would prove nothing about identity — it would prove that two different
    // names get different answers, which no implementation has ever got wrong.
    let all = build(&[QUIET, BUSY]);
    let names: Vec<&str> = all
        .iter()
        .map(|c| c.component_id.rsplit('.').next().unwrap())
        .collect();
    assert_eq!(
        names.iter().filter(|n| **n == "Cart").count(),
        2,
        "two declarations share the spelling `Cart`"
    );
}

#[test]
fn a_helper_reaches_its_own_module_and_not_a_namesake_elsewhere() {
    // The propagation direction. `store.page.Cart` calls `read_carts`, and
    // there is a `read_carts` in the other module that does something else. A
    // name-keyed map merges them; resolution does not.
    const DECOY: &str = "\
module Decoy

fn read_carts(s: Int) -> Int !{ secret<Payments> } { s }
";
    let all = build(&[BUSY, DECOY]);
    let busy = find(&all, "store.page.Cart");
    assert_eq!(
        busy.required_capabilities
            .iter()
            .map(Capability::name)
            .collect::<Vec<_>>(),
        ["database.read<Carts>"],
        "the decoy's `secret<Payments>` must not reach it"
    );
}

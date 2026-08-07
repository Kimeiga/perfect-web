//! **The placement baseline, frozen before `worlds_for` is deleted.**
//!
//! Architect ruling, 2026-08-07:
//!
//! > Use `the_ontology_and_worlds_for_agree_about_placement_where_both_speak`
//! > as the migration oracle: freeze current placement results, make
//! > planner/placement consume ontology + topology only, delete `worlds_for`,
//! > require all intended rows to remain unchanged, and classify any changed
//! > row explicitly.
//!
//! Same discipline as `contract_matrix.rs`, which caught a live security hole
//! on its first run precisely because it predated the change it was built for.
//!
//! # What is being migrated away from
//!
//! `World::worlds_for(family)` is a hard-coded capability→world table in the
//! compiler. The ontology now declares the same fact per effect:
//!
//! ```pleris
//! effect layout.measure   { placement browser  capability none }
//! effect database.read<T> { placement origin   capability database.read<T> }
//! ```
//!
//! Two sources for one answer, agreeing today. The table goes away, and these
//! rows are what says the deletion changed nothing it should not have.

use pw_core::placement::{Demand, World, solve};
use pw_core::privacy::Label;

/// The worlds a body performing these effects can run in.
fn feasible(effects: &[&str]) -> Vec<String> {
    let demand = Demand {
        effects: effects.iter().map(|s| s.to_string()).collect(),
        label: Label::public(),
        declared: None,
    };
    let mut out: Vec<String> = solve(&demand)
        .feasible
        .into_iter()
        .map(|w| w.name().to_string())
        .collect();
    out.sort();
    out
}

#[test]
fn the_solver_answers_at_all() {
    // The control. Every row below is a set comparison, and a solver returning
    // nothing for everything would make half of them trivially true.
    assert_eq!(feasible(&[]), ["browser", "build", "edge", "origin"]);
    assert_eq!(World::worlds_for("database"), Some(&[World::Origin][..]));
}

#[test]
fn the_placement_baseline() {
    // Every effect the corpus writes, and where a body performing it may run.
    // Sorted names rather than `World`s, so a change shows as a readable diff.

    // Origin-only: real authority the browser must not have.
    for e in [
        "database.read<Stores>",
        "database.write<Carts>",
        "database.transaction",
        "database.connect",
        "secret<Payments>",
        "secret.read",
    ] {
        assert_eq!(feasible(&[e]), ["origin"], "{e}");
    }

    // Browser-only: meaningful where the document is.
    for e in [
        "dom.mutate",
        "dom.read",
        "layout.measure",
        "style.mutate<LayoutAffect>",
        "style.mutate<PaintOnly>",
        "animation.composite",
        "paint.custom",
        "device.location",
        "device.query",
    ] {
        assert_eq!(feasible(&[e]), ["browser"], "{e}");
    }

    // Anywhere with a request context; build time has none.
    for e in ["network.fetch", "network.subscribe"] {
        assert_eq!(feasible(&[e]), ["browser", "edge", "origin"], "{e}");
    }

    // Unconstrained today. `session.read` is in this group and the architect
    // has ruled it should not be — it is moving to capability + topology, and
    // this row is what will catch the movement.
    for e in [
        "log<Public>",
        "trace",
        "clock.wall",
        "clock.read",
        "resource.acquire<MapHandle>",
        "resource.release<MapHandle>",
        "unsafe.raw_html",
        "session.read",
    ] {
        assert_eq!(
            feasible(&[e]),
            ["browser", "build", "edge", "origin"],
            "{e}"
        );
    }
}

#[test]
fn the_combinations_that_have_nowhere_to_run() {
    // The rows that prove placement is an INTERSECTION and not a lookup. These
    // are what a planner consuming ontology + topology must keep answering the
    // same way.
    assert!(
        feasible(&["layout.measure", "database.read<Stores>"]).is_empty(),
        "browser-only and origin-only in one body"
    );
    assert!(
        feasible(&["dom.mutate", "secret<Payments>"]).is_empty(),
        "the charter's opening example: a secret in a browser component"
    );
    // And one that DOES intersect, so "empty" is not the answer to everything.
    assert_eq!(
        feasible(&["network.fetch", "database.read<Stores>"]),
        ["origin"],
        "a fetch and a read agree on the origin"
    );
}

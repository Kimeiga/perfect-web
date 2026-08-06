//! Charter §14 M6 task 10: invalidate one store among many, and confirm the
//! others do not regenerate.
//!
//! The measurement is a **count of producer calls**, not a duration. A timing
//! number here would be a number about this laptop; the count is a property of
//! the design, and it is the one the claim rests on:
//!
//! > Change one menu item and show exactly what recomputes — and everything
//! > that does not.
//!
//! Both halves are counted. "One regenerated" is worth nothing without "999 did
//! not", and "999 did not" is worth nothing unless those 999 exist, are stale-
//! able, and would have regenerated had the event named them — so the same
//! fixture is run twice, once with a narrow event and once with a broad one,
//! and the second run's count is the proof that the first run's could have been
//! large.

use pw_materialize::*;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

const STORES: usize = 1_000;

fn graph() -> Graph {
    Graph::from_json(
        r#"{
      "nodes": [
        { "path": "Events.MenuChanged", "name": "MenuChanged", "node": "event", "params": ["store"] },
        { "path": "store.MenuFragment", "name": "MenuFragment", "node": "materialization",
          "partition": "public", "fallback": "last_known_good",
          "varies_by": ["code_version"], "params": ["id"] }
      ],
      "edges": [
        { "from": "store.MenuFragment", "to": "Events.MenuChanged",
          "kind": "invalidated_by", "key": ["id"] }
      ],
      "dangling": []
    }"#,
    )
    .expect("graph")
}

fn key(store: usize) -> EntryKey {
    EntryKey::new("store.MenuFragment", &[&store.to_string()]).with("code_version", "build-1")
}

/// A thousand materialized stores, one event, and the counts on both sides.
fn run(event: Event) -> (usize, usize) {
    let clock = Clock::new();
    let m = Materializer::new(clock.clone());
    m.declare("store.MenuFragment", FragmentPolicy::default());

    let keys: Vec<EntryKey> = (0..STORES).map(key).collect();
    for (i, k) in keys.iter().enumerate() {
        clock.advance(1);
        m.regenerate(k, None, || Ok(format!("menu {i}")));
    }
    assert_eq!(m.all_entries().len(), STORES, "every store materialized");

    m.command::<()>(|_tx| Ok(vec![event])).expect("command");
    let invalidated = m.drain(&graph(), &keys);

    let produced = Arc::new(AtomicUsize::new(0));
    for (k, because) in &invalidated {
        clock.advance(1);
        let p = produced.clone();
        m.regenerate(k, Some(*because), || {
            p.fetch_add(1, Ordering::SeqCst);
            Ok("revised".to_string())
        });
    }

    // "Untouched" is read from the causality record, not from freshness: the
    // one store that DID regenerate is fresh again afterwards, so counting
    // fresh entries would report 1000 of 1000 untouched and call it a pass.
    // An entry with no cause is one that has not been rebuilt since it was
    // first materialized.
    let untouched = m
        .all_entries()
        .values()
        .filter(|e| e.because.is_none())
        .count();
    (produced.load(Ordering::SeqCst), untouched)
}

#[test]
fn one_store_changing_regenerates_one_of_a_thousand() {
    let (regenerated, _) = run(Event::new("Events.MenuChanged", &["47"]));
    assert_eq!(regenerated, 1, "one store changed, one fragment recomputed");
}

#[test]
fn the_other_nine_hundred_and_ninety_nine_are_untouched() {
    let (_, untouched) = run(Event::new("Events.MenuChanged", &["47"]));
    assert_eq!(
        untouched,
        STORES - 1,
        "every other store still serves the entry it had"
    );
}

#[test]
fn the_same_thousand_would_have_regenerated_had_the_event_named_them() {
    // The control. Without it, "1 of 1000 regenerated" is equally consistent
    // with a materializer that regenerates nothing, and the headline number
    // would be measuring the absence of a feature.
    let (regenerated, untouched) = run(Event::new("Events.MenuChanged", &[]));
    assert_eq!(
        regenerated, STORES,
        "an event that names no store reaches every store"
    );
    assert_eq!(untouched, 0, "and none of them is left un-rebuilt");
}

#[test]
fn an_event_for_a_store_that_was_never_materialized_regenerates_nothing() {
    // The third case, and the one an "invalidate everything" implementation
    // would also pass — included so the set of tested shapes is not two.
    let (regenerated, untouched) = run(Event::new("Events.MenuChanged", &["999999"]));
    assert_eq!(regenerated, 0);
    assert_eq!(untouched, STORES);
}

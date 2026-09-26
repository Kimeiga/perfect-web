//! **A listener binds its entry's key, position by position** (ADR-0089).
//!
//! `invalidates_on InventoryChanged(id, _)` says which of an event's values
//! must equal which part of the entry's key: the first must equal the
//! fragment's `id`, and the second may be anything. Until 2026-09-26 the
//! materializer read an event's values as a set and asked whether each was
//! in the entry's key. `InventoryChanged(47, item 3)` was deferred forever,
//! since the item is in no fragment's key, and store 47's menu stayed fresh
//! after its inventory changed. A value at one position also matched a key
//! at another.

use pw_materialize::*;

/// A fragment keyed by a store, listening for two events: the menu's, which
/// carries the store, and the inventory's, which carries the store and an
/// item. And a fragment keyed by two values, listening for an event that
/// carries one.
fn graph() -> Graph {
    Graph::from_json(
        r#"{
      "nodes": [
        { "path": "Events.MenuChanged", "name": "MenuChanged", "node": "event", "params": ["store"] },
        { "path": "Events.InventoryChanged", "name": "InventoryChanged", "node": "event",
          "params": ["store", "item"] },
        { "path": "Events.Swapped", "name": "Swapped", "node": "event", "params": ["from", "to"] },
        { "path": "store.MenuFragment", "name": "MenuFragment", "node": "materialization",
          "placement": "edge", "partition": "public", "regenerate": "on_invalidation",
          "stampede": "single_flight", "fallback": "last_known_good", "params": ["id"] },
        { "path": "store.Pair", "name": "Pair", "node": "materialization",
          "placement": "edge", "partition": "public", "regenerate": "on_invalidation",
          "stampede": "single_flight", "fallback": "last_known_good", "params": ["a", "b"] }
      ],
      "edges": [
        { "from": "store.MenuFragment", "to": "Events.MenuChanged",
          "kind": "invalidated_by", "key": ["id"] },
        { "from": "store.MenuFragment", "to": "Events.InventoryChanged",
          "kind": "invalidated_by", "key": ["id", "_"] },
        { "from": "store.MenuFragment", "to": "Events.Swapped",
          "kind": "invalidated_by", "key": ["_", "id"] },
        { "from": "store.Pair", "to": "Events.MenuChanged",
          "kind": "invalidated_by", "key": ["b"] }
      ],
      "dangling": []
    }"#,
    )
    .expect("the graph parses")
}

fn menu(store: &str) -> EntryKey {
    EntryKey::new("store.MenuFragment", &[store]).with("partition", "public")
}

/// Materialize `keys`, emit `event`, drain, and answer which were
/// invalidated.
fn invalidated(keys: &[EntryKey], event: Event) -> Vec<EntryKey> {
    let clock = Clock::new();
    let m = Materializer::new(clock.clone(), "build-1");
    m.declare("store.MenuFragment", FragmentPolicy::default());
    m.declare("store.Pair", FragmentPolicy::default());
    for k in keys {
        clock.advance(10);
        assert_eq!(
            m.regenerate(k, None, || Ok("body".to_string())),
            Regenerated::Fresh
        );
    }
    m.command::<()>(|_tx| Ok(vec![event])).expect("command");
    m.drain(&graph(), keys)
        .into_iter()
        .map(|(k, _)| k)
        .collect()
}

#[test]
fn an_event_reaches_the_entry_its_listener_binds() {
    let keys = [menu("47"), menu("48")];
    assert_eq!(
        invalidated(
            &keys,
            Event::new("Events.InventoryChanged", &["47", "item-3"])
        ),
        vec![menu("47")],
        "store 47's inventory changed, so store 47's menu is stale, and 48's is not"
    );
    // The control, as it always held: one value, bound by the one argument.
    assert_eq!(
        invalidated(&keys, Event::new("Events.MenuChanged", &["48"])),
        vec![menu("48")]
    );
}

/// The listener binds the second value, `Swapped(_, id)`: the first is not
/// the key, whatever it equals.
#[test]
fn a_value_is_compared_at_the_position_its_listener_binds() {
    let keys = [menu("47"), menu("48")];
    assert_eq!(
        invalidated(&keys, Event::new("Events.Swapped", &["47", "48"])),
        vec![menu("48")]
    );
}

/// A value the listener binds to one part of the key is not compared with
/// another part. `Pair(1, 2)` listens for `MenuChanged(b)`, so the menu of
/// store 1 is not its `b`.
#[test]
fn a_value_is_not_a_match_for_another_part_of_the_key() {
    let pair = EntryKey::new("store.Pair", &["1", "2"]).with("partition", "public");
    assert_eq!(
        invalidated(
            std::slice::from_ref(&pair),
            Event::new("Events.MenuChanged", &["1"])
        ),
        Vec::<EntryKey>::new()
    );
    assert_eq!(
        invalidated(
            std::slice::from_ref(&pair),
            Event::new("Events.MenuChanged", &["2"])
        ),
        vec![pair]
    );
}

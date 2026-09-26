//! **An event reaches what reads what it invalidates** (ADR-0102).
//!
//! Charter §9.4 treats a materialization as a view over resources:
//! `MenuChanged(store_47)` "should invalidate only affected resource snapshots
//! and page fragments". The compiler's graph says so (`Graph::affected_by`
//! follows reads). Until 2026-09-26 the materializer invalidated an event's
//! direct listeners and nothing that reads them: A-009's `MenuFragment`
//! depends on `Store(id)`, which listens for `StoreChanged(id)`, and a renamed
//! store kept its old fragment. Each test states one case, with a control.

use pw_materialize::*;

/// A store query listening for `StoreChanged`, a menu query depending on it,
/// and fragments reading each: one keyed by the store it reads, one reading
/// a banner whose key it does not supply, and one reading nothing that
/// listens.
fn graph() -> Graph {
    Graph::from_json(
        r#"{
      "nodes": [
        { "path": "Events.StoreChanged", "name": "StoreChanged", "node": "event", "params": ["store"] },
        { "path": "Events.BannerChanged", "name": "BannerChanged", "node": "event", "params": ["region"] },
        { "path": "Events.PriceChanged", "name": "PriceChanged", "node": "event", "params": ["store"] },
        { "path": "Resources.Store", "name": "Store", "node": "resource",
          "partition": "shared", "privacy": "public", "params": ["id"] },
        { "path": "Resources.Menu", "name": "Menu", "node": "resource",
          "partition": "shared", "privacy": "public", "params": ["id"] },
        { "path": "Resources.Banner", "name": "Banner", "node": "resource",
          "partition": "shared", "privacy": "public", "params": ["region"] },
        { "path": "Resources.Prices", "name": "Prices", "node": "resource",
          "partition": "shared", "privacy": "public", "params": ["id"] },
        { "path": "store.MenuFragment", "name": "MenuFragment", "node": "materialization",
          "placement": "edge", "partition": "public", "regenerate": "on_invalidation",
          "stampede": "single_flight", "fallback": "last_known_good", "params": ["id"] },
        { "path": "store.BannerFragment", "name": "BannerFragment", "node": "materialization",
          "placement": "edge", "partition": "public", "regenerate": "on_invalidation",
          "stampede": "single_flight", "fallback": "last_known_good", "params": ["id"] },
        { "path": "store.PriceFragment", "name": "PriceFragment", "node": "materialization",
          "placement": "edge", "partition": "public", "regenerate": "on_invalidation",
          "stampede": "single_flight", "fallback": "last_known_good", "params": ["id"] }
      ],
      "edges": [
        { "from": "Resources.Store", "to": "Events.StoreChanged",
          "kind": "invalidated_by", "key": ["id"] },
        { "from": "Resources.Menu", "to": "Resources.Store", "kind": "reads", "key": ["id"] },
        { "from": "Resources.Banner", "to": "Events.BannerChanged",
          "kind": "invalidated_by", "key": ["region"] },
        { "from": "store.MenuFragment", "to": "Resources.Menu", "kind": "reads", "key": ["id"] },
        { "from": "store.BannerFragment", "to": "Resources.Banner", "kind": "reads", "key": [] },
        { "from": "store.PriceFragment", "to": "Resources.Prices", "kind": "reads", "key": ["id"] }
      ],
      "dangling": []
    }"#,
    )
    .expect("the graph parses")
}

fn entry(fragment: &str, store: &str) -> EntryKey {
    EntryKey::new(fragment, &[store]).with("partition", "public")
}

/// Materialize `keys`, emit `event`, drain over `g`, and answer which were
/// invalidated.
fn invalidated_in(g: &Graph, keys: &[EntryKey], event: Event) -> Vec<EntryKey> {
    let clock = Clock::new();
    let m = Materializer::new(clock.clone(), "build-1");
    for k in keys {
        m.declare(&k.fragment, FragmentPolicy::default());
    }
    for k in keys {
        clock.advance(10);
        assert_eq!(
            m.regenerate(k, None, || Ok("body".to_string())),
            Regenerated::Fresh
        );
    }
    m.command::<()>(|_tx| Ok(vec![event])).expect("command");
    m.drain(g, keys).into_iter().map(|(k, _)| k).collect()
}

fn invalidated(keys: &[EntryKey], event: Event) -> Vec<EntryKey> {
    invalidated_in(&graph(), keys, event)
}

#[test]
fn an_event_reaches_what_reads_its_listener() {
    // Through `Menu`, which depends on `Store`: store 47's fragment, and not
    // store 48's.
    let keys = [
        entry("store.MenuFragment", "47"),
        entry("store.MenuFragment", "48"),
    ];
    assert_eq!(
        invalidated(&keys, Event::new("Events.StoreChanged", &["47"])),
        [entry("store.MenuFragment", "47")]
    );
}

#[test]
fn a_key_the_reader_does_not_supply_reaches_every_entry() {
    // `BannerFragment` reads `Banner` without saying which region, so a
    // change to any region may be one it shows.
    let keys = [
        entry("store.BannerFragment", "47"),
        entry("store.BannerFragment", "48"),
    ];
    assert_eq!(
        invalidated(&keys, Event::new("Events.BannerChanged", &["eu"])),
        keys
    );
}

#[test]
fn a_reader_of_what_the_event_does_not_reach_is_untouched() {
    // `Prices` listens for nothing, so `PriceChanged` has no listener and
    // reaches no fragment; `StoreChanged` reaches `Store`, which
    // `PriceFragment` does not read.
    let keys = [entry("store.PriceFragment", "47")];
    assert!(invalidated(&keys, Event::new("Events.PriceChanged", &["47"])).is_empty());
    let keys = [
        entry("store.PriceFragment", "47"),
        entry("store.MenuFragment", "47"),
    ];
    assert_eq!(
        invalidated(&keys, Event::new("Events.StoreChanged", &["47"])),
        [entry("store.MenuFragment", "47")]
    );
}

#[test]
fn reads_that_form_a_cycle_end() {
    // Two queries depending on each other, one of them on `Store`, and a
    // fragment reading the other: the walk ends, and the event reaches the
    // fragment through both.
    let g = Graph::from_json(
        r#"{
      "nodes": [
        { "path": "Events.StoreChanged", "name": "StoreChanged", "node": "event", "params": ["store"] },
        { "path": "Resources.A", "name": "A", "node": "resource", "params": ["id"] },
        { "path": "Resources.B", "name": "B", "node": "resource", "params": ["id"] },
        { "path": "Resources.Store", "name": "Store", "node": "resource", "params": ["id"] },
        { "path": "store.F", "name": "F", "node": "materialization", "params": ["id"] }
      ],
      "edges": [
        { "from": "Resources.A", "to": "Resources.B", "kind": "reads", "key": ["id"] },
        { "from": "Resources.B", "to": "Resources.A", "kind": "reads", "key": ["id"] },
        { "from": "Resources.B", "to": "Resources.Store", "kind": "reads", "key": ["id"] },
        { "from": "Resources.Store", "to": "Events.StoreChanged",
          "kind": "invalidated_by", "key": ["id"] },
        { "from": "store.F", "to": "Resources.A", "kind": "reads", "key": ["id"] }
      ],
      "dangling": []
    }"#,
    )
    .expect("the graph parses");
    let keys = [entry("store.F", "47"), entry("store.F", "48")];
    assert_eq!(
        invalidated_in(&g, &keys, Event::new("Events.StoreChanged", &["47"])),
        [entry("store.F", "47")]
    );
}

#[test]
fn a_node_read_twice_is_followed_at_each_key() {
    // `Pair(a, b)` reads `X` twice, once at each of its keys, through `P`
    // and `Q`; `X` reads `Y`, which listens. Pair 1-2 hears of a change to
    // 2 through `Q` although `P` read `X` at 1 first.
    let g = Graph::from_json(
        r#"{
      "nodes": [
        { "path": "Events.Changed", "name": "Changed", "node": "event", "params": ["id"] },
        { "path": "Resources.P", "name": "P", "node": "resource", "params": ["id"] },
        { "path": "Resources.Q", "name": "Q", "node": "resource", "params": ["id"] },
        { "path": "Resources.X", "name": "X", "node": "resource", "params": ["id"] },
        { "path": "Resources.Y", "name": "Y", "node": "resource", "params": ["id"] },
        { "path": "store.Pair", "name": "Pair", "node": "materialization", "params": ["a", "b"] }
      ],
      "edges": [
        { "from": "store.Pair", "to": "Resources.P", "kind": "reads", "key": ["a"] },
        { "from": "store.Pair", "to": "Resources.Q", "kind": "reads", "key": ["b"] },
        { "from": "Resources.P", "to": "Resources.X", "kind": "reads", "key": ["id"] },
        { "from": "Resources.Q", "to": "Resources.X", "kind": "reads", "key": ["id"] },
        { "from": "Resources.X", "to": "Resources.Y", "kind": "reads", "key": ["id"] },
        { "from": "Resources.Y", "to": "Events.Changed",
          "kind": "invalidated_by", "key": ["id"] }
      ],
      "dangling": []
    }"#,
    )
    .expect("the graph parses");
    let pair = |a: &str, b: &str| EntryKey::new("store.Pair", &[a, b]).with("partition", "public");
    let keys = [pair("1", "2"), pair("1", "3")];
    assert_eq!(
        invalidated_in(&g, &keys, Event::new("Events.Changed", &["2"])),
        [pair("1", "2")]
    );
}

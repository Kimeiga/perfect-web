//! Charter §14 M6's gate, run.
//!
//! | # | item |
//! |---|---|
//! | 1 | `MenuChanged(store_47)` invalidates only store 47 |
//! | 2 | duplicate events are harmless |
//! | 3 | last-known-good follows declared policy |
//! | 4 | private cart resources never reach shared materialization storage |
//! | 5 | stampede tests show bounded regeneration concurrency |
//! | 6 | the graph is inspectable and serializable |
//!
//! Item 4 is decided by the compiler (`PW5101`) and its evidence is in
//! `pw-core`; what is here is the runtime half — that a fragment which passed
//! the compiler's rule keeps private data out of shared storage in fact.
//!
//! Every test is written so that the negative control is visible in the same
//! test. "Store 48 did not regenerate" means nothing without store 48 existing
//! and being regenerable, so it exists and its counter is read.

use pw_materialize::*;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

/// The graph the compiler emits for the store demo, as the compiler emits it.
///
/// A literal rather than a call into `pw-core`: this crate does not link the
/// compiler (ADR-0018/0019), and a test that built the graph in-process would
/// be testing a shape this crate invented. `tests/graph_contract.rs` is the
/// other half — it reads a graph the real `pw emit-graph` produced.
fn graph() -> Graph {
    Graph::from_json(
        r#"{
      "nodes": [
        { "path": "Events.MenuChanged", "name": "MenuChanged", "node": "event", "params": ["store"] },
        { "path": "Events.CartChanged", "name": "CartChanged", "node": "event", "params": ["session"] },
        { "path": "Resources.Menu", "name": "Menu", "node": "resource",
          "partition": "shared", "privacy": "public", "params": ["id"] },
        { "path": "Resources.Cart", "name": "Cart", "node": "resource",
          "partition": "private", "privacy": "session", "params": ["session"] },
        { "path": "store.MenuFragment", "name": "MenuFragment", "node": "materialization",
          "placement": "edge", "partition": "public", "regenerate": "on_invalidation",
          "stampede": "single_flight", "fallback": "last_known_good",
          "varies_by": ["locale", "privacy_partition"], "params": ["id"] },
        { "path": "store.CartBadge", "name": "CartBadge", "node": "materialization",
          "placement": "origin", "partition": "private", "regenerate": "on_invalidation",
          "stampede": "single_flight", "fallback": "last_known_good",
          "varies_by": ["privacy_partition"], "params": ["session"] }
      ],
      "edges": [
        { "from": "store.MenuFragment", "to": "Resources.Menu", "kind": "reads", "key": ["id"] },
        { "from": "store.MenuFragment", "to": "Events.MenuChanged",
          "kind": "invalidated_by", "key": ["id"] },
        { "from": "store.CartBadge", "to": "Resources.Cart", "kind": "reads", "key": ["session"] },
        { "from": "store.CartBadge", "to": "Events.CartChanged",
          "kind": "invalidated_by", "key": ["session"] }
      ],
      "dangling": []
    }"#,
    )
    .expect("the graph parses")
}

/// One entry per store, keyed the way the fragment declares.
fn menu_entry(store: &str) -> EntryKey {
    EntryKey::new("store.MenuFragment", &[store])
        .with("locale", "en")
        .with("partition", "public")
}

/// A materializer with the demo's fragments and three stores materialized.
fn with_three_stores() -> (Materializer, Clock, Vec<EntryKey>) {
    let clock = Clock::new();
    let m = Materializer::new(clock.clone(), "build-1");
    m.declare("store.MenuFragment", FragmentPolicy::default());
    m.declare("store.CartBadge", FragmentPolicy::default());
    let keys: Vec<EntryKey> = ["47", "48", "49"].iter().map(|s| menu_entry(s)).collect();
    for (i, k) in keys.iter().enumerate() {
        clock.advance(10);
        let body = format!("menu for store {}", ["47", "48", "49"][i]);
        assert_eq!(
            m.regenerate(k, None, || Ok(body.clone())),
            Regenerated::Fresh
        );
    }
    (m, clock, keys)
}

// --- gate item 1 ---------------------------------------------------------

#[test]
fn menu_changed_for_one_store_invalidates_that_store_only() {
    let (m, _clock, keys) = with_three_stores();
    let g = graph();

    m.command::<()>(|tx| {
        tx.execute(
            "INSERT INTO state (name, value) VALUES ('menu:47', 'new')",
            [],
        )
        .expect("state");
        Ok(vec![Event::new("Events.MenuChanged", &["47"])])
    })
    .expect("command");

    let hit = m.drain(&g, &keys);

    assert_eq!(hit.len(), 1, "exactly one entry invalidated, got {hit:?}");
    assert_eq!(hit[0].0, keys[0]);

    // The half that matters and that a "did it invalidate?" test never checks:
    // everything else is untouched, and the other two entries are real entries
    // that COULD have been invalidated.
    assert!(matches!(m.read(&keys[0]), Read::LastKnownGood(_)));
    assert!(
        matches!(m.read(&keys[1]), Read::Fresh(_)),
        "store 48 must not be affected"
    );
    assert!(
        matches!(m.read(&keys[2]), Read::Fresh(_)),
        "store 49 must not be affected"
    );
}

#[test]
fn regenerating_one_store_recomputes_one_body() {
    // The demo, stated as a count: change one menu item and exactly one
    // fragment recomputes.
    let (m, clock, keys) = with_three_stores();
    let g = graph();
    let produced = Arc::new(AtomicUsize::new(0));

    m.command::<()>(|_tx| Ok(vec![Event::new("Events.MenuChanged", &["47"])]))
        .expect("command");
    let invalidated = m.drain(&g, &keys);

    for (key, because) in &invalidated {
        clock.advance(5);
        let p = produced.clone();
        m.regenerate(key, Some(*because), || {
            p.fetch_add(1, Ordering::SeqCst);
            Ok("menu for store 47, revised".to_string())
        });
    }

    assert_eq!(
        produced.load(Ordering::SeqCst),
        1,
        "one regeneration, not three"
    );
    assert_eq!(
        m.read(&keys[0]),
        Read::Fresh("menu for store 47, revised".to_string())
    );

    // Causality and duration are recorded — charter §14 M6 task 5.
    let e = m.entry(&keys[0]).expect("entry");
    assert!(e.because.is_some(), "the entry records what caused it");
    assert_eq!(
        e.duration_ms, 0,
        "the test clock did not advance mid-produce"
    );
    assert!(e.generated_at > 0);
}

#[test]
fn an_event_with_no_arguments_reaches_every_instance() {
    // The honest reading of "the menu changed" with nothing said about which
    // menu. Included because the narrowing in the test above would look just
    // as green if arguments were ignored and every event were narrow by
    // accident of there being one entry.
    let (m, _clock, keys) = with_three_stores();
    let g = graph();

    m.command::<()>(|_tx| Ok(vec![Event::new("Events.MenuChanged", &[])]))
        .expect("command");
    let hit = m.drain(&g, &keys);

    assert_eq!(hit.len(), 3, "no argument means no narrowing, got {hit:?}");
}

// --- gate item 2 ---------------------------------------------------------

#[test]
fn a_duplicate_event_is_harmless() {
    let (m, clock, keys) = with_three_stores();
    let g = graph();
    let produced = Arc::new(AtomicUsize::new(0));

    // The same event twice, as a retrying publisher or a redelivering consumer
    // would produce.
    m.command::<()>(|_tx| {
        Ok(vec![
            Event::new("Events.MenuChanged", &["47"]),
            Event::new("Events.MenuChanged", &["47"]),
        ])
    })
    .expect("command");

    let invalidated = m.drain(&g, &keys);
    assert_eq!(
        invalidated.len(),
        1,
        "two events naming one entry coalesce to one invalidation"
    );

    for (key, because) in &invalidated {
        let p = produced.clone();
        clock.advance(1);
        m.regenerate(key, Some(*because), || {
            p.fetch_add(1, Ordering::SeqCst);
            Ok("revised".to_string())
        });
    }
    // And a third arriving after regeneration finds the entry current.
    assert_eq!(
        m.regenerate(&keys[0], None, || {
            produced.fetch_add(1, Ordering::SeqCst);
            Ok("again".to_string())
        }),
        Regenerated::AlreadyCurrent
    );
    assert_eq!(
        produced.load(Ordering::SeqCst),
        1,
        "one regeneration for three deliveries of one fact"
    );
}

#[test]
fn out_of_order_events_reach_the_same_state() {
    // Charter §14 M6 task 8. The materializer must not depend on order,
    // because the outbox promises none beyond the row id and a redelivery can
    // arrive after a later event.
    let g = graph();
    let mut final_bodies = Vec::new();
    for order in [[0usize, 1], [1, 0]] {
        let (m, clock, keys) = with_three_stores();
        let events = [
            Event::new("Events.MenuChanged", &["47"]),
            Event::new("Events.MenuChanged", &["48"]),
        ];
        m.command::<()>(|_tx| Ok(order.iter().map(|i| events[*i].clone()).collect()))
            .expect("command");
        for (key, because) in m.drain(&g, &keys) {
            clock.advance(1);
            m.regenerate(&key, Some(because), || {
                Ok(format!("body of {}", key.key[0]))
            });
        }
        final_bodies.push(
            m.all_entries()
                .into_iter()
                .map(|(k, e)| (k, e.body))
                .collect::<Vec<_>>(),
        );
    }
    assert_eq!(
        final_bodies[0], final_bodies[1],
        "the order two independent events arrive in must not change the result"
    );
}

// --- gate item 3 ---------------------------------------------------------

#[test]
fn last_known_good_is_served_only_where_the_policy_declares_it() {
    let clock = Clock::new();
    let m = Materializer::new(clock.clone(), "build-1");
    m.declare(
        "store.MenuFragment",
        FragmentPolicy {
            fallback: Fallback::LastKnownGood,
            single_flight: true,
        },
    );
    m.declare(
        "store.StrictFragment",
        FragmentPolicy {
            fallback: Fallback::None,
            single_flight: true,
        },
    );

    let lenient = menu_entry("47");
    let strict = EntryKey::new("store.StrictFragment", &["47"]);
    clock.advance(5);
    m.regenerate(&lenient, None, || Ok("good".to_string()));
    m.regenerate(&strict, None, || Ok("good".to_string()));

    m.invalidate(&lenient, 1);
    m.invalidate(&strict, 1);

    assert_eq!(m.read(&lenient), Read::LastKnownGood("good".to_string()));
    assert_eq!(
        m.read(&strict),
        Read::Stale,
        "a fragment that did not declare the fallback must not get it"
    );
    assert!(
        m.trace()
            .iter()
            .any(|t| matches!(t, Trace::ServedLastKnownGood { .. })),
        "serving stale content is recorded, not silent"
    );
}

#[test]
fn a_failed_regeneration_leaves_the_last_good_entry_in_place() {
    // Charter §14 M6 task 8, "regeneration failure". The failure mode being
    // prevented: a failed regeneration that clears the entry turns a servable
    // stale page into a missing one, so an origin blip becomes an outage.
    let clock = Clock::new();
    let m = Materializer::new(clock.clone(), "build-1");
    m.declare("store.MenuFragment", FragmentPolicy::default());
    let key = menu_entry("47");
    clock.advance(5);
    m.regenerate(&key, None, || Ok("good".to_string()));
    m.invalidate(&key, 1);

    assert_eq!(
        m.regenerate(&key, Some(1), || Err("origin timeout".to_string())),
        Regenerated::Failed
    );
    assert_eq!(
        m.read(&key),
        Read::LastKnownGood("good".to_string()),
        "a failure must not destroy what is there"
    );
    assert!(
        m.trace().iter().any(|t| matches!(t, Trace::Failed { .. })),
        "the failure is recorded"
    );
}

// --- gate item 4 (the runtime half) --------------------------------------

#[test]
fn a_private_fragment_never_shares_an_entry_between_sessions() {
    // The compiler refuses a shared fragment that depends on session data
    // (`PW5101`). This is the other half: a fragment that IS private must give
    // two sessions two entries, or the compiler's rule protects nothing.
    let clock = Clock::new();
    let m = Materializer::new(clock.clone(), "build-1");
    m.declare("store.CartBadge", FragmentPolicy::default());

    let a = EntryKey::new("store.CartBadge", &["session-a"]).with("partition", "private");
    let b = EntryKey::new("store.CartBadge", &["session-b"]).with("partition", "private");

    clock.advance(1);
    m.regenerate(&a, None, || Ok("3 items".to_string()));
    assert_eq!(
        m.read(&b),
        Read::Missing,
        "session B is not served A's entry"
    );

    clock.advance(1);
    m.regenerate(&b, None, || Ok("0 items".to_string()));
    assert_eq!(m.read(&a), Read::Fresh("3 items".to_string()));
    assert_eq!(m.read(&b), Read::Fresh("0 items".to_string()));
}

#[test]
fn a_cart_event_reaches_one_session_only() {
    let clock = Clock::new();
    let m = Materializer::new(clock.clone(), "build-1");
    m.declare("store.CartBadge", FragmentPolicy::default());
    let g = graph();
    let keys: Vec<EntryKey> = ["session-a", "session-b"]
        .iter()
        .map(|s| EntryKey::new("store.CartBadge", &[s]).with("partition", "private"))
        .collect();
    for k in &keys {
        clock.advance(1);
        m.regenerate(k, None, || Ok("items".to_string()));
    }

    m.command::<()>(|_tx| Ok(vec![Event::new("Events.CartChanged", &["session-a"])]))
        .expect("command");
    let hit = m.drain(&g, &keys);

    assert_eq!(hit.len(), 1);
    assert_eq!(hit[0].0, keys[0]);
    assert!(matches!(m.read(&keys[1]), Read::Fresh(_)));
}

// --- gate item 5 ---------------------------------------------------------

#[test]
fn concurrent_readers_of_one_missing_entry_regenerate_it_once() {
    // The stampede. Eight threads find the same entry missing at the same
    // moment; `stampede single_flight` admits one and the other seven take its
    // answer.
    let clock = Clock::new();
    let m = Arc::new(Materializer::new(clock.clone(), "build-1"));
    m.declare("store.MenuFragment", FragmentPolicy::default());
    let key = menu_entry("47");
    let produced = Arc::new(AtomicUsize::new(0));
    let gate = Arc::new((std::sync::Mutex::new(false), std::sync::Condvar::new()));

    let outcomes: Vec<Regenerated> = std::thread::scope(|s| {
        let handles: Vec<_> = (0..8)
            .map(|_| {
                let (m, key, produced, gate) =
                    (m.clone(), key.clone(), produced.clone(), gate.clone());
                s.spawn(move || {
                    m.regenerate(&key, None, || {
                        produced.fetch_add(1, Ordering::SeqCst);
                        // Hold the flight open until every other thread has had
                        // a chance to arrive. Without this the winner could
                        // finish before the others start and the test would
                        // pass with no coalescing at all — green, and measuring
                        // nothing.
                        let (lock, cv) = &*gate;
                        let mut open = lock.lock().expect("gate");
                        while !*open {
                            let r = cv
                                .wait_timeout(open, std::time::Duration::from_millis(500))
                                .expect("wait");
                            open = r.0;
                            if r.1.timed_out() {
                                break;
                            }
                        }
                        Ok("menu".to_string())
                    })
                })
            })
            .collect();

        // Let the waiters pile up, then release the producer.
        std::thread::sleep(std::time::Duration::from_millis(50));
        {
            let (lock, cv) = &*gate;
            *lock.lock().expect("gate") = true;
            cv.notify_all();
        }
        handles
            .into_iter()
            .map(|h| h.join().expect("join"))
            .collect()
    });

    assert_eq!(
        produced.load(Ordering::SeqCst),
        1,
        "eight readers, one regeneration"
    );
    assert_eq!(
        outcomes
            .iter()
            .filter(|o| **o == Regenerated::Fresh)
            .count(),
        1
    );
    let coalesced = outcomes
        .iter()
        .filter(|o| **o == Regenerated::Coalesced)
        .count();
    assert!(
        coalesced >= 1,
        "at least one reader waited rather than duplicating the work; got {outcomes:?}"
    );
    assert_eq!(m.read(&key), Read::Fresh("menu".to_string()));
}

#[test]
fn without_single_flight_the_same_work_is_done_more_than_once() {
    // The negative control for the test above. A single-flight test that never
    // shows the unbounded case is asserting that a number is 1 without
    // establishing it could have been anything else.
    let clock = Clock::new();
    let m = Arc::new(Materializer::new(clock.clone(), "build-1"));
    m.declare(
        "store.Unbounded",
        FragmentPolicy {
            fallback: Fallback::LastKnownGood,
            single_flight: false,
        },
    );
    let key = EntryKey::new("store.Unbounded", &["47"]);
    let produced = Arc::new(AtomicUsize::new(0));
    let start = Arc::new(std::sync::Barrier::new(8));

    std::thread::scope(|s| {
        for _ in 0..8 {
            let (m, key, produced, start) =
                (m.clone(), key.clone(), produced.clone(), start.clone());
            s.spawn(move || {
                start.wait();
                m.regenerate(&key, None, || {
                    produced.fetch_add(1, Ordering::SeqCst);
                    std::thread::sleep(std::time::Duration::from_millis(20));
                    Ok("menu".to_string())
                });
            });
        }
    });

    assert!(
        produced.load(Ordering::SeqCst) > 1,
        "without single_flight the work is duplicated — this is what the \
         previous test shows is prevented, and it must be demonstrably \
         possible for that to mean anything"
    );
}

// --- the transaction (charter §14 M6 task 4) -----------------------------

#[test]
fn a_rolled_back_command_emits_nothing() {
    // The property ADR-0019 exists for. If the state change does not happen,
    // the event must not exist — otherwise a fragment regenerates from state
    // that was never committed.
    let m = Materializer::new(Clock::new(), "build-1");
    let r: Result<Vec<i64>, &str> = m.command(|tx| {
        tx.execute(
            "INSERT INTO state (name, value) VALUES ('menu:47', 'new')",
            [],
        )
        .expect("state");
        Err("the command failed after writing")
    });

    assert!(r.is_err());
    assert_eq!(m.state("menu:47"), None, "the state change rolled back");
    assert!(m.pending().is_empty(), "and so did the event");
}

#[test]
fn a_command_that_panics_leaves_neither_the_state_nor_the_event() {
    // Charter §14 M6 task 8, "materializer crash" — the crash placed exactly
    // where it hurts, between the state write and the commit.
    let m = Materializer::new(Clock::new(), "build-1");
    let caught = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        m.command::<()>(|tx| {
            tx.execute(
                "INSERT INTO state (name, value) VALUES ('menu:47', 'new')",
                [],
            )
            .expect("state");
            panic!("crash between the write and the commit");
        })
    }));

    assert!(caught.is_err(), "the panic propagated");
    assert_eq!(m.state("menu:47"), None);
    assert!(m.pending().is_empty());
}

#[test]
fn a_committed_command_leaves_both() {
    // The positive control. Without it the two tests above pass on a
    // `command` that never writes anything at all.
    let m = Materializer::new(Clock::new(), "build-1");
    let ids = m
        .command::<()>(|tx| {
            tx.execute(
                "INSERT INTO state (name, value) VALUES ('menu:47', 'new')",
                [],
            )
            .expect("state");
            Ok(vec![Event::new("Events.MenuChanged", &["47"])])
        })
        .expect("command");

    assert_eq!(ids.len(), 1);
    assert_eq!(m.state("menu:47").as_deref(), Some("new"));
    assert_eq!(m.pending().len(), 1);
    assert_eq!(m.pending()[0].event.args, vec!["47".to_string()]);
}

#[test]
fn an_event_nothing_listens_for_is_recorded_rather_than_dropped() {
    let m = Materializer::new(Clock::new(), "build-1");
    let g = graph();
    m.command::<()>(|_tx| Ok(vec![Event::new("Events.PromotionChanged", &["47"])]))
        .expect("command");

    assert!(m.drain(&g, &[]).is_empty());
    assert!(
        m.trace()
            .iter()
            .any(|t| matches!(t, Trace::NoSubscriber { .. })),
        "an event with no subscriber means a missing edge or a pointless \
         emitter, and both are worth seeing"
    );
    assert!(m.pending().is_empty(), "and it is still consumed");
}

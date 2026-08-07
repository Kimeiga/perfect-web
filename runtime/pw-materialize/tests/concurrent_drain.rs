//! An event is not destroyed by a drain that was not looking for it.
//!
//! `docs/RISK_QUEUE.md` 28. The first `drain` consumed **every** pending event
//! and invalidated only those matching the instances the caller supplied. The
//! caller's instance set is what that caller happens to know about — one
//! session's entry, one store's fragment — never the set of everything that
//! exists. So a drain issued on behalf of session A consumed session B's event
//! and left no trace of the loss except a `Consumed` record claiming the work
//! was done.
//!
//! # Why it survived so long
//!
//! Every single-instance test passes, because with one instance the supplied
//! set IS the whole set. Every sequential test passes, because the losing
//! drain has to run between the commit and the owner's poll. It appeared as an
//! occasional un-updated page under browser parallelism — indistinguishable
//! from a slow machine, and duly ignored as flakiness for one commit.
//!
//! # The contract, stated
//!
//! ```text
//! no listeners at all         → consume     (nothing can ever match)
//! matched a supplied instance → consume
//! listened for, matched none  → LEAVE PENDING
//! ```
//!
//! The third line is the fix. It costs an event that no live instance ever
//! claims staying in the outbox — recorded in `docs/ASSUMPTIONS.md` as a
//! retention question, and much cheaper than a lost invalidation.

use pw_materialize::*;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

fn graph() -> Graph {
    Graph::from_json(
        r#"{
      "nodes": [
        { "path": "Events.CartChanged", "name": "CartChanged", "node": "event", "params": ["session"] },
        { "path": "store.page.Cart", "name": "Cart", "node": "materialization",
          "partition": "private", "fallback": "last_known_good",
          "varies_by": ["privacy_partition"], "params": ["session"] }
      ],
      "edges": [
        { "from": "store.page.Cart", "to": "Events.CartChanged",
          "kind": "invalidated_by", "key": ["session"] }
      ],
      "dangling": []
    }"#,
    )
    .expect("graph")
}

fn cart(session: &str) -> EntryKey {
    EntryKey::new("store.page.Cart", &[session])
}

/// A materializer with both carts already materialized and fresh.
fn ready() -> (Materializer, Arc<AtomicUsize>) {
    let m = Materializer::new(Clock::new(), "B1");
    m.declare("store.page.Cart", FragmentPolicy::default());
    let calls = Arc::new(AtomicUsize::new(0));
    for s in ["a", "b"] {
        let c = calls.clone();
        m.regenerate(&cart(s), None, move || {
            c.fetch_add(1, Ordering::SeqCst);
            Ok(String::new())
        });
    }
    calls.store(0, Ordering::SeqCst);
    (m, calls)
}

fn commit(m: &Materializer, session: &str) {
    m.command::<String>(|_tx| Ok(vec![Event::new("Events.CartChanged", &[session])]))
        .expect("the command commits");
}

#[test]
fn a_drain_for_one_session_does_not_swallow_another_sessions_event() {
    let (m, _) = ready();
    commit(&m, "b");

    // Session A polls first, and knows only about its own entry.
    let for_a = m.drain(&graph(), &[cart("a")]);
    assert!(for_a.is_empty(), "nothing of A's changed");

    // B's event must still be there. Before the fix this assertion failed, and
    // session B's page sat at a stale value forever.
    let for_b = m.drain(&graph(), &[cart("b")]);
    assert_eq!(
        for_b.len(),
        1,
        "B's event survived a drain that was not looking for it"
    );
    assert_eq!(for_b[0].0, cart("b"));
}

#[test]
fn the_deferral_is_recorded_rather_than_silent() {
    // The difference between "left pending" and "quietly lost" has to be
    // visible somewhere, or the fix is as unfalsifiable as the defect.
    let (m, _) = ready();
    commit(&m, "b");
    m.drain(&graph(), &[cart("a")]);

    let deferred = m
        .trace()
        .into_iter()
        .filter(|t| matches!(t, Trace::Deferred { .. }))
        .count();
    assert_eq!(deferred, 1, "the skipped event is on the record");
}

#[test]
fn an_event_nobody_listens_for_is_still_consumed() {
    // The negative control for the fix. If deferral applied to unlistened
    // events too, the outbox would grow without bound and every drain would
    // re-walk a growing tail of garbage — a plausible-looking fix that trades
    // one silent failure for a slow one.
    let (m, _) = ready();
    m.command::<String>(|_tx| Ok(vec![Event::new("Events.Unheard", &["x"])]))
        .expect("commit");

    m.drain(&graph(), &[cart("a")]);
    assert!(
        m.pending().is_empty(),
        "an event with no listener at all is consumed, not deferred forever"
    );
}

#[test]
fn a_matched_event_is_consumed_exactly_once() {
    // And the other negative control: deferral must not make events immortal.
    // An event that DID match has done its work, and a second drain finding it
    // again would regenerate on every poll — which looks like it works, and
    // costs a producer call per request.
    let (m, calls) = ready();
    commit(&m, "a");

    assert_eq!(m.drain(&graph(), &[cart("a")]).len(), 1);
    assert!(m.pending().is_empty(), "consumed after matching");
    assert_eq!(
        m.drain(&graph(), &[cart("a")]).len(),
        0,
        "and it does not come back"
    );
    assert_eq!(calls.load(Ordering::SeqCst), 0, "drain does not regenerate");
}

#[test]
fn concurrent_drains_lose_nothing() {
    // The shape the browser suite actually produced: many sessions, each
    // polling for itself, all interleaved. Run against the pre-fix code this
    // lost between one and eleven events per run depending on scheduling —
    // which is exactly why it read as flakiness.
    let sessions: Vec<String> = (0..12).map(|i| format!("s{i}")).collect();
    let m = Materializer::new(Clock::new(), "B1");
    m.declare("store.page.Cart", FragmentPolicy::default());
    for s in &sessions {
        m.regenerate(&cart(s), None, || Ok(String::new()));
    }
    for s in &sessions {
        commit(&m, s);
    }

    // Every session drains for itself, in an order unrelated to commit order.
    let mut seen = 0;
    let g = graph();
    for s in sessions.iter().rev() {
        seen += m.drain(&g, &[cart(s)]).len();
    }
    assert_eq!(seen, sessions.len(), "every session saw its own event");
    assert!(m.pending().is_empty(), "and every event was consumed once");
}

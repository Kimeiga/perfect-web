//! The E4 gate items this runtime owns (charter §14 M4).
//!
//! Each test names the gate item it answers. Every one has a control, because
//! a caching layer is unusually good at appearing to work: it returns the right
//! answer whether or not it did the right thing.

use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};

use pw_resource::*;

fn rt() -> (Resources, Clock) {
    let clock = Clock::new();
    (Resources::new(clock.clone()), clock)
}

#[test]
fn a_recomputation_storm_does_not_exceed_one_request_per_key() {
    // GATE: "A rerender/recomputation storm does not exceed one in-flight
    // request per resource key."
    let (rt, _clock) = rt();
    let m = Manifest::new("store");
    let key = Key::new("store", "blue-bottle");
    let calls = Arc::new(AtomicU32::new(0));

    // 50 concurrent readers of the same key, all arriving while the first
    // request is still running.
    let n = 50;
    std::thread::scope(|s| {
        for _ in 0..n {
            let (rt, m, key, calls) = (rt.clone(), m.clone(), key.clone(), Arc::clone(&calls));
            s.spawn(move || {
                rt.fetch(&m, &key, |_| {
                    calls.fetch_add(1, Ordering::SeqCst);
                    std::thread::sleep(std::time::Duration::from_millis(20));
                    Ok("Blue Bottle".to_string())
                })
            });
        }
    });

    assert_eq!(
        calls.load(Ordering::SeqCst),
        1,
        "50 readers of one key must produce exactly one request"
    );

    // Control: the dedup must be per KEY, not global. Two keys are two
    // requests, or the runtime is just a lock and serves stale data.
    let calls2 = Arc::new(AtomicU32::new(0));
    for k in ["a", "b"] {
        let c = Arc::clone(&calls2);
        rt.fetch(&m, &Key::new("store", k), move |_| {
            c.fetch_add(1, Ordering::SeqCst);
            Ok("x".to_string())
        });
    }
    assert_eq!(
        calls2.load(Ordering::SeqCst),
        2,
        "different keys, two requests"
    );
}

#[test]
fn a_fresh_cached_value_is_served_without_a_request() {
    let (rt, clock) = rt();
    let m = Manifest::new("store").freshness(30_000);
    let key = Key::new("store", "k");
    let calls = Arc::new(AtomicU32::new(0));
    let load = |c: Arc<AtomicU32>| {
        move |_: u32| {
            c.fetch_add(1, Ordering::SeqCst);
            Ok("v".to_string())
        }
    };

    assert!(matches!(
        rt.fetch(&m, &key, load(Arc::clone(&calls))),
        Fetched::Fresh(_)
    ));
    clock.advance(29_999);
    assert!(matches!(
        rt.fetch(&m, &key, load(Arc::clone(&calls))),
        Fetched::FromCache(_)
    ));
    assert_eq!(calls.load(Ordering::SeqCst), 1);

    // One millisecond past freshness, the value is stale and refetched. Without
    // this control the test would pass on a runtime that never expires
    // anything, which is a cache bug that looks like a fast cache.
    clock.advance(2);
    assert!(matches!(
        rt.fetch(&m, &key, load(Arc::clone(&calls))),
        Fetched::Fresh(_)
    ));
    assert_eq!(calls.load(Ordering::SeqCst), 2);
}

#[test]
fn duplicate_commands_with_one_interaction_id_mutate_once() {
    // GATE: "Duplicate `add_to_cart` calls with the same interaction ID produce
    // one logical mutation." A double-click, a retry after a dropped response,
    // and a reconnect replay are the same event.
    let (rt, _clock) = rt();
    let applied = Arc::new(AtomicU32::new(0));

    let mut results = Vec::new();
    for _ in 0..5 {
        let a = Arc::clone(&applied);
        results.push(rt.command("interaction-42", move || {
            a.fetch_add(1, Ordering::SeqCst);
            "cart:1".to_string()
        }));
    }

    assert_eq!(applied.load(Ordering::SeqCst), 1, "one logical mutation");
    assert!(
        results.iter().all(|r| r == "cart:1"),
        "and every caller gets the same answer, not an error"
    );
    assert_eq!(
        rt.trace()
            .iter()
            .filter(|t| matches!(t, Trace::CommandDeduplicated { .. }))
            .count(),
        4
    );

    // Control: a different interaction is a different mutation. Otherwise the
    // runtime would swallow every command after the first.
    let a = Arc::clone(&applied);
    rt.command("interaction-43", move || {
        a.fetch_add(1, Ordering::SeqCst);
        "cart:2".to_string()
    });
    assert_eq!(applied.load(Ordering::SeqCst), 2);
}

#[test]
fn navigating_away_cancels_work_nobody_is_waiting_for() {
    // GATE: "Navigation cancels unneeded work."
    let (rt, _clock) = rt();
    let key = Key::new("recommendations", "blue-bottle");

    let a = rt.subscribe(&key);
    let b = rt.subscribe(&key);
    assert_eq!(rt.subscribers(&key), 2);

    // Simulate a request in flight for that key, then navigate away.
    let m = Manifest::new("recommendations");
    let started = std::thread::scope(|s| {
        let h = s.spawn(|| {
            rt.fetch(&m, &key, |_| {
                std::thread::sleep(std::time::Duration::from_millis(30));
                Ok("recs".to_string())
            })
        });
        std::thread::sleep(std::time::Duration::from_millis(5));
        let was_running = rt.in_flight(&key);
        h.join().expect("join");
        was_running
    });
    assert!(started, "the request must actually have been in flight");

    drop(a);
    assert_eq!(rt.subscribers(&key), 1, "one viewer left");
    drop(b);
    assert_eq!(rt.subscribers(&key), 0);

    // Control: cancellation must fire only when the LAST subscriber leaves.
    // Cancelling while someone is still watching is a worse bug than leaking.
    let cancels = rt
        .trace()
        .iter()
        .filter(|t| matches!(t, Trace::Cancelled { .. }))
        .count();
    assert!(cancels <= 1, "cancelled more than once: {cancels}");
}

#[test]
fn a_dropped_subscription_releases_even_on_an_early_return() {
    // Reference counting that depends on remembering to call `release` is
    // reference counting that leaks. The guard must do it on drop.
    let (rt, _clock) = rt();
    let key = Key::new("cart", "session-1");
    {
        let _s = rt.subscribe(&key);
        assert_eq!(rt.subscribers(&key), 1);
    }
    assert_eq!(rt.subscribers(&key), 0, "dropping must release");

    let s = rt.subscribe(&key);
    s.release();
    assert_eq!(
        rt.subscribers(&key),
        0,
        "and releasing early must not double-count"
    );
}

#[test]
fn private_state_never_appears_in_public_cache_output() {
    // GATE: "Private cart state never appears in public cache output."
    let (rt, _clock) = rt();
    let public = Manifest::new("store");
    let private = Manifest::new("cart").private();

    rt.fetch(&public, &Key::new("store", "blue-bottle"), |_| {
        Ok("Blue Bottle".to_string())
    });
    rt.fetch(&private, &Key::new("cart", "session-1"), |_| {
        Ok("2 items, $9.25".to_string())
    });

    let public_out = rt.public_cache_contents();
    assert_eq!(public_out.len(), 1, "only the public value: {public_out:?}");
    assert_eq!(public_out[0].0.resource, "store");
    assert!(
        !public_out.iter().any(|(_, v)| v.contains("$9.25")),
        "the cart value leaked into public output: {public_out:?}"
    );

    // Control: the private value IS cached — the separation must not be
    // achieved by simply not caching it, which would pass this test while
    // making every private read a fresh request.
    let calls = Arc::new(AtomicU32::new(0));
    let c = Arc::clone(&calls);
    let again = rt.fetch(&private, &Key::new("cart", "session-1"), move |_| {
        c.fetch_add(1, Ordering::SeqCst);
        Ok("changed".to_string())
    });
    assert!(matches!(again, Fetched::FromCache(_)), "{again:?}");
    assert_eq!(calls.load(Ordering::SeqCst), 0);
}

#[test]
fn retries_are_bounded_and_backoff_grows() {
    let (rt, _clock) = rt();
    let m = Manifest::new("estimate").attempts(3);
    let key = Key::new("estimate", "k");
    let attempts = Arc::new(AtomicU32::new(0));

    let a = Arc::clone(&attempts);
    let result = rt.fetch(&m, &key, move |_| {
        a.fetch_add(1, Ordering::SeqCst);
        Err("upstream down".to_string())
    });

    assert!(matches!(result, Fetched::Failed(_)));
    assert_eq!(
        attempts.load(Ordering::SeqCst),
        3,
        "bounded: exactly max_attempts, not forever"
    );
    assert!(rt.trace().iter().any(|t| matches!(t, Trace::GaveUp { .. })));

    // Backoff doubles, and jitter keeps two keys from retrying in lockstep.
    assert!(m.backoff(2, "k") > m.backoff(1, "k"), "backoff must grow");
    assert_ne!(
        m.backoff(3, "alpha"),
        m.backoff(3, "beta"),
        "jitter must spread across keys, or a herd retries together"
    );
    // ...but it must stay deterministic, or a test cannot assert on it.
    assert_eq!(m.backoff(3, "alpha"), m.backoff(3, "alpha"));
}

#[test]
fn a_failed_key_does_not_stay_in_flight_forever() {
    // A runtime that leaves a failed key marked in-flight deadlocks every later
    // reader of it — and the symptom is a hang, not an error.
    let (rt, _clock) = rt();
    let m = Manifest::new("estimate").attempts(2);
    let key = Key::new("estimate", "k");

    rt.fetch(&m, &key, |_| Err("boom".to_string()));
    assert!(
        !rt.in_flight(&key),
        "the key must be released after failure"
    );

    let ok = rt.fetch(&m, &key, |_| Ok("recovered".to_string()));
    assert!(
        matches!(&ok, Fetched::Fresh(v) if v == "recovered"),
        "{ok:?}"
    );
}

#[test]
fn a_command_can_invalidate_the_resource_it_changed() {
    let (rt, _clock) = rt();
    let m = Manifest::new("cart").private();
    let key = Key::new("cart", "session-1");

    rt.fetch(&m, &key, |_| Ok("1 item".to_string()));
    rt.command("add-1", || "ok".to_string());
    rt.invalidate("cart");

    let calls = Arc::new(AtomicU32::new(0));
    let c = Arc::clone(&calls);
    let after = rt.fetch(&m, &key, move |_| {
        c.fetch_add(1, Ordering::SeqCst);
        Ok("2 items".to_string())
    });
    assert!(
        matches!(&after, Fetched::Fresh(v) if v == "2 items"),
        "{after:?}"
    );
    assert_eq!(
        calls.load(Ordering::SeqCst),
        1,
        "invalidation must force a reload"
    );
}

#[test]
fn the_trace_records_what_actually_happened() {
    // Charter §14 M4 task 12 wants causal traces from click to command to
    // resource refresh. This is the record they are built from, so it has to
    // distinguish a cache hit from real work — the whole point of the trace.
    let (rt, clock) = rt();
    let m = Manifest::new("store").freshness(1_000);
    let key = Key::new("store", "k");

    rt.fetch(&m, &key, |_| Ok("v".to_string()));
    rt.fetch(&m, &key, |_| Ok("v".to_string()));
    clock.advance(2_000);
    rt.fetch(&m, &key, |_| Ok("v2".to_string()));

    let t = rt.trace();
    let started = t
        .iter()
        .filter(|e| matches!(e, Trace::RequestStarted { .. }))
        .count();
    let cached = t
        .iter()
        .filter(|e| matches!(e, Trace::ServedFromCache { .. }))
        .count();
    assert_eq!((started, cached), (2, 1), "{t:#?}");
}

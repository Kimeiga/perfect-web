//! ADR-0027: assert caller outcomes under controlled overlap, not just counters.
//! Timeouts below guard a deadlocked test; channels establish the schedule.

use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};

use pw_resource::{Clock, Fetched, Key, Manifest, Resources, Trace};

const GUARD: Duration = Duration::from_secs(10);

fn until(mut predicate: impl FnMut() -> bool) {
    let deadline = Instant::now() + GUARD;
    while !predicate() {
        assert!(Instant::now() < deadline, "operation did not reach its scheduled state");
        thread::yield_now();
    }
}

fn joined_fetch(stale: bool, failure: bool) {
    let clock = Clock::new();
    let rt = Resources::new(clock.clone());
    let manifest = Manifest::new("query").freshness(1);
    let key = Key::new("query", "one");
    if stale {
        assert_eq!(rt.fetch(&manifest, &key, |_| Ok("old".into())), Fetched::Fresh("old".into()));
        clock.advance(1);
    }
    thread::scope(|scope| {
        let (started_tx, started_rx) = mpsc::channel();
        let (release_tx, release_rx) = mpsc::channel();
        let leader = {
            let rt = rt.clone();
            let manifest = manifest.clone();
            let key = key.clone();
            scope.spawn(move || rt.fetch(&manifest, &key, |_| {
                started_tx.send(()).unwrap();
                release_rx.recv_timeout(GUARD).expect("release leader");
                if failure { Err("upstream failed".into()) } else { Ok("new".into()) }
            }))
        };
        started_rx.recv_timeout(GUARD).unwrap();
        let follower = scope.spawn(|| rt.fetch(&manifest, &key, |_| panic!("duplicate load")));
        until(|| rt.trace().iter().any(|event| matches!(event, Trace::RequestDeduplicated { .. })));
        release_tx.send(()).unwrap();
        let first = leader.join().unwrap();
        let second = follower.join().unwrap();
        if failure {
            assert_eq!(first, Fetched::Failed("upstream failed".into()));
            assert_eq!(second, Fetched::Failed("upstream failed".into()));
        } else {
            assert_eq!(first, Fetched::Fresh("new".into()));
            assert_eq!(second, Fetched::Deduplicated("new".into()));
        }
    });
}

#[test]
fn duplicate_cold_fetch_receives_the_real_result() {
    joined_fetch(false, false);
}

#[test]
fn duplicate_refresh_does_not_receive_the_expired_value() {
    joined_fetch(true, false);
}

#[test]
fn duplicate_fetch_receives_the_same_failure() {
    joined_fetch(false, true);
}

#[test]
fn overlapping_commands_execute_once_and_share_the_result() {
    let rt = Resources::new(Clock::new());
    let applied = AtomicUsize::new(0);
    thread::scope(|scope| {
        let (started_tx, started_rx) = mpsc::channel();
        let (release_tx, release_rx) = mpsc::channel();
        let leader = {
            let rt = rt.clone();
            let applied = &applied;
            scope.spawn(move || rt.command("interaction", || {
                applied.fetch_add(1, Ordering::SeqCst);
                started_tx.send(()).unwrap();
                release_rx.recv_timeout(GUARD).expect("release command");
                "committed".into()
            }))
        };
        started_rx.recv_timeout(GUARD).unwrap();
        let follower = scope.spawn(|| rt.command("interaction", || {
            applied.fetch_add(1, Ordering::SeqCst);
            "second mutation".into()
        }));
        until(|| applied.load(Ordering::SeqCst) > 1 || rt.trace().iter().any(|event| matches!(event, Trace::CommandDeduplicated { .. })));
        release_tx.send(()).unwrap();
        assert_eq!(leader.join().unwrap(), "committed");
        assert_eq!(follower.join().unwrap(), "committed");
        assert_eq!(applied.load(Ordering::SeqCst), 1);
    });
}

#[test]
fn an_invalidated_completion_cannot_remove_or_overwrite_its_replacement() {
    let rt = Resources::new(Clock::new());
    let manifest = Manifest::new("query");
    let key = Key::new("query", "one");
    thread::scope(|scope| {
        let (old_started_tx, old_started_rx) = mpsc::channel();
        let (old_release_tx, old_release_rx) = mpsc::channel();
        let old = {
            let rt = rt.clone();
            let manifest = manifest.clone();
            let key = key.clone();
            scope.spawn(move || rt.fetch(&manifest, &key, |_| {
                old_started_tx.send(()).unwrap();
                old_release_rx.recv_timeout(GUARD).unwrap();
                Ok("obsolete".into())
            }))
        };
        old_started_rx.recv_timeout(GUARD).unwrap();
        rt.invalidate("query");
        let (new_started_tx, new_started_rx) = mpsc::channel();
        let (new_release_tx, new_release_rx) = mpsc::channel();
        let new = {
            let rt = rt.clone();
            let manifest = manifest.clone();
            let key = key.clone();
            scope.spawn(move || rt.fetch(&manifest, &key, |_| {
                new_started_tx.send(()).unwrap();
                new_release_rx.recv_timeout(GUARD).unwrap();
                Ok("replacement".into())
            }))
        };
        // On the defective implementation, the replacement returns a fabricated
        // duplicate without running. A timeout fails the test rather than hangs it.
        let replacement_started = new_started_rx.recv_timeout(GUARD);
        old_release_tx.send(()).unwrap();
        let old_result = old.join().unwrap();
        let still_running = rt.in_flight(&key);
        let _ = new_release_tx.send(());
        let new_result = new.join().unwrap();
        assert!(replacement_started.is_ok(), "replacement was not admitted");
        assert!(matches!(old_result, Fetched::Failed(_)), "{old_result:?}");
        assert!(still_running, "old completion removed the replacement's slot");
        assert_eq!(new_result, Fetched::Fresh("replacement".into()));
        assert_eq!(rt.fetch(&manifest, &key, |_| panic!("replacement was not cached")), Fetched::FromCache("replacement".into()));
    });
}

#[test]
fn final_release_wakes_waiters_and_fences_the_still_running_callback() {
    let rt = Resources::new(Clock::new());
    let manifest = Manifest::new("query");
    let key = Key::new("query", "one");
    let first_subscription = rt.subscribe(&key);
    let last_subscription = rt.subscribe(&key);
    thread::scope(|scope| {
        let (started_tx, started_rx) = mpsc::channel();
        let (release_tx, release_rx) = mpsc::channel();
        let leader = {
            let rt = rt.clone();
            let manifest = manifest.clone();
            let key = key.clone();
            scope.spawn(move || rt.fetch(&manifest, &key, |_| {
                started_tx.send(()).unwrap();
                release_rx.recv_timeout(GUARD).unwrap();
                Ok("late".into())
            }))
        };
        started_rx.recv_timeout(GUARD).unwrap();
        let (done_tx, done_rx) = mpsc::channel();
        let follower = scope.spawn(|| {
            let outcome = rt.fetch(&manifest, &key, |_| panic!("duplicate load"));
            done_tx.send(outcome).unwrap();
        });
        until(|| rt.trace().iter().any(|event| matches!(event, Trace::RequestDeduplicated { .. })));
        drop(first_subscription);
        assert_eq!(rt.subscribers(&key), 1);
        assert!(rt.in_flight(&key));
        drop(last_subscription);
        // Observe the waiter before allowing the arbitrary callback to finish.
        let follower_result = done_rx.recv_timeout(GUARD);
        release_tx.send(()).unwrap();
        let leader_result = leader.join().unwrap();
        follower.join().unwrap();
        assert!(matches!(follower_result, Ok(Fetched::Failed(_))), "{follower_result:?}");
        assert!(matches!(leader_result, Fetched::Failed(_)), "{leader_result:?}");
        assert!(!rt.in_flight(&key));
        assert!(rt.public_cache_contents().is_empty());
        assert_eq!(rt.trace().iter().filter(|event| matches!(event, Trace::Cancelled { .. })).count(), 1);
    });
}

#[test]
fn a_panicking_loader_does_not_poison_or_strand_the_key() {
    let rt = Resources::new(Clock::new());
    let manifest = Manifest::new("query");
    let key = Key::new("query", "one");
    assert!(catch_unwind(AssertUnwindSafe(|| rt.fetch(&manifest, &key, |_| panic!("loader panic")))).is_err());
    assert!(!rt.in_flight(&key));
    assert_eq!(rt.fetch(&manifest, &key, |_| Ok("recovered".into())), Fetched::Fresh("recovered".into()));
}

#[test]
fn a_panicking_command_is_not_silently_reexecuted() {
    let rt = Resources::new(Clock::new());
    let applied = AtomicUsize::new(0);
    assert!(catch_unwind(AssertUnwindSafe(|| rt.command("uncertain", || {
        applied.fetch_add(1, Ordering::SeqCst);
        panic!("a side effect may already have happened")
    }))).is_err());
    assert!(catch_unwind(AssertUnwindSafe(|| rt.command("uncertain", || {
        applied.fetch_add(1, Ordering::SeqCst);
        "unsafe replay".into()
    }))).is_err());
    assert_eq!(applied.load(Ordering::SeqCst), 1);
    assert_eq!(rt.command("independent", || "ok".into()), "ok");
}

#[test]
fn contradictory_privacy_cannot_reuse_a_private_cached_value() {
    let rt = Resources::new(Clock::new());
    let key = Key::new("query", "one");
    let private = Manifest::new("query").private();
    let public = Manifest::new("query");
    assert_eq!(rt.fetch(&private, &key, |_| Ok("private fixture".into())), Fetched::Fresh("private fixture".into()));
    assert!(matches!(rt.fetch(&public, &key, |_| panic!("contradictory manifest")), Fetched::Failed(_)));
    assert!(rt.public_cache_contents().is_empty());
}

#[test]
fn a_same_thread_recursive_fetch_fails_instead_of_waiting_for_itself() {
    let rt = Resources::new(Clock::new());
    let manifest = Manifest::new("query");
    let key = Key::new("query", "one");
    assert_eq!(rt.fetch(&manifest, &key, |_| {
        assert!(matches!(rt.fetch(&manifest, &key, |_| panic!("recursive load")), Fetched::Failed(_)));
        Ok("outer".into())
    }), Fetched::Fresh("outer".into()));
}

#[test]
fn independent_callbacks_do_not_run_under_the_global_lock() {
    let rt = Resources::new(Clock::new());
    assert_eq!(rt.command("outer", || rt.command("inner", || "ok".into())), "ok");
    let manifest = Manifest::new("query");
    assert_eq!(rt.fetch(&manifest, &Key::new("query", "outer"), |_| {
        assert_eq!(rt.fetch(&manifest, &Key::new("query", "inner"), |_| Ok("inner".into())), Fetched::Fresh("inner".into()));
        Ok("outer".into())
    }), Fetched::Fresh("outer".into()));
}

#[test]
fn the_test_clock_never_wraps_backwards() {
    let clock = Clock::new();
    clock.advance(u64::MAX);
    clock.advance(1);
    assert_eq!(clock.now(), u64::MAX);
}

#[test]
fn backoff_saturates_instead_of_wrapping_or_panicking() {
    let mut manifest = Manifest::new("query");
    manifest.retry_base = u64::MAX;
    assert_eq!(manifest.backoff(1, "one"), u64::MAX);
    assert_eq!(manifest.backoff(u32::MAX, "one"), u64::MAX);
}

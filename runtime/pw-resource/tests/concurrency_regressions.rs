//! Adversarial schedules against the real synchronous resource runtime.
//! Channels order work; the wall-clock deadline only detects a stuck test.
use pw_resource::*;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, mpsc};
use std::time::{Duration, Instant};

fn runtime() -> (Resources, Clock, Manifest, Key) {
    let clock = Clock::new();
    (
        Resources::new(clock.clone()),
        clock,
        Manifest::new("items"),
        Key::new("items", "1"),
    )
}
fn wait_for(mut predicate: impl FnMut() -> bool) {
    let deadline = Instant::now() + Duration::from_secs(5);
    while !predicate() {
        assert!(
            Instant::now() < deadline,
            "test schedule did not make progress"
        );
        std::thread::yield_now();
    }
}
fn successful(value: &Fetched) -> bool {
    matches!(
        value,
        Fetched::Fresh(_) | Fetched::FromCache(_) | Fetched::Deduplicated(_)
    )
}

#[test]
fn joined_readers_receive_the_completed_value_not_an_empty_placeholder() {
    let (rt, _, manifest, key) = runtime();
    let (started, ready) = mpsc::channel();
    let (release, resume) = mpsc::channel();
    let (one, two) = std::thread::scope(|scope| {
        let (rt, manifest, key) = (&rt, &manifest, &key);
        let first = scope.spawn(move || {
            rt.fetch(manifest, key, |_| {
                started.send(()).unwrap();
                resume.recv().unwrap();
                Ok("completed".into())
            })
        });
        ready.recv_timeout(Duration::from_secs(5)).unwrap();
        let second = scope.spawn(move || rt.fetch(manifest, key, |_| panic!("duplicate load")));
        wait_for(|| {
            rt.trace()
                .iter()
                .any(|t| matches!(t, Trace::RequestDeduplicated { .. }))
        });
        release.send(()).unwrap();
        (first.join().unwrap(), second.join().unwrap())
    });
    assert_eq!(one, Fetched::Fresh("completed".into()));
    assert_eq!(two, Fetched::Deduplicated("completed".into()));
}

#[test]
fn joined_readers_receive_failure_not_stale_success() {
    let (rt, _, manifest, key) = runtime();
    let (started, ready) = mpsc::channel();
    let (release, resume) = mpsc::channel();
    let (one, two) = std::thread::scope(|scope| {
        let (rt, manifest, key) = (&rt, &manifest, &key);
        let first = scope.spawn(move || {
            rt.fetch(manifest, key, |_| {
                started.send(()).unwrap();
                resume.recv().unwrap();
                Err("upstream failed".into())
            })
        });
        ready.recv_timeout(Duration::from_secs(5)).unwrap();
        let second = scope.spawn(move || rt.fetch(manifest, key, |_| panic!("duplicate load")));
        wait_for(|| {
            rt.trace()
                .iter()
                .any(|t| matches!(t, Trace::RequestDeduplicated { .. }))
        });
        release.send(()).unwrap();
        (first.join().unwrap(), second.join().unwrap())
    });
    assert_eq!(one, Fetched::Failed("upstream failed".into()));
    assert_eq!(two, one);
}

#[test]
fn concurrent_duplicate_commands_share_one_execution_and_result() {
    let (rt, _, _, _) = runtime();
    let count = Arc::new(AtomicUsize::new(0));
    let (started, ready) = mpsc::channel();
    let (release, resume) = mpsc::channel();
    let (one, two) = std::thread::scope(|scope| {
        let rt = &rt;
        let count = &count;
        let first = scope.spawn(move || {
            rt.command("interaction-1", || {
                count.fetch_add(1, Ordering::SeqCst);
                started.send(()).unwrap();
                resume.recv().unwrap();
                "first result".into()
            })
        });
        ready.recv_timeout(Duration::from_secs(5)).unwrap();
        let second = scope.spawn(move || {
            rt.command("interaction-1", || {
                count.fetch_add(1, Ordering::SeqCst);
                "second result".into()
            })
        });
        wait_for(|| {
            rt.trace().iter().any(|t| {
                matches!(
                    t,
                    Trace::CommandDeduplicated { .. } | Trace::CommandApplied { .. }
                )
            })
        });
        release.send(()).unwrap();
        (first.join().unwrap(), second.join().unwrap())
    });
    assert_eq!(count.load(Ordering::SeqCst), 1);
    assert_eq!(one, two);
}

#[test]
fn invalidated_work_cannot_repopulate_cache_or_erase_replacement() {
    let (rt, _, manifest, key) = runtime();
    let (started, ready) = mpsc::channel();
    let (release, resume) = mpsc::channel();
    let (old, replacement) = std::thread::scope(|scope| {
        let (rt, manifest, key) = (&rt, &manifest, &key);
        let first = scope.spawn(move || {
            rt.fetch(manifest, key, |_| {
                started.send(()).unwrap();
                resume.recv().unwrap();
                Ok("obsolete".into())
            })
        });
        ready.recv_timeout(Duration::from_secs(5)).unwrap();
        rt.invalidate("items");
        let replacement = rt.fetch(manifest, key, |_| Ok("current".into()));
        release.send(()).unwrap();
        (first.join().unwrap(), replacement)
    });
    assert!(!successful(&old), "obsolete request published: {old:?}");
    assert_eq!(replacement, Fetched::Fresh("current".into()));
    assert_eq!(
        rt.fetch(&manifest, &key, |_| panic!("lost replacement")),
        Fetched::FromCache("current".into())
    );
}

#[test]
fn last_subscriber_leaving_fences_late_success() {
    let (rt, _, manifest, key) = runtime();
    let subscription = rt.subscribe(&key);
    let (started, ready) = mpsc::channel();
    let (release, resume) = mpsc::channel();
    let result = std::thread::scope(|scope| {
        let (rt, manifest, key) = (&rt, &manifest, &key);
        let first = scope.spawn(move || {
            rt.fetch(manifest, key, |_| {
                started.send(()).unwrap();
                resume.recv().unwrap();
                Ok("too late".into())
            })
        });
        ready.recv_timeout(Duration::from_secs(5)).unwrap();
        drop(subscription);
        release.send(()).unwrap();
        first.join().unwrap()
    });
    assert!(!successful(&result), "disposed owner published: {result:?}");
    assert!(rt.public_cache_contents().is_empty());
    assert!(!rt.in_flight(&key));
}

#[test]
fn loader_unwind_releases_request_ownership() {
    let (rt, _, manifest, key) = runtime();
    let panic = std::panic::catch_unwind(|| rt.fetch(&manifest, &key, |_| panic!("load panic")));
    assert!(
        panic.is_err(),
        "the runtime must not swallow a loader panic"
    );
    assert!(!rt.in_flight(&key), "panic stranded the in-flight entry");
    assert_eq!(
        rt.fetch(&manifest, &key, |_| Ok("recovered".into())),
        Fetched::Fresh("recovered".into())
    );
}

#[test]
fn public_request_cannot_reuse_a_private_cache_entry() {
    let (rt, _, manifest, key) = runtime();
    rt.fetch(&manifest.clone().private(), &key, |_| {
        Ok("private value".into())
    });
    let result = rt.fetch(&manifest, &key, |_| Ok("public value".into()));
    assert!(
        !successful(&result),
        "privacy mismatch was accepted: {result:?}"
    );
    assert!(rt.public_cache_contents().is_empty());
}

#[test]
fn elapsed_deadline_prevents_late_success_and_cache_publication() {
    let (rt, clock, manifest, key) = runtime();
    let result = rt.fetch(&manifest, &key, |_| {
        clock.advance(manifest.timeout);
        Ok("too late".into())
    });
    assert!(!successful(&result), "timeout was ignored: {result:?}");
    assert!(rt.public_cache_contents().is_empty());
    assert!(!rt.in_flight(&key));
}

#[test]
fn monotonic_clock_and_backoff_do_not_wrap() {
    let clock = Clock::new();
    clock.advance(u64::MAX);
    clock.advance(1);
    assert_eq!(clock.now(), u64::MAX);
    let mut manifest = Manifest::new("items");
    manifest.retry_base = u64::MAX;
    assert_eq!(manifest.backoff(2, "key"), u64::MAX);
}

#[test]
fn distinct_command_keys_can_execute_while_another_command_is_blocked() {
    let (rt, _, _, _) = runtime();
    let (started, ready) = mpsc::channel();
    let (release, resume) = mpsc::channel();
    std::thread::scope(|scope| {
        let rt = &rt;
        let first = scope.spawn(move || {
            rt.command("blocked", || {
                started.send(()).unwrap();
                resume.recv().unwrap();
                "one".into()
            })
        });
        ready.recv_timeout(Duration::from_secs(5)).unwrap();
        let (done, result) = mpsc::channel();
        scope.spawn(move || {
            done.send(rt.command("independent", || "two".into()))
                .unwrap()
        });
        let independent = result.recv_timeout(Duration::from_secs(5));
        release.send(()).unwrap();
        assert_eq!(first.join().unwrap(), "one");
        assert_eq!(independent.unwrap(), "two");
    });
}

#[test]
fn command_panic_wakes_followers_and_prevents_uncertain_reexecution() {
    let (rt, _, _, _) = runtime();
    let count = AtomicUsize::new(0);
    let (started, ready) = mpsc::channel();
    let (release, resume) = mpsc::channel();
    std::thread::scope(|scope| {
        let (rt, count) = (&rt, &count);
        let first = scope.spawn(move || {
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                rt.try_command("uncertain", || {
                    count.fetch_add(1, Ordering::SeqCst);
                    started.send(()).unwrap();
                    resume.recv().unwrap();
                    panic!("after side effect")
                })
            }))
        });
        ready.recv_timeout(Duration::from_secs(5)).unwrap();
        let second = scope.spawn(move || rt.try_command("uncertain", || panic!("must not retry")));
        wait_for(|| {
            rt.trace()
                .iter()
                .any(|t| matches!(t, Trace::CommandDeduplicated { .. }))
        });
        release.send(()).unwrap();
        assert!(first.join().unwrap().is_err());
        assert_eq!(second.join().unwrap(), Err(CommandError::OutcomeUnknown));
    });
    assert_eq!(count.load(Ordering::SeqCst), 1);
    assert_eq!(
        rt.try_command("uncertain", || panic!("must retain unknown outcome")),
        Err(CommandError::OutcomeUnknown)
    );
    assert_eq!(rt.try_command("different", || "ok".into()), Ok("ok".into()));
}

#[test]
fn same_thread_recursive_joins_fail_without_deadlocking() {
    let (rt, _, manifest, key) = runtime();
    assert_eq!(
        rt.fetch(&manifest, &key, |_| {
            assert!(matches!(
                rt.fetch(&manifest, &key, |_| panic!("recursive load")),
                Fetched::Failed(_)
            ));
            Ok("outer".into())
        }),
        Fetched::Fresh("outer".into())
    );
    assert_eq!(
        rt.try_command("outer", || {
            assert_eq!(
                rt.try_command("outer", || panic!("recursive command")),
                Err(CommandError::Reentrant)
            );
            "outer".into()
        }),
        Ok("outer".into())
    );
}

#[test]
fn manifest_resource_mismatch_does_not_execute() {
    let (rt, _, manifest, _) = runtime();
    assert!(matches!(
        rt.fetch(&manifest, &Key::new("other", "1"), |_| panic!(
            "invalid manifest"
        )),
        Fetched::Failed(_)
    ));
    assert!(rt.trace().is_empty());
}

#[test]
fn active_flight_with_different_privacy_or_timeout_does_not_coalesce() {
    let (rt, _, manifest, key) = runtime();
    let (started, ready) = mpsc::channel();
    let (release, resume) = mpsc::channel();
    std::thread::scope(|scope| {
        let (rt, manifest, key) = (&rt, &manifest, &key);
        let first = scope.spawn(move || {
            rt.fetch(manifest, key, |_| {
                started.send(()).unwrap();
                resume.recv().unwrap();
                Ok("public".into())
            })
        });
        ready.recv_timeout(Duration::from_secs(5)).unwrap();
        let private = rt.fetch(&manifest.clone().private(), key, |_| {
            panic!("privacy mismatch")
        });
        let mut changed = manifest.clone();
        changed.timeout += 1;
        let timeout = rt.fetch(&changed, key, |_| panic!("policy mismatch"));
        release.send(()).unwrap();
        assert_eq!(first.join().unwrap(), Fetched::Fresh("public".into()));
        assert!(matches!(private, Fetched::Failed(_)));
        assert!(matches!(timeout, Fetched::Failed(_)));
    });
}

#[test]
fn joined_readers_timeout_before_uncooperative_owner_returns() {
    let (rt, clock, manifest, key) = runtime();
    let (started, ready) = mpsc::channel();
    let (release, resume) = mpsc::channel();
    let (done, result) = mpsc::channel();
    std::thread::scope(|scope| {
        let (rt, manifest, key) = (&rt, &manifest, &key);
        let first = scope.spawn(move || {
            rt.fetch(manifest, key, |_| {
                started.send(()).unwrap();
                resume.recv().unwrap();
                Ok("late".into())
            })
        });
        ready.recv_timeout(Duration::from_secs(5)).unwrap();
        scope.spawn(move || {
            done.send(rt.fetch(manifest, key, |_| panic!("duplicate")))
                .unwrap()
        });
        wait_for(|| {
            rt.trace()
                .iter()
                .any(|t| matches!(t, Trace::RequestDeduplicated { .. }))
        });
        clock.advance(manifest.timeout);
        let follower = result.recv_timeout(Duration::from_secs(5));
        release.send(()).unwrap();
        assert_eq!(first.join().unwrap(), Fetched::TimedOut);
        assert_eq!(follower.unwrap(), Fetched::TimedOut);
    });
    assert!(!rt.in_flight(&key));
    assert!(rt.public_cache_contents().is_empty());
    assert_eq!(
        rt.trace()
            .iter()
            .filter(|t| matches!(t, Trace::RequestTimedOut { .. }))
            .count(),
        1
    );
}

#[test]
fn cooperative_callback_observes_cancellation_before_returning() {
    let (rt, _, manifest, key) = runtime();
    let subscription = rt.subscribe(&key);
    let (started, ready) = mpsc::channel();
    std::thread::scope(|scope| {
        let (rt, manifest, key) = (&rt, &manifest, &key);
        let first = scope.spawn(move || {
            rt.fetch_cancellable(manifest, key, |_, cancel| {
                started.send(()).unwrap();
                wait_for(|| cancel.is_cancelled());
                assert_eq!(cancel.reason(), Some(StopReason::NoSubscribers));
                Err("adapter stopped".into())
            })
        });
        ready.recv_timeout(Duration::from_secs(5)).unwrap();
        drop(subscription);
        assert_eq!(
            first.join().unwrap(),
            Fetched::Cancelled(StopReason::NoSubscribers)
        );
    });
}

#[test]
fn invalidation_wakes_followers_before_owner_returns() {
    let (rt, _, manifest, key) = runtime();
    let (started, ready) = mpsc::channel();
    let (release, resume) = mpsc::channel();
    let (done, result) = mpsc::channel();
    std::thread::scope(|scope| {
        let (rt, manifest, key) = (&rt, &manifest, &key);
        let first = scope.spawn(move || {
            rt.fetch(manifest, key, |_| {
                started.send(()).unwrap();
                resume.recv().unwrap();
                Ok("late".into())
            })
        });
        ready.recv_timeout(Duration::from_secs(5)).unwrap();
        scope.spawn(move || {
            done.send(rt.fetch(manifest, key, |_| panic!("duplicate")))
                .unwrap()
        });
        wait_for(|| {
            rt.trace()
                .iter()
                .any(|t| matches!(t, Trace::RequestDeduplicated { .. }))
        });
        rt.invalidate("items");
        let follower = result.recv_timeout(Duration::from_secs(5));
        release.send(()).unwrap();
        assert_eq!(
            first.join().unwrap(),
            Fetched::Cancelled(StopReason::Invalidated)
        );
        assert_eq!(
            follower.unwrap(),
            Fetched::Cancelled(StopReason::Invalidated)
        );
    });
}

#[test]
fn unwinding_loader_wakes_followers_with_failure() {
    let (rt, _, manifest, key) = runtime();
    let (started, ready) = mpsc::channel();
    let (release, resume) = mpsc::channel();
    std::thread::scope(|scope| {
        let (rt, manifest, key) = (&rt, &manifest, &key);
        let first = scope.spawn(move || {
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                rt.fetch(manifest, key, |_| {
                    started.send(()).unwrap();
                    resume.recv().unwrap();
                    panic!("loader panic")
                })
            }))
        });
        ready.recv_timeout(Duration::from_secs(5)).unwrap();
        let second = scope.spawn(move || rt.fetch(manifest, key, |_| panic!("duplicate")));
        wait_for(|| {
            rt.trace()
                .iter()
                .any(|t| matches!(t, Trace::RequestDeduplicated { .. }))
        });
        release.send(()).unwrap();
        assert!(first.join().unwrap().is_err());
        assert_eq!(
            second.join().unwrap(),
            Fetched::Failed("loader panicked".into())
        );
    });
    assert!(!rt.in_flight(&key));
}

#[test]
fn failed_old_callback_does_not_remove_a_still_running_replacement() {
    let (rt, _, manifest, key) = runtime();
    let (started, ready) = mpsc::channel();
    let (release, resume) = mpsc::channel();
    let (new_started, new_ready) = mpsc::channel();
    let (new_release, new_resume) = mpsc::channel();
    std::thread::scope(|scope| {
        let (rt, manifest, key) = (&rt, &manifest, &key);
        let first = scope.spawn(move || {
            rt.fetch(manifest, key, |_| {
                started.send(()).unwrap();
                resume.recv().unwrap();
                Err("obsolete failure".into())
            })
        });
        ready.recv_timeout(Duration::from_secs(5)).unwrap();
        rt.invalidate("items");
        let second = scope.spawn(move || {
            rt.fetch(manifest, key, |_| {
                new_started.send(()).unwrap();
                new_resume.recv().unwrap();
                Ok("replacement".into())
            })
        });
        new_ready.recv_timeout(Duration::from_secs(5)).unwrap();
        release.send(()).unwrap();
        let old = first.join().unwrap();
        let still_running = rt.in_flight(key);
        new_release.send(()).unwrap();
        assert_eq!(second.join().unwrap(), Fetched::Fresh("replacement".into()));
        assert_eq!(old, Fetched::Cancelled(StopReason::Invalidated));
        assert!(still_running, "old callback deleted replacement ownership");
    });
}

#[test]
fn retry_budget_includes_backoff_and_never_starts_attempt_after_deadline() {
    let (rt, clock, mut manifest, key) = runtime();
    manifest.timeout = 5;
    manifest.max_attempts = 50;
    let mut calls = 0;
    assert_eq!(
        rt.fetch(&manifest, &key, |_| {
            calls += 1;
            Err("retry".into())
        }),
        Fetched::TimedOut
    );
    assert_eq!(calls, 1);
    assert_eq!(clock.now(), 5);
    assert!(!rt.in_flight(&key));
}

#[test]
fn zero_timeout_prevents_first_attempt() {
    let (rt, _, mut manifest, key) = runtime();
    manifest.timeout = 0;
    assert_eq!(
        rt.fetch(&manifest, &key, |_| panic!("expired before dispatch")),
        Fetched::TimedOut
    );
    assert!(
        !rt.trace()
            .iter()
            .any(|t| matches!(t, Trace::RequestStarted { .. }))
    );
}

#[test]
fn deadline_near_clock_saturation_fails_closed() {
    let (rt, clock, manifest, key) = runtime();
    clock.advance(u64::MAX - 1);
    assert_eq!(
        rt.fetch(&manifest, &key, |_| {
            clock.advance(2);
            Ok("late".into())
        }),
        Fetched::TimedOut
    );
}

#[test]
fn a_real_empty_value_is_a_valid_shared_result() {
    let (rt, _, manifest, key) = runtime();
    assert_eq!(
        rt.fetch(&manifest, &key, |_| Ok(String::new())),
        Fetched::Fresh(String::new())
    );
    assert_eq!(
        rt.fetch(&manifest, &key, |_| panic!("empty is not missing")),
        Fetched::FromCache(String::new())
    );
}

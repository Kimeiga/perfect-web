//! E2A-R — the runtime half of structured concurrency (ADR-0016).
//!
//! `docs/RISK_QUEUE.md` splits E2A because *a runtime task tree cannot make
//! compile-fail fixtures fail at compilation*. The static half is
//! `pw-core::scope`, which rejects `.pw` programs. This half proves the same
//! semantics are implementable and behave as specified when tasks actually run.
//!
//! **Everything here is behaviour, never a static guarantee.** A passing test
//! in this crate does not make any misuse a compile error.
//!
//! # The five properties
//!
//! | property | provided by |
//! |---|---|
//! | a task cannot outlive its scope | [`std::thread::scope`], reused |
//! | cancellation propagates to descendants | [`Cancel`] |
//! | cleanup runs children-first, LIFO within a scope | [`Scope::on_cleanup`] |
//! | a result arriving after its scope died is discarded | [`Scope::commit`] |
//! | no task is left running when a scope exits | [`live_tasks`] |
//!
//! Reusing `std::thread::scope` for the first row is deliberate (ADR-0016): the
//! guarantee is already sound, and re-deriving it would add risk without adding
//! evidence.

use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

/// Tasks running anywhere in the process. Useful for a whole-runtime check,
/// but **not** for asserting leak-freedom of one scope: concurrent scopes share
/// it. Per-scope counting is [`Scope::task_counter`], and that is what the
/// leak tests use.
static LIVE: AtomicUsize = AtomicUsize::new(0);

/// How many tasks are running in the whole process right now.
pub fn live_tasks() -> usize {
    LIVE.load(Ordering::SeqCst)
}

/// A live-task count for one scope. Cloneable, so it can be read *after* the
/// scope has ended — which is exactly when leak-freedom must be checked.
#[derive(Clone, Default)]
pub struct TaskCounter(Arc<AtomicUsize>);

impl TaskCounter {
    pub fn get(&self) -> usize {
        self.0.load(Ordering::SeqCst)
    }
}

/// A cancellation signal shared by a scope and everything beneath it.
///
/// Cancellation is one-way and monotonic: once set it never clears. A task that
/// checks it and then keeps running is not prevented from doing so — the
/// runtime cannot preempt arbitrary code, and pretending otherwise would be the
/// kind of overclaim `docs/RISK_QUEUE.md` exists to catch. What the runtime
/// guarantees is that the signal *arrives* and that any result produced
/// afterwards is refused (see [`Scope::commit`]).
#[derive(Clone, Default)]
pub struct Cancel {
    flag: Arc<AtomicBool>,
    /// Descendants, so cancelling a parent reaches a child created later.
    children: Arc<Mutex<Vec<Cancel>>>,
}

impl Cancel {
    pub fn new() -> Self {
        Self::default()
    }

    /// A token that is cancelled when this one is.
    pub fn child(&self) -> Cancel {
        let c = Cancel::new();
        // A child created *after* the parent was already cancelled must start
        // cancelled. Without this, ordering decides correctness — the classic
        // race in every hand-rolled cancellation tree.
        if self.is_cancelled() {
            c.cancel();
        }
        self.children.lock().expect("cancel lock").push(c.clone());
        c
    }

    pub fn cancel(&self) {
        self.flag.store(true, Ordering::SeqCst);
        for c in self.children.lock().expect("cancel lock").iter() {
            c.cancel();
        }
    }

    pub fn is_cancelled(&self) -> bool {
        self.flag.load(Ordering::SeqCst)
    }
}

/// Why a scope ended. Cleanup handlers receive it, because "we finished" and
/// "we were cancelled" are different situations and a handler that cannot tell
/// them apart cannot do the right thing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Exit {
    Completed,
    Cancelled,
}

/// The result of trying to commit a value produced by a task.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Commit {
    Applied,
    /// The owning scope was cancelled before the value arrived. Charter §7.5's
    /// late-result case: there is nothing left to apply it to.
    RefusedScopeEnded,
}

type Cleanup = Box<dyn FnOnce(Exit) + Send>;

/// A lexical scope that owns the tasks spawned inside it.
pub struct Scope<'a, 'env: 'a> {
    inner: &'a std::thread::Scope<'a, 'env>,
    cancel: Cancel,
    name: String,
    cleanups: Arc<Mutex<Vec<Cleanup>>>,
    live: TaskCounter,
}

impl<'a, 'env> Scope<'a, 'env> {
    pub fn name(&self) -> &str {
        &self.name
    }

    /// This scope's cancellation token. Hand it to work that should stop when
    /// the scope does.
    pub fn cancel_token(&self) -> Cancel {
        self.cancel.clone()
    }

    /// Cancel this scope and everything beneath it.
    pub fn cancel(&self) {
        self.cancel.cancel();
    }

    pub fn is_cancelled(&self) -> bool {
        self.cancel.is_cancelled()
    }

    /// This scope's live-task count. Clone it before the scope ends to assert
    /// afterwards that it returned to zero.
    pub fn task_counter(&self) -> TaskCounter {
        self.live.clone()
    }

    /// Run `f` on a new thread owned by this scope.
    ///
    /// The task is joined before the enclosing [`scope`] call returns — that is
    /// `std::thread::scope`'s guarantee and the reason this crate reuses it.
    pub fn spawn<F>(&self, f: F)
    where
        F: FnOnce(Cancel) + Send + 'env,
    {
        let token = self.cancel.clone();
        let live = self.live.clone();
        LIVE.fetch_add(1, Ordering::SeqCst);
        live.0.fetch_add(1, Ordering::SeqCst);
        self.inner.spawn(move || {
            // Decremented on unwind too: a panicking task that left the counter
            // raised would make every later leak assertion fail for the wrong
            // reason, which is worse than not counting at all.
            struct Guard(TaskCounter);
            impl Drop for Guard {
                fn drop(&mut self) {
                    LIVE.fetch_sub(1, Ordering::SeqCst);
                    self.0.0.fetch_sub(1, Ordering::SeqCst);
                }
            }
            let _g = Guard(live);
            f(token);
        });
    }

    /// Register work to run when this scope ends, in reverse order of
    /// registration. Cleanup always runs, cancelled or not; the [`Exit`] says
    /// which happened.
    pub fn on_cleanup<F>(&self, f: F)
    where
        F: FnOnce(Exit) + Send + 'static,
    {
        self.cleanups
            .lock()
            .expect("cleanup lock")
            .push(Box::new(f));
    }

    /// Apply a value produced by a task, unless the scope has ended.
    ///
    /// This is the runtime counterpart of `PW2003`: the static checker rejects
    /// a *handle* used outside its owner, and this refuses a *value* arriving
    /// after the owner is gone. Neither subsumes the other — a task can finish
    /// late without any handle ever escaping.
    pub fn commit<T>(&self, value: T, apply: impl FnOnce(T)) -> Commit {
        if self.cancel.is_cancelled() {
            return Commit::RefusedScopeEnded;
        }
        apply(value);
        Commit::Applied
    }

    /// Open a scope inside this one. It is cancelled when this one is, and its
    /// cleanups run before this one's.
    pub fn child<'e, F, R>(&self, name: &str, f: F) -> R
    where
        F: for<'s> FnOnce(&Scope<'s, 'e>) -> R,
    {
        run(name, self.cancel.child(), f)
    }
}

/// Open a root scope.
///
/// Every task spawned inside has finished by the time this returns, and every
/// registered cleanup has run.
pub fn scope<'env, F, R>(name: &str, f: F) -> R
where
    // `'env` is the caller's frame, so a task may borrow the caller's locals —
    // most of the point of a scoped runtime. The *borrow* of the scope is left
    // free rather than tied to `'s`: `std::thread::Scope` is invariant in its
    // first lifetime, and tying them makes the reference impossible to form.
    F: for<'s> FnOnce(&Scope<'s, 'env>) -> R,
{
    run(name, Cancel::new(), f)
}

fn run<'env, F, R>(name: &str, cancel: Cancel, f: F) -> R
where
    F: for<'s> FnOnce(&Scope<'s, 'env>) -> R,
{
    let cleanups: Arc<Mutex<Vec<Cleanup>>> = Arc::new(Mutex::new(Vec::new()));
    let result = std::thread::scope(|inner| {
        let s = Scope {
            inner,
            cancel: cancel.clone(),
            name: name.to_string(),
            cleanups: Arc::clone(&cleanups),
            live: TaskCounter::default(),
        };
        f(&s)
        // `inner` joins every spawned task here, before cleanup runs below.
        // The order matters: a cleanup that closed a connection while a task
        // was still using it would be a use-after-free in a safer language's
        // clothing.
    });

    let exit = if cancel.is_cancelled() {
        Exit::Cancelled
    } else {
        Exit::Completed
    };
    let mut pending = cleanups.lock().expect("cleanup lock");
    while let Some(c) = pending.pop() {
        c(exit);
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::AtomicUsize;
    use std::time::Duration;

    /// A place for a test's tasks to record what happened, in order.
    #[derive(Default)]
    struct Log(Mutex<Vec<String>>);

    impl Log {
        fn push(&self, s: impl Into<String>) {
            self.0.lock().expect("log").push(s.into());
        }
        fn take(&self) -> Vec<String> {
            self.0.lock().expect("log").clone()
        }
    }

    #[test]
    fn a_task_cannot_outlive_its_scope() {
        let done = AtomicBool::new(false);
        let mut counter = TaskCounter::default();

        scope("request", |s| {
            counter = s.task_counter();
            s.spawn(|_| {
                std::thread::sleep(Duration::from_millis(30));
                done.store(true, Ordering::SeqCst);
            });
            // Deliberately no join here: the scope is what joins.
        });

        assert!(done.load(Ordering::SeqCst), "the scope must have waited");
        // Per-scope, not the process-wide counter: cargo runs tests in
        // parallel, so a global count is another test's business too. Asserting
        // on it made three tests fail for a reason that had nothing to do with
        // the runtime.
        assert_eq!(counter.get(), 0, "no task may still be running");
    }

    #[test]
    fn cancellation_reaches_a_task_that_is_already_running() {
        let observed = Arc::new(AtomicBool::new(false));
        let o = Arc::clone(&observed);

        scope("request", |s| {
            s.spawn(move |cancel| {
                while !cancel.is_cancelled() {
                    std::thread::sleep(Duration::from_millis(1));
                }
                o.store(true, Ordering::SeqCst);
            });
            std::thread::sleep(Duration::from_millis(10));
            s.cancel();
        });

        assert!(
            observed.load(Ordering::SeqCst),
            "the task must have seen it"
        );
    }

    #[test]
    fn cancellation_propagates_to_a_child_scope() {
        let inner_saw = Arc::new(AtomicBool::new(false));
        let i = Arc::clone(&inner_saw);

        scope("route", |outer| {
            let token = outer.cancel_token();
            outer.spawn(move |_| {
                std::thread::sleep(Duration::from_millis(10));
                token.cancel();
            });

            outer.child("component", |inner| {
                inner.spawn(move |cancel| {
                    while !cancel.is_cancelled() {
                        std::thread::sleep(Duration::from_millis(1));
                    }
                    i.store(true, Ordering::SeqCst);
                });
            });
        });

        assert!(
            inner_saw.load(Ordering::SeqCst),
            "cancelling the parent must reach a task in the child scope"
        );
    }

    #[test]
    fn a_child_created_after_cancellation_starts_cancelled() {
        // The classic race in a hand-rolled cancellation tree: the child
        // registers with a parent that has already fired, and waits forever.
        let parent = Cancel::new();
        parent.cancel();
        let late = parent.child();
        assert!(
            late.is_cancelled(),
            "a child of a cancelled parent is cancelled"
        );

        // Control: a child of a live parent is not.
        let live = Cancel::new();
        assert!(!live.child().is_cancelled());
    }

    #[test]
    fn cleanup_runs_children_before_parents_and_lifo_within_a_scope() {
        let log = Arc::new(Log::default());
        {
            let l1 = Arc::clone(&log);
            let l2 = Arc::clone(&log);
            let l3 = Arc::clone(&log);
            let l4 = Arc::clone(&log);
            scope("route", |outer| {
                outer.on_cleanup(move |_| l1.push("route-first-registered"));
                outer.on_cleanup(move |_| l2.push("route-second-registered"));
                outer.child("component", |inner| {
                    inner.on_cleanup(move |_| l3.push("component-first-registered"));
                    inner.on_cleanup(move |_| l4.push("component-second-registered"));
                });
            });
        }

        assert_eq!(
            log.take(),
            [
                // The child scope ends first, so its cleanups run first...
                "component-second-registered",
                "component-first-registered",
                // ...and within a scope, reverse registration order.
                "route-second-registered",
                "route-first-registered",
            ]
        );
    }

    #[test]
    fn cleanup_runs_after_every_task_has_stopped() {
        // A cleanup that closed a resource while a task was still using it
        // would be a use-after-free wearing a safe language's clothes.
        let log = Arc::new(Log::default());
        let l = Arc::clone(&log);

        scope("request", |s| {
            let inner = Arc::clone(&log);
            s.spawn(move |_| {
                std::thread::sleep(Duration::from_millis(20));
                inner.push("task-finished");
            });
            s.on_cleanup(move |_| l.push("cleanup"));
        });

        assert_eq!(log.take(), ["task-finished", "cleanup"]);
    }

    #[test]
    fn cleanup_says_whether_the_scope_was_cancelled() {
        let seen = Arc::new(Mutex::new(Vec::new()));

        let s1 = Arc::clone(&seen);
        scope("completed", |s| {
            s.on_cleanup(move |exit| s1.lock().expect("l").push(exit));
        });

        let s2 = Arc::clone(&seen);
        scope("cancelled", |s| {
            s.on_cleanup(move |exit| s2.lock().expect("l").push(exit));
            s.cancel();
        });

        assert_eq!(
            *seen.lock().expect("l"),
            [Exit::Completed, Exit::Cancelled],
            "a handler that cannot tell them apart cannot do the right thing"
        );
    }

    #[test]
    fn a_result_arriving_after_the_scope_died_is_refused() {
        // The runtime counterpart of PW2003. The static rule rejects a handle
        // used outside its owner; this refuses a *value* produced after the
        // owner is gone, which can happen with no handle escaping at all.
        let applied = AtomicUsize::new(0);

        let outcome = scope("component", |s| {
            let before = s.commit(1, |_| {
                applied.fetch_add(1, Ordering::SeqCst);
            });
            s.cancel();
            let after = s.commit(2, |_| {
                applied.fetch_add(1, Ordering::SeqCst);
            });
            (before, after)
        });

        assert_eq!(outcome.0, Commit::Applied, "a live scope accepts the value");
        assert_eq!(
            outcome.1,
            Commit::RefusedScopeEnded,
            "a dead scope must refuse it"
        );
        assert_eq!(
            applied.load(Ordering::SeqCst),
            1,
            "the refused value must not have been applied"
        );
    }

    #[test]
    fn the_leak_counter_can_detect_a_running_task() {
        // Negative control for `a_task_cannot_outlive_its_scope`. That test
        // asserts `live_tasks() == 0` after a scope; if the counter never rose,
        // it would pass without measuring anything.
        let mut counter = TaskCounter::default();
        let peak = Arc::new(AtomicUsize::new(0));

        scope("request", |s| {
            counter = s.task_counter();
            let c = s.task_counter();
            let p = Arc::clone(&peak);
            s.spawn(move |_| {
                std::thread::sleep(Duration::from_millis(20));
                p.store(c.get(), Ordering::SeqCst);
            });
            std::thread::sleep(Duration::from_millis(5));
        });

        assert!(
            peak.load(Ordering::SeqCst) >= 1,
            "the counter must rise while a task runs, or the leak check is vacuous"
        );
        assert_eq!(counter.get(), 0, "and fall back to zero afterwards");
    }

    #[test]
    fn a_panicking_task_does_not_corrupt_the_leak_counter() {
        // Otherwise one panicking test poisons every later leak assertion, and
        // the failures point anywhere but at the cause.
        let counter: Arc<Mutex<TaskCounter>> = Arc::default();
        let c = Arc::clone(&counter);
        let result = std::panic::catch_unwind(move || {
            scope("request", |s| {
                *c.lock().expect("l") = s.task_counter();
                s.spawn(|_| panic!("deliberate"));
            });
        });
        assert!(result.is_err(), "the panic must surface, not be swallowed");
        assert_eq!(
            counter.lock().expect("l").get(),
            0,
            "the counter must be balanced even when a task unwinds"
        );
    }

    #[test]
    fn many_tasks_across_nested_scopes_all_stop() {
        let done = Arc::new(AtomicUsize::new(0));

        scope("session", |s0| {
            for _ in 0..4 {
                let d = Arc::clone(&done);
                s0.spawn(move |_| {
                    std::thread::sleep(Duration::from_millis(5));
                    d.fetch_add(1, Ordering::SeqCst);
                });
            }
            s0.child("route", |s1| {
                for _ in 0..4 {
                    let d = Arc::clone(&done);
                    s1.spawn(move |_| {
                        std::thread::sleep(Duration::from_millis(5));
                        d.fetch_add(1, Ordering::SeqCst);
                    });
                }
            });
        });

        assert_eq!(done.load(Ordering::SeqCst), 8);
    }

    #[test]
    fn the_process_wide_counter_observes_running_work() {
        // `live_tasks()` is documented as whole-runtime, not per-scope. Assert
        // only what is true under parallel tests: it rises while work runs.
        let seen = Arc::new(AtomicUsize::new(0));
        let s2 = Arc::clone(&seen);
        scope("request", |s| {
            s.spawn(move |_| {
                s2.store(live_tasks(), Ordering::SeqCst);
                std::thread::sleep(Duration::from_millis(10));
            });
            std::thread::sleep(Duration::from_millis(3));
        });
        assert!(seen.load(Ordering::SeqCst) >= 1);
    }
}

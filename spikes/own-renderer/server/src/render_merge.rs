//! **One render's reads** (NEXT 4c's first step, track `render-merge`; W9's
//! design note "a read is bounded by construction", its second case, and the
//! integrator's ruling of 2026-10-14): within one render the host makes each
//! identical operation call once, the operation and its arguments as the
//! key, and every binding that asks gets the same answer; and the render's
//! calls read one snapshot of the data layer.
//!
//! - **A render** is the host reading what a session's documents show at one
//!   moment: a page's bindings (`bindings_where`), and a session's documents
//!   read again after a commit, with the drain's read of the cart before them
//!   (`drain_held`, `send_documents_where`). A render begun inside another
//!   for the same session is that one; one for another session is its own.
//! - **Nothing is kept past it.** Its answers go when it ends, so an answer
//!   is one the render itself read, and no commit has to say which answers
//!   it makes stale.
//! - **Its snapshot is the layer's** ([`DataLayer::snapshot`]), taken at the
//!   render's first read and given up when it ends: the store as one moment
//!   left it in memory, one read transaction on PostgreSQL. Two bindings of
//!   one render cannot straddle a commit. What pw-resource answers from its
//!   cache, or from a flight another thread began, is as old as the query's
//!   policy lets it be, as before.
//! - **Only the layer's operations are merged**, never the platform's (the
//!   session, the principal, the clock), whose calls note what a read holds
//!   until (ADR-0304); and only answers, never failures: a call that failed
//!   is made again by the next binding that asks.

use super::*;
use crate::data::{DataLayer, Ops, Snapshot};
use std::cell::RefCell;
use std::sync::OnceLock;

/// **A render being read**: the layer it reads, the session whose documents
/// it reads, the layer's snapshot, and what its calls answered.
pub(crate) struct Render {
    /// The layer, by its address: a render answers for its server's layer
    /// alone.
    layer: usize,
    session: String,
    /// Taken at the render's first read, and dropped with the render.
    snapshot: OnceLock<Option<Snapshot>>,
    /// Each call made, by the session it was made for, its operation and its
    /// arguments, and what it answered.
    answers: Mutex<Vec<(Call, Vec<Val>)>>,
}

/// A call: the session it is made for, the operation, its arguments.
type Call = (String, String, Vec<Val>);

thread_local! {
    /// **The renders this thread is reading**, the innermost last.
    static READING: RefCell<Vec<Arc<Render>>> = const { RefCell::new(Vec::new()) };
}

/// A layer's address, which names it among the servers of one process.
fn address(data: &dyn DataLayer) -> usize {
    data as *const dyn DataLayer as *const () as usize
}

/// **A render begun on this thread**, which ends when this is dropped.
#[must_use = "a render ends when what began it is dropped"]
pub(crate) struct Rendering(Option<Arc<Render>>);

/// **Begin a render of `data`'s for `session`** on this thread: its own,
/// unless the innermost render this thread reads is of the same layer for
/// the same session, whose reads these are then. A command run inside a
/// render (a test's hook does) reads what it committed in a render of its own.
pub(crate) fn begin(data: &dyn DataLayer, session: &str) -> Rendering {
    let layer = address(data);
    READING.with(|reading| {
        let mut reading = reading.borrow_mut();
        if reading
            .last()
            .is_some_and(|r| r.layer == layer && r.session == session)
        {
            return Rendering(None);
        }
        let render = Arc::new(Render {
            layer,
            session: session.to_string(),
            snapshot: OnceLock::new(),
            answers: Mutex::default(),
        });
        reading.push(render.clone());
        Rendering(Some(render))
    })
}

impl Drop for Rendering {
    fn drop(&mut self) {
        let Some(mine) = self.0.take() else {
            return;
        };
        // In whatever order renders end, a panic's unwinding among them.
        let _ = READING.try_with(|reading| {
            if let Ok(mut reading) = reading.try_borrow_mut() {
                reading.retain(|r| !Arc::ptr_eq(r, &mine));
            }
        });
    }
}

/// **The render this thread reads `data` in**, the innermost, where it
/// reads one: a public read inside it (a materialization's, for no session)
/// is the render's too.
pub(crate) fn reading(data: &dyn DataLayer) -> Option<Arc<Render>> {
    let layer = address(data);
    READING.with(|reading| {
        reading
            .borrow()
            .iter()
            .rev()
            .find(|r| r.layer == layer)
            .cloned()
    })
}

impl Render {
    /// **The layer's operations for one query of the render**, for
    /// `session`, each reading the render's snapshot, which the first of
    /// them takes.
    pub(crate) fn reads(
        &self,
        data: &dyn DataLayer,
        session: &str,
        stopped: Option<Stopped>,
    ) -> Ops {
        match self.snapshot.get_or_init(|| data.snapshot()) {
            Some(snapshot) => data.reads_in(snapshot, session, stopped),
            None => data.reads(session, stopped),
        }
    }

    /// **`op`, its identical calls in the render made once**: a call already
    /// answered in the render, for `session` with the same arguments, is
    /// answered as it was, and `op` is not called; any other is `op`'s, and
    /// what it answers is kept until the render ends. A failure is not kept.
    pub(crate) fn once(self: &Arc<Self>, session: &str, name: &str, op: HostFn) -> HostFn {
        let (render, session, name) = (self.clone(), session.to_string(), name.to_string());
        Arc::new(move |args: &[Val]| {
            if let Some(answer) = render.answered(&session, &name, args) {
                return Ok(answer);
            }
            let answer = op(args)?;
            render.answers.lock().expect("answers").push((
                (session.clone(), name.clone(), args.to_vec()),
                answer.clone(),
            ));
            Ok(answer)
        })
    }

    /// What the render's call of `name` with `args`, for `session`,
    /// answered, where it made one.
    fn answered(&self, session: &str, name: &str, args: &[Val]) -> Option<Vec<Val>> {
        self.answers
            .lock()
            .expect("answers")
            .iter()
            .find(|((s, n, a), _)| s == session && n == name && a.as_slice() == args)
            .map(|(_, answer)| answer.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicI64, AtomicU64, Ordering};

    const READ: &str = "test:data/value#read";

    /// **A layer of one number**, which a test changes: `READ` answers the
    /// number as the read sees it, the session it was read for and its
    /// arguments, and fails while `failing` says so. Its calls are counted,
    /// and so are its snapshots alive.
    #[derive(Default)]
    struct Numbered {
        value: Arc<AtomicI64>,
        calls: Arc<AtomicU64>,
        failing: Arc<AtomicU64>,
        alive: Arc<AtomicU64>,
    }

    /// A snapshot of [`Numbered`]: the number when it was taken.
    struct Taken(i64, Arc<AtomicU64>);

    impl Drop for Taken {
        fn drop(&mut self) {
            self.1.fetch_sub(1, Ordering::SeqCst);
        }
    }

    impl Numbered {
        /// `READ`, answering what `now` says the number is.
        fn ops(&self, session: &str, now: impl Fn() -> i64 + Send + Sync + 'static) -> Ops {
            let (calls, failing, session) = (
                self.calls.clone(),
                self.failing.clone(),
                session.to_string(),
            );
            let mut ops = Ops::new();
            ops.insert(
                READ.to_string(),
                Arc::new(move |args: &[Val]| {
                    calls.fetch_add(1, Ordering::SeqCst);
                    if failing.load(Ordering::SeqCst) > 0 {
                        failing.fetch_sub(1, Ordering::SeqCst);
                        return Err("the number is unavailable".to_string());
                    }
                    Ok(vec![Val::String(format!("{session}:{}:{args:?}", now()))])
                }),
            );
            ops
        }
    }

    /// A command's staging, which stages nothing.
    struct Nothing;

    impl crate::data::Staged for Nothing {
        fn ops(&self) -> Ops {
            Ops::new()
        }
        fn rows(&self) -> Option<Vec<(String, String)>> {
            None
        }
        fn publish(&mut self) {}
    }

    impl DataLayer for Numbered {
        fn reads(&self, session: &str, _stopped: Option<Stopped>) -> Ops {
            let value = self.value.clone();
            self.ops(session, move || value.load(Ordering::SeqCst))
        }

        fn snapshot(&self) -> Option<Snapshot> {
            self.alive.fetch_add(1, Ordering::SeqCst);
            Some(Box::new(Taken(
                self.value.load(Ordering::SeqCst),
                self.alive.clone(),
            )))
        }

        fn reads_in(&self, snapshot: &Snapshot, session: &str, _stopped: Option<Stopped>) -> Ops {
            let taken = snapshot.downcast_ref::<Taken>().expect("this layer's").0;
            self.ops(session, move || taken)
        }

        fn begin<'a>(&'a self, _session: &str) -> Box<dyn crate::data::Staged + 'a> {
            Box::new(Nothing)
        }

        fn grants(&self) -> Vec<&'static str> {
            Vec::new()
        }

        fn default_page(&self) -> Option<&'static str> {
            None
        }
    }

    /// **One query's read** of `READ` with `args`, for `session`, as the
    /// host's `answer_within` makes it: in the render this thread reads,
    /// each identical call once, where it reads one.
    fn read(data: &Numbered, session: &str, args: &[Val]) -> Result<Val, String> {
        let op = match reading(data) {
            Some(render) => {
                let op = render
                    .reads(data, session, None)
                    .remove(READ)
                    .expect("READ");
                render.once(session, READ, op)
            }
            None => data.reads(session, None).remove(READ).expect("READ"),
        };
        op(args).map(|mut out| out.remove(0))
    }

    fn n(v: i64) -> Val {
        Val::S64(v)
    }

    /// **Each identical call of a render is made once**, and every query
    /// that asks is given the same answer; outside a render, each is made.
    #[test]
    fn a_renders_identical_calls_are_made_once() {
        let data = Numbered::default();
        {
            let _render = begin(&data, "a");
            let first = read(&data, "a", &[n(1)]);
            let second = read(&data, "a", &[n(1)]);
            assert_eq!(first, second);
            assert_eq!(data.calls.load(Ordering::SeqCst), 1, "one call");
        }
        read(&data, "a", &[n(1)]).expect("read");
        read(&data, "a", &[n(1)]).expect("read");
        assert_eq!(
            data.calls.load(Ordering::SeqCst),
            3,
            "each, outside a render"
        );
    }

    /// **The control: calls with other arguments are each made**, and each
    /// answered for its own.
    #[test]
    fn a_renders_calls_with_other_arguments_are_each_made() {
        let data = Numbered::default();
        let _render = begin(&data, "a");
        let one = read(&data, "a", &[n(1)]).expect("read");
        let two = read(&data, "a", &[n(2)]).expect("read");
        assert_ne!(one, two);
        assert_eq!(data.calls.load(Ordering::SeqCst), 2, "two calls");
    }

    /// **A call for another session is its own**: a read for no session
    /// inside a reader's render (a materialization's) is not answered as
    /// the reader's was.
    #[test]
    fn a_renders_call_for_another_session_is_its_own() {
        let data = Numbered::default();
        let _render = begin(&data, "a");
        let readers = read(&data, "a", &[n(1)]).expect("read");
        let nobodys = read(&data, "", &[n(1)]).expect("read");
        assert_ne!(readers, nobodys);
        assert_eq!(data.calls.load(Ordering::SeqCst), 2, "two calls");
    }

    /// **A failure is not kept**: the next query that asks calls again, and
    /// is answered.
    #[test]
    fn a_renders_failed_call_is_made_again() {
        let data = Numbered::default();
        data.failing.store(1, Ordering::SeqCst);
        let _render = begin(&data, "a");
        assert!(read(&data, "a", &[n(1)]).is_err(), "failed");
        assert!(read(&data, "a", &[n(1)]).is_ok(), "asked again, answered");
        assert_eq!(data.calls.load(Ordering::SeqCst), 2, "two calls");
    }

    /// **A render reads one snapshot**: a change after its first read is
    /// not seen by its later ones, and is by the next render's. Nothing of
    /// the render is kept past it, its snapshot neither.
    #[test]
    fn a_render_reads_one_snapshot_and_keeps_nothing_past_it() {
        let data = Numbered::default();
        {
            let _render = begin(&data, "a");
            let before = read(&data, "a", &[n(1)]).expect("read");
            data.value.store(7, Ordering::SeqCst);
            // Another call, so the layer is read: in the render's snapshot.
            let after = read(&data, "a", &[n(2)]).expect("read");
            assert_eq!(before, Val::String("a:0:[S64(1)]".into()));
            assert_eq!(after, Val::String("a:0:[S64(2)]".into()), "the snapshot's");
            assert_eq!(data.alive.load(Ordering::SeqCst), 1, "one snapshot");
        }
        assert_eq!(data.alive.load(Ordering::SeqCst), 0, "given up");
        let _render = begin(&data, "a");
        assert_eq!(
            read(&data, "a", &[n(1)]).expect("read"),
            Val::String("a:7:[S64(1)]".into()),
            "the next render reads the change, its own call"
        );
        assert_eq!(data.calls.load(Ordering::SeqCst), 3);
    }

    /// **A render begun inside another for the same session is that one**;
    /// one for another session is its own, and reads what was committed
    /// before it began.
    #[test]
    fn a_render_inside_another_is_its_for_its_session_alone() {
        let data = Numbered::default();
        let _outer = begin(&data, "a");
        read(&data, "a", &[n(1)]).expect("read");
        data.value.store(7, Ordering::SeqCst);
        {
            let _same = begin(&data, "a");
            assert_eq!(
                read(&data, "a", &[n(1)]).expect("read"),
                Val::String("a:0:[S64(1)]".into()),
                "the outer render's answer"
            );
        }
        {
            let _other = begin(&data, "b");
            assert_eq!(
                read(&data, "b", &[n(1)]).expect("read"),
                Val::String("b:7:[S64(1)]".into()),
                "its own snapshot"
            );
        }
        assert_eq!(data.calls.load(Ordering::SeqCst), 2);
        assert_eq!(
            data.alive.load(Ordering::SeqCst),
            1,
            "the other's, given up"
        );
    }

    /// **A render of one layer answers for no other's**: two servers in one
    /// process each read their own.
    #[test]
    fn a_render_of_one_layer_answers_for_no_other() {
        let (one, two) = (Numbered::default(), Numbered::default());
        let _render = begin(&one, "a");
        assert!(reading(&one).is_some());
        assert!(reading(&two).is_none(), "no render of the second layer");
    }
}

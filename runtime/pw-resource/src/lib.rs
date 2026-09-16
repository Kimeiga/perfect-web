//! E4 — the local resource runtime.
//!
//! Charter §14 M4 task 3 lists what a resource runtime owes the language:
//! one in-flight request per key, request deduplication, reference-counted
//! subscribers, cancellation when unused, bounded retries with jitter, a
//! deterministic test clock, trace events, and private/public cache separation.
//!
//! These are **behaviour**, like E2A-R (ADR-0016). A passing test here does not
//! make misuse a compile error; the compiler-side half of E4 is the resource
//! manifest in `pw-core::hir`.
//!
//! # Why a test clock
//!
//! Freshness, timeouts and retry backoff are all time. A test that sleeps is
//! slow and flaky, and a flaky concurrency test gets muted rather than fixed.
//! [`Clock`] is advanced by the test, so every timing property here is exact.

pub mod entry;
pub use entry::{
    DeploymentIdentityKey, DevelopmentIdentityKey, EntryIdentity, ExplicitIdentityKey,
    IdentityKeyProvider, Partition, ResourceEntryId, Version,
};

use std::collections::HashMap;
use std::panic::{AssertUnwindSafe, catch_unwind, resume_unwind};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Condvar, Mutex};
use std::thread::{self, ThreadId};
use std::time::Duration;

/// Milliseconds since the runtime started.
pub type Millis = u64;

/// A clock the tests own.
///
/// Real time appears only when a host advances this from a real source, which
/// keeps every timing rule in one place instead of scattered across `Instant`
/// calls that cannot be controlled.
#[derive(Clone, Default)]
pub struct Clock(Arc<AtomicU64>);

impl Clock {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn now(&self) -> Millis {
        self.0.load(Ordering::SeqCst)
    }
    pub fn advance(&self, ms: Millis) {
        let _ = self
            .0
            .fetch_update(Ordering::SeqCst, Ordering::SeqCst, |now| {
                Some(now.saturating_add(ms))
            });
    }
}

/// Which cache a value may be stored in.
///
/// The separation is the point: charter §7.8 and the M4 gate both say private
/// state must never appear in public cache output. Both lookup and public
/// export enforce this classification; keys must still be scoped by the host.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Privacy {
    /// Shareable between users.
    Public,
    /// Belongs to one session and must never be served to another.
    Private,
}

/// What a resource is asked for: its name plus the key that identifies the row.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Key {
    pub resource: String,
    pub key: String,
}

impl Key {
    pub fn new(resource: &str, key: &str) -> Self {
        Self {
            resource: resource.to_string(),
            key: key.to_string(),
        }
    }
}

/// Something the runtime did, in order. Charter §14 M4 task 12 wants causal
/// traces; this is the record they are built from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Trace {
    RequestStarted {
        key: Key,
        attempt: u32,
    },
    RequestSucceeded {
        key: Key,
    },
    RequestFailed {
        key: Key,
        attempt: u32,
        error: String,
    },
    RequestDeduplicated {
        key: Key,
    },
    ServedFromCache {
        key: Key,
    },
    Cancelled {
        key: Key,
        reason: &'static str,
    },
    Subscribed {
        key: Key,
        subscribers: usize,
    },
    Unsubscribed {
        key: Key,
        subscribers: usize,
    },
    RetryScheduled {
        key: Key,
        attempt: u32,
        delay: Millis,
    },
    GaveUp {
        key: Key,
        attempts: u32,
    },
    RequestTimedOut {
        key: Key,
    },
    CommandOutcomeUnknown {
        idempotency_key: String,
    },
    CommandApplied {
        idempotency_key: String,
    },
    CommandDeduplicated {
        idempotency_key: String,
    },
}

/// How a resource behaves. The runtime half of the manifest the compiler
/// records in `pw-core::hir::Policy`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Manifest {
    pub resource: String,
    pub privacy: Privacy,
    /// How long a cached value stays usable.
    pub freshness: Millis,
    /// Attempts in total, not retries after the first. 1 means "do not retry".
    pub max_attempts: u32,
    /// Base backoff; attempt *n* waits `base * 2^(n-1)`, plus deterministic
    /// jitter derived from the key rather than from a random source — a test
    /// that cannot predict the delay cannot assert on it.
    pub retry_base: Millis,
    /// Whole-flight budget, including retries, measured by the injected clock.
    pub timeout: Millis,
}

impl Manifest {
    pub fn new(resource: &str) -> Self {
        Self {
            resource: resource.to_string(),
            privacy: Privacy::Public,
            freshness: 30_000,
            max_attempts: 1,
            retry_base: 100,
            timeout: 2_000,
        }
    }
    pub fn private(mut self) -> Self {
        self.privacy = Privacy::Private;
        self
    }
    pub fn freshness(mut self, ms: Millis) -> Self {
        self.freshness = ms;
        self
    }
    pub fn attempts(mut self, n: u32) -> Self {
        self.max_attempts = n;
        self
    }

    /// Backoff for `attempt`, doubling, with jitter that depends on the key.
    ///
    /// Jitter exists so a thundering herd does not retry in lockstep. Deriving
    /// it from the key rather than from randomness keeps it spread *across
    /// keys* while staying exactly predictable in a test.
    pub fn backoff(&self, attempt: u32, key: &str) -> Millis {
        let base = self
            .retry_base
            .saturating_mul(2u64.pow(attempt.saturating_sub(1).min(16)));
        let spread = base / 4;
        if spread == 0 {
            return base;
        }
        let h = key
            .bytes()
            .fold(0u64, |a, b| a.wrapping_mul(31).wrapping_add(b as u64));
        base.saturating_add(h % spread)
    }
}

#[derive(Debug, Clone)]
struct Entry {
    value: String,
    stored_at: Millis,
    privacy: Privacy,
}

/// Why a request lost permission to publish a result.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StopReason {
    NoSubscribers,
    Invalidated,
    TimedOut,
}

#[derive(Debug, Clone)]
enum QueryError {
    Load(String),
    Stopped(StopReason),
}

type QueryResult = Result<String, QueryError>;

/// The result and its wakeup predicate have one mutex. No user code runs here.
struct Completion<T> {
    owner: ThreadId,
    result: Mutex<Option<T>>,
    changed: Condvar,
}

impl<T: Clone> Completion<T> {
    fn new() -> Self {
        Self {
            owner: thread::current().id(),
            result: Mutex::new(None),
            changed: Condvar::new(),
        }
    }

    fn get(&self) -> Option<T> {
        self.result.lock().expect("completion").clone()
    }

    /// First terminal outcome wins, including cancellation racing completion.
    fn finish(&self, value: T) -> T {
        let mut result = self.result.lock().expect("completion");
        let terminal = result.get_or_insert(value).clone();
        self.changed.notify_all();
        terminal
    }

    fn wait(&self) -> T {
        let mut result = self.result.lock().expect("completion");
        loop {
            if let Some(value) = &*result {
                return value.clone();
            }
            result = self.changed.wait(result).expect("completion");
        }
    }
}

struct Flight {
    manifest: Manifest,
    started: Millis,
    completion: Completion<QueryResult>,
}

impl Flight {
    fn expired(&self, now: Millis) -> bool {
        now >= self.started.saturating_add(self.manifest.timeout)
    }
}

/// A synchronous adapter may inspect this between interruptible operations.
/// Signalling cancellation fences publication; stopping foreign I/O is the
/// adapter's responsibility. A cancelled request cannot be made valid again.
#[derive(Clone)]
pub struct Cancellation {
    flight: Arc<Flight>,
    clock: Clock,
}

impl Cancellation {
    pub fn reason(&self) -> Option<StopReason> {
        match self.flight.completion.get() {
            Some(Err(QueryError::Stopped(reason))) => Some(reason),
            Some(_) => None,
            None if self.flight.expired(self.clock.now()) => Some(StopReason::TimedOut),
            None => None,
        }
    }

    pub fn is_cancelled(&self) -> bool {
        self.reason().is_some()
    }
}

/// An uncertain command must not be silently retried as a new execution.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommandError {
    OutcomeUnknown,
    Reentrant,
}

type CommandResult = Result<String, CommandError>;

#[derive(Default)]
struct State {
    cache: HashMap<Key, Entry>,
    in_flight: HashMap<Key, Arc<Flight>>,
    subscribers: HashMap<Key, usize>,
    /// Reservations and results are kept for this runtime's lifetime.
    applied: HashMap<String, Arc<Completion<CommandResult>>>,
    trace: Vec<Trace>,
}

/// The runtime. Cheap to clone; every clone shares one state.
#[derive(Clone)]
pub struct Resources {
    state: Arc<Mutex<State>>,
    clock: Clock,
}

/// What a fetch did, so a caller can tell a cache hit from real work.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Fetched {
    FromCache(String),
    Fresh(String),
    /// Another caller's request was already running; this one joined it.
    Deduplicated(String),
    Failed(String),
    Cancelled(StopReason),
    TimedOut,
}

impl Resources {
    pub fn new(clock: Clock) -> Self {
        Self {
            state: Arc::new(Mutex::new(State::default())),
            clock,
        }
    }

    pub fn trace(&self) -> Vec<Trace> {
        self.state.lock().expect("state").trace.clone()
    }

    pub fn in_flight(&self, key: &Key) -> bool {
        self.state
            .lock()
            .expect("state")
            .in_flight
            .contains_key(key)
    }

    pub fn subscribers(&self, key: &Key) -> usize {
        self.state
            .lock()
            .expect("state")
            .subscribers
            .get(key)
            .copied()
            .unwrap_or(0)
    }

    /// Everything a public cache would serve. Used to assert that private
    /// values are not in it.
    pub fn public_cache_contents(&self) -> Vec<(Key, String)> {
        self.state
            .lock()
            .expect("state")
            .cache
            .iter()
            .filter(|(_, e)| e.privacy == Privacy::Public)
            .map(|(k, e)| (k.clone(), e.value.clone()))
            .collect()
    }

    /// Take a reference to a key. The value is kept while anyone holds one.
    pub fn subscribe(&self, key: &Key) -> Subscription {
        let mut st = self.state.lock().expect("state");
        let n = st.subscribers.entry(key.clone()).or_insert(0);
        *n += 1;
        let count = *n;
        st.trace.push(Trace::Subscribed {
            key: key.clone(),
            subscribers: count,
        });
        Subscription {
            rt: self.clone(),
            key: key.clone(),
            released: false,
        }
    }

    /// Fetch using a synchronous callback. See `fetch_cancellable` for adapters
    /// that can stop work after the initiating owner leaves.
    pub fn fetch(
        &self,
        manifest: &Manifest,
        key: &Key,
        mut load: impl FnMut(u32) -> Result<String, String>,
    ) -> Fetched {
        self.fetch_cancellable(manifest, key, |attempt, _| load(attempt))
    }

    /// Coalesce matching reads onto one owned flight and share its real result.
    /// User code runs without runtime locks. Deadlines use the injected clock,
    /// not a wall-clock preemption of an arbitrary synchronous callback.
    pub fn fetch_cancellable(
        &self,
        manifest: &Manifest,
        key: &Key,
        mut load: impl FnMut(u32, &Cancellation) -> Result<String, String>,
    ) -> Fetched {
        if manifest.resource != key.resource {
            return Fetched::Failed("manifest resource does not match the key".into());
        }
        let flight;
        {
            let mut st = self.state.lock().expect("state");
            let now = self.clock.now();
            if let Some(entry) = st.cache.get(key) {
                if entry.privacy != manifest.privacy {
                    return Fetched::Failed("cache privacy does not match the manifest".into());
                }
                if now.saturating_sub(entry.stored_at) < manifest.freshness {
                    let value = entry.value.clone();
                    st.trace.push(Trace::ServedFromCache { key: key.clone() });
                    return Fetched::FromCache(value);
                }
            }
            if let Some(existing) = st.in_flight.get(key).cloned() {
                if existing.manifest != *manifest {
                    return Fetched::Failed("in-flight policy does not match the manifest".into());
                }
                if existing.completion.owner == thread::current().id() {
                    return Fetched::Failed("a request cannot synchronously join itself".into());
                }
                st.trace
                    .push(Trace::RequestDeduplicated { key: key.clone() });
                drop(st);
                return Self::fetched(self.join_query(key, &existing), true);
            }
            flight = Arc::new(Flight {
                manifest: manifest.clone(),
                started: now,
                completion: Completion::new(),
            });
            st.in_flight.insert(key.clone(), flight.clone());
        }
        let cancellation = Cancellation {
            flight: flight.clone(),
            clock: self.clock.clone(),
        };
        let attempts = manifest.max_attempts.max(1);
        for attempt in 1..=attempts {
            {
                let mut st = self.state.lock().expect("state");
                if let Some(outcome) = Self::stopped(&mut st, key, &flight, self.clock.now()) {
                    return Self::fetched(outcome, false);
                }
                st.trace.push(Trace::RequestStarted {
                    key: key.clone(),
                    attempt,
                });
            }
            let result = catch_unwind(AssertUnwindSafe(|| load(attempt, &cancellation)));
            let result = match result {
                Ok(result) => result,
                Err(payload) => {
                    let _ = self.finish_query(
                        key,
                        &flight,
                        Err(QueryError::Load("loader panicked".into())),
                    );
                    resume_unwind(payload);
                }
            };
            let mut st = self.state.lock().expect("state");
            if let Some(outcome) = Self::stopped(&mut st, key, &flight, self.clock.now()) {
                return Self::fetched(outcome, false);
            }
            match result {
                Ok(value) => {
                    st.cache.insert(
                        key.clone(),
                        Entry {
                            value: value.clone(),
                            stored_at: self.clock.now(),
                            privacy: manifest.privacy,
                        },
                    );
                    st.trace.push(Trace::RequestSucceeded { key: key.clone() });
                    let outcome = Self::finish_locked(&mut st, key, &flight, Ok(value));
                    return Self::fetched(outcome, false);
                }
                Err(error) => {
                    st.trace.push(Trace::RequestFailed {
                        key: key.clone(),
                        attempt,
                        error: error.clone(),
                    });
                    if attempt == attempts {
                        st.trace.push(Trace::GaveUp {
                            key: key.clone(),
                            attempts,
                        });
                        let outcome = Self::finish_locked(
                            &mut st,
                            key,
                            &flight,
                            Err(QueryError::Load(error)),
                        );
                        return Self::fetched(outcome, false);
                    }
                    let delay = manifest.backoff(attempt, &key.key);
                    st.trace.push(Trace::RetryScheduled {
                        key: key.clone(),
                        attempt,
                        delay,
                    });
                    // This existing local-clock runtime simulates backoff. Do
                    // not mistake advancing logical time for sleeping real I/O.
                    let remaining = manifest
                        .timeout
                        .saturating_sub(self.clock.now().saturating_sub(flight.started));
                    self.clock.advance(delay.min(remaining));
                }
            }
        }
        unreachable!("at least one attempt; final attempt returns")
    }

    fn fetched(result: QueryResult, joined: bool) -> Fetched {
        match result {
            Ok(value) if joined => Fetched::Deduplicated(value),
            Ok(value) => Fetched::Fresh(value),
            Err(QueryError::Load(error)) => Fetched::Failed(error),
            Err(QueryError::Stopped(StopReason::TimedOut)) => Fetched::TimedOut,
            Err(QueryError::Stopped(reason)) => Fetched::Cancelled(reason),
        }
    }

    /// Lock order is always state -> completion. Joiners release completion
    /// before asking the runtime to expire a flight, preventing lock inversion.
    fn join_query(&self, key: &Key, flight: &Arc<Flight>) -> QueryResult {
        let mut result = flight.completion.result.lock().expect("completion");
        loop {
            if let Some(value) = &*result {
                return value.clone();
            }
            if flight.expired(self.clock.now()) {
                drop(result);
                return self.finish_query(
                    key,
                    flight,
                    Err(QueryError::Stopped(StopReason::TimedOut)),
                );
            }
            let (next, _) = flight
                .completion
                .changed
                .wait_timeout(result, Duration::from_millis(10))
                .expect("completion");
            result = next;
        }
    }

    fn stopped(
        st: &mut State,
        key: &Key,
        flight: &Arc<Flight>,
        now: Millis,
    ) -> Option<QueryResult> {
        if let Some(outcome) = flight.completion.get() {
            return Some(outcome);
        }
        if flight.expired(now) {
            return Some(Self::finish_locked(
                st,
                key,
                flight,
                Err(QueryError::Stopped(StopReason::TimedOut)),
            ));
        }
        None
    }

    fn finish_query(&self, key: &Key, flight: &Arc<Flight>, result: QueryResult) -> QueryResult {
        let mut st = self.state.lock().expect("state");
        Self::finish_locked(&mut st, key, flight, result)
    }

    fn finish_locked(
        st: &mut State,
        key: &Key,
        flight: &Arc<Flight>,
        result: QueryResult,
    ) -> QueryResult {
        if let Some(previous) = flight.completion.get() {
            return previous;
        }
        match &result {
            Err(QueryError::Stopped(StopReason::TimedOut)) => {
                st.trace.push(Trace::RequestTimedOut { key: key.clone() });
            }
            Err(QueryError::Stopped(reason)) => {
                st.trace.push(Trace::Cancelled {
                    key: key.clone(),
                    reason: match reason {
                        StopReason::NoSubscribers => "no subscribers remain",
                        StopReason::Invalidated => "resource invalidated",
                        StopReason::TimedOut => unreachable!(),
                    },
                });
            }
            _ => {}
        }
        let terminal = flight.completion.finish(result);
        // Revoked work must never delete a newer request for the same key.
        if st
            .in_flight
            .get(key)
            .is_some_and(|current| Arc::ptr_eq(current, flight))
        {
            st.in_flight.remove(key);
        }
        terminal
    }

    /// Compatibility wrapper. An unknown outcome is not a successful string.
    /// Use `try_command` to handle that state explicitly.
    pub fn command(&self, idempotency_key: &str, apply: impl FnOnce() -> String) -> String {
        self.try_command(idempotency_key, apply)
            .expect("command outcome is not known to be successful")
    }

    /// Reserve before execution, share completion, and never re-execute after
    /// an unwinding callback. This is process-local, not durable exactly-once.
    /// The host must bind identity to authority, operation and payload.
    pub fn try_command(
        &self,
        idempotency_key: &str,
        apply: impl FnOnce() -> String,
    ) -> CommandResult {
        let completion;
        {
            let mut st = self.state.lock().expect("state");
            if let Some(existing) = st.applied.get(idempotency_key).cloned() {
                if existing.owner == thread::current().id() && existing.get().is_none() {
                    return Err(CommandError::Reentrant);
                }
                st.trace.push(Trace::CommandDeduplicated {
                    idempotency_key: idempotency_key.into(),
                });
                drop(st);
                return existing.wait();
            }
            completion = Arc::new(Completion::new());
            st.applied
                .insert(idempotency_key.into(), completion.clone());
        }
        match catch_unwind(AssertUnwindSafe(apply)) {
            Ok(value) => {
                let mut st = self.state.lock().expect("state");
                st.trace.push(Trace::CommandApplied {
                    idempotency_key: idempotency_key.into(),
                });
                completion.finish(Ok(value))
            }
            Err(payload) => {
                {
                    let mut st = self.state.lock().expect("state");
                    st.trace.push(Trace::CommandOutcomeUnknown {
                        idempotency_key: idempotency_key.into(),
                    });
                    let _ = completion.finish(Err(CommandError::OutcomeUnknown));
                }
                resume_unwind(payload);
            }
        }
    }

    /// Invalidate both stored values and publication rights of outstanding work.
    pub fn invalidate(&self, resource: &str) {
        let mut st = self.state.lock().expect("state");
        st.cache.retain(|key, _| key.resource != resource);
        let obsolete: Vec<_> = st
            .in_flight
            .iter()
            .filter(|(key, _)| key.resource == resource)
            .map(|(key, flight)| (key.clone(), flight.clone()))
            .collect();
        for (key, flight) in obsolete {
            let _ = Self::finish_locked(
                &mut st,
                &key,
                &flight,
                Err(QueryError::Stopped(StopReason::Invalidated)),
            );
        }
    }

    fn release(&self, key: &Key) {
        let mut st = self.state.lock().expect("state");
        let Some(count) = st.subscribers.get_mut(key) else {
            return;
        };
        *count -= 1;
        let count = *count;
        if count == 0 {
            st.subscribers.remove(key);
        }
        st.trace.push(Trace::Unsubscribed {
            key: key.clone(),
            subscribers: count,
        });
        if count == 0
            && let Some(flight) = st.in_flight.get(key).cloned()
        {
            let _ = Self::finish_locked(
                &mut st,
                key,
                &flight,
                Err(QueryError::Stopped(StopReason::NoSubscribers)),
            );
        }
    }
}

/// A reference to a key. Dropping it releases the reference, and the last one
/// out cancels work nobody is waiting for.
pub struct Subscription {
    rt: Resources,
    key: Key,
    released: bool,
}

impl Subscription {
    pub fn key(&self) -> &Key {
        &self.key
    }
    /// Release early, rather than at end of scope.
    pub fn release(mut self) {
        self.rt.release(&self.key);
        self.released = true;
    }
}

impl Drop for Subscription {
    fn drop(&mut self) {
        if !self.released {
            self.rt.release(&self.key);
        }
    }
}

//! E4: the local synchronous resource runtime.
//!
//! A resource key owns one admitted request generation and its shared result.
//! Invalidating or releasing that generation fences publication and wakes its
//! waiters. It cannot preempt an arbitrary synchronous callback. Host adapter
//! cancellation, deadlines and durable command admission remain separate work.
//! See ADR-0018 and ADR-0027. These tests establish local behavior, not compiler
//! enforcement, tenant isolation or the E10-I generated source-to-host path.

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

/// Milliseconds since the runtime started.
pub type Millis = u64;

/// A clock advanced by a test or host. Overflow must not move it backwards.
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

/// Cache classification, not a principal or tenant identity. Production
/// partition identity is represented by `EntryIdentity`, not this enum.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Privacy {
    Public,
    Private,
}

/// A resource name plus its local key. This prototype requires its host to
/// provide consistent manifests and correctly scoped keys.
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

/// Events in admission/completion order. `Cancelled` means the generation's
/// result is no longer usable, not that arbitrary external work was stopped.
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
    CommandApplied {
        idempotency_key: String,
    },
    CommandDeduplicated {
        idempotency_key: String,
    },
}

/// Runtime policy data. Neither this crate nor the compiler owns the other
/// crate's execution model (ADR-0018).
#[derive(Debug, Clone)]
pub struct Manifest {
    pub resource: String,
    pub privacy: Privacy,
    pub freshness: Millis,
    /// Total attempts. One means no retry.
    pub max_attempts: u32,
    pub retry_base: Millis,
    /// Recorded policy only: synchronous callbacks are not preempted here.
    /// An adapter must enforce deadlines before this can be a runtime guarantee.
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

    /// Deterministic, key-derived jitter for this local test runtime. It spreads
    /// different keys, not independent clients retrying the same key.
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

/// One terminal result shared by everyone admitted to an operation. No callback
/// runs while either this cell or the runtime state is locked.
struct Completion {
    owner: ThreadId,
    result: Mutex<Option<Result<String, String>>>,
    ready: Condvar,
}

impl Completion {
    fn new() -> Self {
        Self {
            owner: thread::current().id(),
            result: Mutex::new(None),
            ready: Condvar::new(),
        }
    }

    fn finish(&self, outcome: Result<String, String>) -> Result<String, String> {
        let mut result = self.result.lock().expect("completion");
        let terminal = result.get_or_insert(outcome).clone();
        self.ready.notify_all();
        terminal
    }

    fn outcome(&self) -> Option<Result<String, String>> {
        self.result.lock().expect("completion").clone()
    }

    fn wait(&self) -> Result<String, String> {
        let mut result = self.result.lock().expect("completion");
        while result.is_none() {
            if self.owner == thread::current().id() {
                return Err("an operation cannot synchronously wait for itself".into());
            }
            result = self.ready.wait(result).expect("completion");
        }
        result.as_ref().expect("terminal result").clone()
    }
}

struct QueryFlight {
    privacy: Privacy,
    completion: Completion,
}

#[derive(Default)]
struct State {
    cache: HashMap<Key, Entry>,
    in_flight: HashMap<Key, Arc<QueryFlight>>,
    subscribers: HashMap<Key, usize>,
    /// Pending and completed commands, admitted before execution. Kept for this
    /// Resources instance's lifetime; neither durable nor a bounded replay log.
    applied: HashMap<String, Arc<Completion>>,
    trace: Vec<Trace>,
}

#[derive(Clone)]
pub struct Resources {
    state: Arc<Mutex<State>>,
    clock: Clock,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Fetched {
    FromCache(String),
    Fresh(String),
    /// The successful terminal result of the request this caller joined.
    Deduplicated(String),
    /// Loader failure, invalidation, cancellation or rejected local admission.
    Failed(String),
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

    pub fn public_cache_contents(&self) -> Vec<(Key, String)> {
        self.state
            .lock()
            .expect("state")
            .cache
            .iter()
            .filter(|(_, entry)| entry.privacy == Privacy::Public)
            .map(|(key, entry)| (key.clone(), entry.value.clone()))
            .collect()
    }

    pub fn subscribe(&self, key: &Key) -> Subscription {
        let mut state = self.state.lock().expect("state");
        let count = state.subscribers.entry(key.clone()).or_insert(0);
        *count += 1;
        let subscribers = *count;
        state.trace.push(Trace::Subscribed {
            key: key.clone(),
            subscribers,
        });
        Subscription {
            rt: self.clone(),
            key: key.clone(),
            released: false,
        }
    }

    /// Fetch or join one admitted request. A duplicate waits for the shared
    /// terminal result rather than returning unrelated cached/placeholder data.
    /// `load` is synchronous and must not depend cyclically on another waiter.
    pub fn fetch(
        &self,
        manifest: &Manifest,
        key: &Key,
        mut load: impl FnMut(u32) -> Result<String, String>,
    ) -> Fetched {
        let flight = {
            let mut state = self.state.lock().expect("state");
            if let Some(entry) = state.cache.get(key) {
                if entry.privacy != manifest.privacy {
                    return Fetched::Failed("resource privacy classification mismatch".into());
                }
                if self.clock.now().saturating_sub(entry.stored_at) < manifest.freshness {
                    let value = entry.value.clone();
                    state
                        .trace
                        .push(Trace::ServedFromCache { key: key.clone() });
                    return Fetched::FromCache(value);
                }
            }
            if let Some(running) = state.in_flight.get(key).cloned() {
                if running.privacy != manifest.privacy {
                    return Fetched::Failed("resource privacy classification mismatch".into());
                }
                state
                    .trace
                    .push(Trace::RequestDeduplicated { key: key.clone() });
                drop(state);
                return match running.completion.wait() {
                    Ok(value) => Fetched::Deduplicated(value),
                    Err(error) => Fetched::Failed(error),
                };
            }
            let flight = Arc::new(QueryFlight {
                privacy: manifest.privacy,
                completion: Completion::new(),
            });
            state.in_flight.insert(key.clone(), Arc::clone(&flight));
            flight
        };

        let attempts = manifest.max_attempts.max(1);
        let mut last_error = String::new();
        for attempt in 1..=attempts {
            {
                let mut state = self.state.lock().expect("state");
                if !Self::owns_flight(&state, key, &flight) {
                    return Self::fetch_outcome(Self::obsolete_outcome(&flight));
                }
                state.trace.push(Trace::RequestStarted {
                    key: key.clone(),
                    attempt,
                });
            }
            match catch_unwind(AssertUnwindSafe(|| load(attempt))) {
                Ok(Ok(value)) => {
                    return Self::fetch_outcome(self.finish_fetch(
                        key,
                        &flight,
                        Ok(value),
                        attempt,
                    ));
                }
                Ok(Err(error)) => {
                    let mut state = self.state.lock().expect("state");
                    if !Self::owns_flight(&state, key, &flight) {
                        return Self::fetch_outcome(Self::obsolete_outcome(&flight));
                    }
                    state.trace.push(Trace::RequestFailed {
                        key: key.clone(),
                        attempt,
                        error: error.clone(),
                    });
                    last_error = error;
                    if attempt < attempts {
                        let delay = manifest.backoff(attempt, &key.key);
                        state.trace.push(Trace::RetryScheduled {
                            key: key.clone(),
                            attempt,
                            delay,
                        });
                        drop(state);
                        self.clock.advance(delay);
                    }
                }
                Err(panic) => {
                    let _ = self.finish_fetch(
                        key,
                        &flight,
                        Err("resource loader panicked".into()),
                        attempt,
                    );
                    resume_unwind(panic);
                }
            }
        }
        Self::fetch_outcome(self.finish_fetch(key, &flight, Err(last_error), attempts))
    }

    fn owns_flight(state: &State, key: &Key, flight: &Arc<QueryFlight>) -> bool {
        state
            .in_flight
            .get(key)
            .is_some_and(|current| Arc::ptr_eq(current, flight))
    }

    fn obsolete_outcome(flight: &QueryFlight) -> Result<String, String> {
        flight
            .completion
            .outcome()
            .unwrap_or_else(|| Err("request is no longer admitted".into()))
    }

    fn fetch_outcome(outcome: Result<String, String>) -> Fetched {
        match outcome {
            Ok(value) => Fetched::Fresh(value),
            Err(error) => Fetched::Failed(error),
        }
    }

    fn finish_fetch(
        &self,
        key: &Key,
        flight: &Arc<QueryFlight>,
        outcome: Result<String, String>,
        attempts: u32,
    ) -> Result<String, String> {
        let mut state = self.state.lock().expect("state");
        if !Self::owns_flight(&state, key, flight) {
            return Self::obsolete_outcome(flight);
        }
        state.in_flight.remove(key);
        match &outcome {
            Ok(value) => {
                state.cache.insert(
                    key.clone(),
                    Entry {
                        value: value.clone(),
                        stored_at: self.clock.now(),
                        privacy: flight.privacy,
                    },
                );
                state
                    .trace
                    .push(Trace::RequestSucceeded { key: key.clone() });
            }
            Err(_) => state.trace.push(Trace::GaveUp {
                key: key.clone(),
                attempts,
            }),
        }
        flight.completion.finish(outcome)
    }

    /// Apply once per key in this Resources instance, including overlapping
    /// calls. The host owns key scope and payload agreement. This is not a
    /// crash-safe transaction protocol or a distributed exactly-once guarantee.
    ///
    /// A panic is propagated, and its uncertain outcome retained so a later
    /// duplicate cannot silently execute a possibly committed mutation again.
    pub fn command(&self, idempotency_key: &str, apply: impl FnOnce() -> String) -> String {
        let completion = {
            let mut state = self.state.lock().expect("state");
            if let Some(running) = state.applied.get(idempotency_key).cloned() {
                state.trace.push(Trace::CommandDeduplicated {
                    idempotency_key: idempotency_key.to_string(),
                });
                drop(state);
                return running
                    .wait()
                    .unwrap_or_else(|error| panic!("command not replayed: {error}"));
            }
            let completion = Arc::new(Completion::new());
            state
                .applied
                .insert(idempotency_key.to_string(), Arc::clone(&completion));
            completion
        };
        match catch_unwind(AssertUnwindSafe(apply)) {
            Ok(result) => {
                let mut state = self.state.lock().expect("state");
                state.trace.push(Trace::CommandApplied {
                    idempotency_key: idempotency_key.to_string(),
                });
                let _ = completion.finish(Ok(result.clone()));
                result
            }
            Err(panic) => {
                let _ = completion.finish(Err("command outcome is unknown after a panic".into()));
                resume_unwind(panic);
            }
        }
    }

    /// Remove cached values and terminate current generations. Their callbacks
    /// may still run, but cannot publish or remove a replacement's state.
    pub fn invalidate(&self, resource: &str) {
        let mut state = self.state.lock().expect("state");
        state.cache.retain(|key, _| key.resource != resource);
        let keys: Vec<_> = state
            .in_flight
            .keys()
            .filter(|key| key.resource == resource)
            .cloned()
            .collect();
        for key in keys {
            if let Some(flight) = state.in_flight.remove(&key) {
                let _ = flight.completion.finish(Err("resource invalidated".into()));
                state.trace.push(Trace::Cancelled {
                    key,
                    reason: "resource invalidated",
                });
            }
        }
    }

    fn release(&self, key: &Key) {
        let mut state = self.state.lock().expect("state");
        let count = state.subscribers.entry(key.clone()).or_insert(0);
        *count = count.saturating_sub(1);
        let subscribers = *count;
        state.trace.push(Trace::Unsubscribed {
            key: key.clone(),
            subscribers,
        });
        if subscribers == 0 {
            state.subscribers.remove(key);
            if let Some(flight) = state.in_flight.remove(key) {
                let _ = flight
                    .completion
                    .finish(Err("no subscribers remain".into()));
                state.trace.push(Trace::Cancelled {
                    key: key.clone(),
                    reason: "no subscribers remain",
                });
            }
        }
    }
}

/// A reference to a key. The last release fences its active generation.
/// It does not preempt an arbitrary synchronous loader (ADR-0027).
pub struct Subscription {
    rt: Resources,
    key: Key,
    released: bool,
}

impl Subscription {
    pub fn key(&self) -> &Key {
        &self.key
    }
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

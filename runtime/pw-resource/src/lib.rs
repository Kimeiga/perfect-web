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

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

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
        self.0.fetch_add(ms, Ordering::SeqCst);
    }
}

/// Which cache a value may be stored in.
///
/// The separation is the point: charter §7.8 and the M4 gate both say private
/// state must never appear in public cache output, and a single cache with a
/// flag would make that a review question rather than a structural one.
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
    CommandApplied {
        idempotency_key: String,
    },
    CommandDeduplicated {
        idempotency_key: String,
    },
}

/// How a resource behaves. The runtime half of the manifest the compiler
/// records in `pw-core::hir::Policy`.
#[derive(Debug, Clone)]
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
        let base = self.retry_base * 2u64.pow(attempt.saturating_sub(1).min(16));
        let spread = base / 4;
        if spread == 0 {
            return base;
        }
        let h = key
            .bytes()
            .fold(0u64, |a, b| a.wrapping_mul(31).wrapping_add(b as u64));
        base + h % spread
    }
}

#[derive(Debug, Clone)]
struct Entry {
    value: String,
    stored_at: Millis,
    privacy: Privacy,
}

#[derive(Default)]
struct State {
    cache: HashMap<Key, Entry>,
    /// Keys with a request currently running. The heart of "one in-flight
    /// request per key".
    in_flight: HashMap<Key, u32>,
    subscribers: HashMap<Key, usize>,
    /// Idempotency keys of commands already applied.
    applied: HashMap<String, String>,
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

    /// Fetch a key, running `load` only if no usable cached value exists and no
    /// request for the same key is already running.
    ///
    /// `load` returns `Err` to fail an attempt; the manifest decides whether to
    /// retry.
    pub fn fetch(
        &self,
        manifest: &Manifest,
        key: &Key,
        mut load: impl FnMut(u32) -> Result<String, String>,
    ) -> Fetched {
        {
            let mut st = self.state.lock().expect("state");
            let now = self.clock.now();

            if let Some(e) = st
                .cache
                .get(key)
                .filter(|e| now.saturating_sub(e.stored_at) < manifest.freshness)
            {
                let v = e.value.clone();
                st.trace.push(Trace::ServedFromCache { key: key.clone() });
                return Fetched::FromCache(v);
            }

            // One in-flight request per key. A recomputation storm hits this
            // branch, which is the M4 gate item: many callers, one request.
            if st.in_flight.contains_key(key) {
                st.trace
                    .push(Trace::RequestDeduplicated { key: key.clone() });
                let v = st
                    .cache
                    .get(key)
                    .map(|e| e.value.clone())
                    .unwrap_or_default();
                return Fetched::Deduplicated(v);
            }
            st.in_flight.insert(key.clone(), 1);
        }

        let mut last_error = String::new();
        for attempt in 1..=manifest.max_attempts.max(1) {
            self.state
                .lock()
                .expect("state")
                .trace
                .push(Trace::RequestStarted {
                    key: key.clone(),
                    attempt,
                });

            match load(attempt) {
                Ok(value) => {
                    let mut st = self.state.lock().expect("state");
                    st.cache.insert(
                        key.clone(),
                        Entry {
                            value: value.clone(),
                            stored_at: self.clock.now(),
                            privacy: manifest.privacy,
                        },
                    );
                    st.in_flight.remove(key);
                    st.trace.push(Trace::RequestSucceeded { key: key.clone() });
                    return Fetched::Fresh(value);
                }
                Err(e) => {
                    let mut st = self.state.lock().expect("state");
                    st.trace.push(Trace::RequestFailed {
                        key: key.clone(),
                        attempt,
                        error: e.clone(),
                    });
                    last_error = e;
                    if attempt < manifest.max_attempts {
                        let delay = manifest.backoff(attempt, &key.key);
                        st.trace.push(Trace::RetryScheduled {
                            key: key.clone(),
                            attempt,
                            delay,
                        });
                        drop(st);
                        self.clock.advance(delay);
                    }
                }
            }
        }

        let mut st = self.state.lock().expect("state");
        st.in_flight.remove(key);
        st.trace.push(Trace::GaveUp {
            key: key.clone(),
            attempts: manifest.max_attempts.max(1),
        });
        Fetched::Failed(last_error)
    }

    /// Apply a command once per idempotency key.
    ///
    /// The M4 gate: duplicate `add_to_cart` calls with the same interaction ID
    /// produce one logical mutation. A double-click, a retry after a dropped
    /// response, and a reconnect replay are all the same event.
    pub fn command(&self, idempotency_key: &str, apply: impl FnOnce() -> String) -> String {
        {
            let mut st = self.state.lock().expect("state");
            if let Some(prev) = st.applied.get(idempotency_key) {
                let prev = prev.clone();
                st.trace.push(Trace::CommandDeduplicated {
                    idempotency_key: idempotency_key.to_string(),
                });
                return prev;
            }
        }
        let result = apply();
        let mut st = self.state.lock().expect("state");
        st.applied
            .insert(idempotency_key.to_string(), result.clone());
        st.trace.push(Trace::CommandApplied {
            idempotency_key: idempotency_key.to_string(),
        });
        result
    }

    /// Drop cached values for a resource, e.g. after a command invalidates it.
    pub fn invalidate(&self, resource: &str) {
        let mut st = self.state.lock().expect("state");
        st.cache.retain(|k, _| k.resource != resource);
    }

    fn release(&self, key: &Key) {
        let mut st = self.state.lock().expect("state");
        let n = st.subscribers.entry(key.clone()).or_insert(0);
        *n = n.saturating_sub(1);
        let count = *n;
        st.trace.push(Trace::Unsubscribed {
            key: key.clone(),
            subscribers: count,
        });
        // Nobody is watching, so nothing should still be running for it. This
        // is the "navigation cancels unneeded work" gate item.
        if count == 0 && st.in_flight.remove(key).is_some() {
            st.trace.push(Trace::Cancelled {
                key: key.clone(),
                reason: "no subscribers remain",
            });
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

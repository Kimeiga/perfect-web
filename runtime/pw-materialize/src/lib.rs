//! E6 — the materializer (charter §14 M6 tasks 2, 4, 5, 6, 8).
//!
//! ISR regenerates a route when a timer expires. This regenerates a fragment
//! when something it depends on changed, and the difference is not efficiency —
//! it is that the second one can be **wrong in a way you can see**. A timer that
//! is too long serves stale content and a timer that is too short serves the
//! same content again; neither is an error, so neither is ever fixed.
//!
//! # The three things that are actually hard
//!
//! **Losing an event.** An event published after a commit can be lost between
//! the two, and then nothing regenerates — this milestone's failure, arriving
//! through the door labelled "reliable messaging". Published before the commit,
//! it can describe a change that then rolls back. So the event and the state
//! change are ONE transaction, in one store (ADR-0019).
//!
//! **Delivering it twice.** Avoided rather than prevented. An event here is *a
//! reason to look at the graph*, never *an instruction to regenerate*, so
//! processing one twice reaches the same conclusion twice and the second
//! regeneration is skipped because the first already produced an entry at that
//! version. Exactly-once delivery is the expensive thing, and an idempotent
//! consumer does not need it.
//!
//! **Everyone regenerating at once.** A popular entry expiring is N concurrent
//! readers finding a miss and N concurrent regenerations of the same thing,
//! which is how a cache turns a traffic spike into an origin outage.
//! [`Materializer::regenerate`] admits one per key and the rest wait for its
//! answer.
//!
//! # What is measured here and what is not
//!
//! Like `pw-tasks` (ADR-0016) this is **behaviour**. A passing test here does
//! not make misuse a compile error — the compiler half of E6 is `pw-core`'s
//! graph and its three rules. What this proves is that the policy those rules
//! check can be honoured, and, where it is violated, what breaks.

use std::collections::BTreeMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Condvar, Mutex};

use rusqlite::{Connection, params};

pub mod graph;
pub use graph::{Dimension, EdgeKind, Graph};

/// Milliseconds since the runtime started. A test owns it, as in `pw-resource`:
/// freshness and duration are both time, and a test that sleeps is a test that
/// gets muted.
#[derive(Clone, Default)]
pub struct Clock(Arc<AtomicU64>);

impl Clock {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn now(&self) -> u64 {
        self.0.load(Ordering::SeqCst)
    }
    pub fn advance(&self, ms: u64) {
        self.0.fetch_add(ms, Ordering::SeqCst);
    }
}

/// One typed event, as a command emitted it.
///
/// `name` is the declaration's path in the graph — `Events.MenuChanged` — so a
/// consumer joins against the graph rather than against a naming convention.
/// `args` are its parameters in declaration order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Event {
    pub name: String,
    pub args: Vec<String>,
}

impl Event {
    pub fn new(name: &str, args: &[&str]) -> Event {
        Event {
            name: name.to_string(),
            args: args.iter().map(|a| a.to_string()).collect(),
        }
    }
}

/// A row in the outbox, once committed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Committed {
    pub id: i64,
    pub event: Event,
}

/// What one entry of a materialized fragment is keyed by.
///
/// The fragment, the argument that identifies the instance, and every dimension
/// the declaration says the key separates. Dimensions are part of the key
/// rather than part of the value because two readers who should see different
/// content must not share an entry — and a cache serving the wrong reader does
/// so with a HIT, which no invalidation test finds.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct EntryKey {
    pub fragment: String,
    pub key: Vec<String>,
    /// `(dimension, value)`, sorted, so two spellings of one key are one key.
    pub dimensions: Vec<(String, String)>,
}

impl EntryKey {
    pub fn new(fragment: &str, key: &[&str]) -> EntryKey {
        EntryKey {
            fragment: fragment.to_string(),
            key: key.iter().map(|k| k.to_string()).collect(),
            dimensions: Vec::new(),
        }
    }

    pub fn with(mut self, dimension: &str, value: &str) -> EntryKey {
        self.dimensions
            .push((dimension.to_string(), value.to_string()));
        self.dimensions.sort();
        self.dimensions.dedup_by(|a, b| a.0 == b.0);
        self
    }

    /// The **logical** key: what the application decided.
    ///
    /// Not a storage identity. `Materializer::physical` adds the compatibility
    /// generation, which is the platform's, and this type deliberately cannot
    /// produce one on its own — see the note there.
    fn logical(&self) -> String {
        let dims: Vec<String> = self
            .dimensions
            .iter()
            .map(|(d, v)| format!("{d}={v}"))
            .collect();
        format!(
            "{}({})[{}]",
            self.fragment,
            self.key.join(","),
            dims.join(",")
        )
    }
}

/// A materialized entry, with what produced it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    pub body: String,
    /// The outbox id that caused this regeneration, or `None` for the first
    /// materialization. Charter §14 M6 task 5 — "records causality".
    pub because: Option<i64>,
    pub generated_at: u64,
    pub duration_ms: u64,
    /// Set when the entry is known to be out of date and has not been replaced.
    pub stale: bool,
}

/// What a read got, and how.
///
/// A four-way answer rather than `Option<String>`, because the difference
/// between "fresh", "known stale but policy allows it" and "there is nothing"
/// is what charter §14 M6 gate item 3 is about, and a caller that cannot see
/// the difference cannot follow the policy.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Read {
    Fresh(String),
    /// Serving an entry that is known out of date, because `fallback
    /// last_known_good` says that is better than an error.
    LastKnownGood(String),
    /// The entry is stale and the policy does not allow serving it.
    Stale,
    Missing,
}

/// What a regeneration attempt did.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Regenerated {
    /// This call did the work.
    Fresh,
    /// Another call was already doing it; this one waited and took its answer.
    /// Charter §14 M6 gate item 5 — bounded regeneration concurrency.
    Coalesced,
    /// Nothing to do: an entry at this version already exists. The duplicate
    /// event's answer, and the reason duplicates are harmless.
    AlreadyCurrent,
    /// The producer failed. The previous entry, if any, is left in place.
    Failed,
}

/// How stale an entry may be served.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Fallback {
    /// `fallback last_known_good`: serve it and say so.
    LastKnownGood,
    /// Serve nothing rather than something out of date.
    None,
}

/// The declared policy for one fragment, from its `materialize` block.
#[derive(Debug, Clone)]
pub struct FragmentPolicy {
    pub fallback: Fallback,
    /// `stampede single_flight`. False admits every caller, which exists so a
    /// test can show the difference rather than assert it.
    pub single_flight: bool,
}

impl Default for FragmentPolicy {
    fn default() -> Self {
        FragmentPolicy {
            fallback: Fallback::LastKnownGood,
            single_flight: true,
        }
    }
}

/// Something the materializer did, for the record.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Trace {
    Emitted {
        id: i64,
        event: String,
    },
    Consumed {
        id: i64,
        event: String,
    },
    /// An event arrived and matched no edge. Recorded rather than dropped: an
    /// event nothing listens for is either a graph that is missing an edge or
    /// a command emitting something nobody wants, and both are worth seeing.
    NoSubscriber {
        id: i64,
        event: String,
    },
    Invalidated {
        entry: String,
        because: i64,
    },
    Regenerated {
        entry: String,
        duration_ms: u64,
    },
    Coalesced {
        entry: String,
    },
    AlreadyCurrent {
        entry: String,
    },
    Failed {
        entry: String,
        error: String,
    },
    ServedLastKnownGood {
        entry: String,
    },
}

/// The store: application state, the outbox, and the materialized entries.
pub struct Materializer {
    db: Mutex<Connection>,
    clock: Clock,
    /// The compatibility generation every entry is namespaced by.
    compatibility: String,
    entries: Mutex<BTreeMap<String, Entry>>,
    /// Keys a regeneration is in flight for, and a condition to wait on.
    in_flight: Mutex<Vec<String>>,
    done: Condvar,
    trace: Mutex<Vec<Trace>>,
    policies: Mutex<BTreeMap<String, FragmentPolicy>>,
}

impl Materializer {
    /// An in-memory store, for one compatibility generation.
    ///
    /// The generation is a **constructor parameter**, so a materializer cannot
    /// exist without one. It used to be a cache-key dimension the author wrote
    /// as `code_version included_in_key`, with `PW5102` making the omission an
    /// error.
    ///
    /// Architect ruling, 2026-08-06:
    ///
    /// > If every shared materialization must be separated across incompatible
    /// > code generations, that is platform mechanism, not application policy.
    ///
    /// The same reasoning as `Authorised` in E7V: the way to guarantee
    /// something is not to check that a caller did it, it is to leave no way to
    /// skip it. A rule saying "remember to write this" is a rule that measures
    /// whether people remember.
    ///
    /// The schema is created here so a test never has a half-built database.
    pub fn new(clock: Clock, compatibility: &str) -> Materializer {
        let db = Connection::open_in_memory().expect("open in-memory sqlite");
        db.execute_batch(
            "CREATE TABLE state (
                 name  TEXT PRIMARY KEY,
                 value TEXT NOT NULL
             );
             CREATE TABLE outbox (
                 id          INTEGER PRIMARY KEY AUTOINCREMENT,
                 event       TEXT NOT NULL,
                 args        TEXT NOT NULL,
                 consumed_at INTEGER
             );",
        )
        .expect("schema");
        Materializer {
            db: Mutex::new(db),
            clock,
            compatibility: compatibility.to_string(),
            entries: Mutex::new(BTreeMap::new()),
            in_flight: Mutex::new(Vec::new()),
            done: Condvar::new(),
            trace: Mutex::new(Vec::new()),
            policies: Mutex::new(BTreeMap::new()),
        }
    }

    /// The connection, recovering a poisoned lock.
    ///
    /// Deliberately not the same treatment as the in-memory maps. What this
    /// mutex protects is a SQLite connection, and what keeps its contents
    /// consistent across a panic is the TRANSACTION: an unwind drops the
    /// `Transaction` and rolls it back, so the database on the other side of
    /// the panic is exactly the database from before it.
    ///
    /// Poisoning would be right if a panic could leave the data half-written.
    /// It cannot, and treating the store as destroyed would mean a command
    /// that panicked took the materializer down with it — which is the
    /// availability failure charter §14 M6 task 8 asks about, caused by the
    /// mechanism meant to detect it.
    fn db(&self) -> std::sync::MutexGuard<'_, Connection> {
        self.db.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// The storage identity: the application's logical key inside this
    /// build's compatibility namespace.
    ///
    /// Every read and write goes through here, which is what makes the
    /// namespace unforgettable rather than merely required. Two generations of
    /// the same program address different entries even for an identical logical
    /// key — the property `PW5102` used to ask the author to guarantee.
    fn physical(&self, key: &EntryKey) -> String {
        format!("{}|{}", self.compatibility, key.logical())
    }

    /// The generation this materializer serves.
    pub fn compatibility(&self) -> &str {
        &self.compatibility
    }

    pub fn declare(&self, fragment: &str, policy: FragmentPolicy) {
        self.policies
            .lock()
            .expect("policies")
            .insert(fragment.to_string(), policy);
    }

    fn policy(&self, fragment: &str) -> FragmentPolicy {
        self.policies
            .lock()
            .expect("policies")
            .get(fragment)
            .cloned()
            .unwrap_or_default()
    }

    pub fn trace(&self) -> Vec<Trace> {
        self.trace.lock().expect("trace").clone()
    }

    fn record(&self, t: Trace) {
        self.trace.lock().expect("trace").push(t);
    }

    /// Charter §14 M6 task 4: the state change and the event, atomically.
    ///
    /// `apply` runs inside the transaction and returns the events its change
    /// implies. If it panics or returns `Err`, the transaction rolls back and
    /// **neither** the state nor the event survives — which is the property
    /// this whole design exists for, and the one the crash-injection test
    /// attacks directly.
    pub fn command<E>(
        &self,
        apply: impl FnOnce(&rusqlite::Transaction<'_>) -> Result<Vec<Event>, E>,
    ) -> Result<Vec<i64>, E> {
        let mut db = self.db();
        let tx = db.transaction().expect("begin");
        let events = match apply(&tx) {
            Ok(e) => e,
            Err(e) => {
                // Explicit, though the `Drop` would also roll back. A reader
                // should not have to know that to know what happens here.
                tx.rollback().expect("rollback");
                return Err(e);
            }
        };
        let mut ids = Vec::new();
        for ev in &events {
            tx.execute(
                "INSERT INTO outbox (event, args, consumed_at) VALUES (?1, ?2, NULL)",
                params![ev.name, ev.args.join("\u{1}")],
            )
            .expect("insert outbox");
            ids.push(tx.last_insert_rowid());
        }
        tx.commit().expect("commit");
        for (id, ev) in ids.iter().zip(&events) {
            self.record(Trace::Emitted {
                id: *id,
                event: ev.name.clone(),
            });
        }
        Ok(ids)
    }

    /// Read a state value, for tests that check the state and the outbox agree.
    pub fn state(&self, name: &str) -> Option<String> {
        let db = self.db();
        db.query_row(
            "SELECT value FROM state WHERE name = ?1",
            params![name],
            |r| r.get::<_, String>(0),
        )
        .ok()
    }

    /// Events committed and not yet consumed, oldest first.
    pub fn pending(&self) -> Vec<Committed> {
        let db = self.db();
        let mut stmt = db
            .prepare("SELECT id, event, args FROM outbox WHERE consumed_at IS NULL ORDER BY id")
            .expect("prepare");
        let rows = stmt
            .query_map([], |r| {
                let args: String = r.get(2)?;
                Ok(Committed {
                    id: r.get(0)?,
                    event: Event {
                        name: r.get(1)?,
                        args: if args.is_empty() {
                            Vec::new()
                        } else {
                            args.split('\u{1}').map(str::to_string).collect()
                        },
                    },
                })
            })
            .expect("query");
        rows.map(|r| r.expect("row")).collect()
    }

    fn consume(&self, id: i64) {
        let db = self.db();
        db.execute(
            "UPDATE outbox SET consumed_at = ?2 WHERE id = ?1",
            params![id, self.clock.now() as i64],
        )
        .expect("consume");
    }

    /// Read an entry, following the fragment's declared fallback policy.
    pub fn read(&self, key: &EntryKey) -> Read {
        let entries = self.entries.lock().expect("entries");
        match entries.get(&self.physical(key)) {
            None => Read::Missing,
            Some(e) if !e.stale => Read::Fresh(e.body.clone()),
            Some(e) => match self.policy(&key.fragment).fallback {
                Fallback::LastKnownGood => {
                    let body = e.body.clone();
                    drop(entries);
                    self.record(Trace::ServedLastKnownGood {
                        entry: self.physical(key),
                    });
                    Read::LastKnownGood(body)
                }
                Fallback::None => Read::Stale,
            },
        }
    }

    pub fn entry(&self, key: &EntryKey) -> Option<Entry> {
        self.entries
            .lock()
            .expect("entries")
            .get(&self.physical(key))
            .cloned()
    }

    /// Every entry that currently exists, by its key string.
    ///
    /// The regeneration test's instrument: "everything that does NOT recompute"
    /// is only checkable against the full set.
    pub fn all_entries(&self) -> BTreeMap<String, Entry> {
        self.entries.lock().expect("entries").clone()
    }

    /// Mark an entry out of date. Idempotent — marking twice is one mark, which
    /// is what makes a duplicate event harmless.
    pub fn invalidate(&self, key: &EntryKey, because: i64) {
        let id = self.physical(key);
        let mut entries = self.entries.lock().expect("entries");
        if let Some(e) = entries.get_mut(&id)
            && !e.stale
        {
            e.stale = true;
            drop(entries);
            self.record(Trace::Invalidated { entry: id, because });
        }
    }

    /// Produce an entry, admitting one caller per key.
    ///
    /// `produce` returns the body, or an error the materializer records without
    /// destroying what is already there — charter §14 M6 task 8's "regeneration
    /// failure". A failed regeneration must not turn a stale entry into no
    /// entry, because the stale one is what `last_known_good` serves.
    pub fn regenerate(
        &self,
        key: &EntryKey,
        because: Option<i64>,
        produce: impl FnOnce() -> Result<String, String>,
    ) -> Regenerated {
        let id = self.physical(key);

        // Nothing to do: an entry exists and is not stale. The duplicate
        // event's path, and the reason duplicates cost nothing.
        if let Some(e) = self.entries.lock().expect("entries").get(&id)
            && !e.stale
        {
            self.record(Trace::AlreadyCurrent { entry: id.clone() });
            return Regenerated::AlreadyCurrent;
        }

        let single_flight = self.policy(&key.fragment).single_flight;
        // Held for the duration of `produce`. Its `Drop` clears the key and
        // wakes the waiters even when `produce` panics — without it, one
        // panicking producer strands its key in flight and every later reader
        // of that entry waits forever. A `catch_unwind` would also work and
        // would additionally have to decide what to return; the guard does not
        // have to decide anything, because the panic keeps propagating.
        let _flight;
        if single_flight {
            let mut flight = self.in_flight.lock().expect("in_flight");
            if flight.contains(&id) {
                // Someone else is producing this exact entry. Wait for their
                // answer instead of computing the same thing again — the
                // difference between a spike in readers and a spike in origin
                // load.
                while flight.contains(&id) {
                    flight = self.done.wait(flight).expect("wait");
                }
                self.record(Trace::Coalesced { entry: id.clone() });
                return Regenerated::Coalesced;
            }
            flight.push(id.clone());
            drop(flight);
            _flight = Some(FlightGuard {
                owner: self,
                id: id.clone(),
            });
        } else {
            _flight = None;
        }

        let started = self.clock.now();
        let produced = produce();
        let finished = self.clock.now();

        match produced {
            Ok(body) => {
                self.entries.lock().expect("entries").insert(
                    id.clone(),
                    Entry {
                        body,
                        because,
                        generated_at: finished,
                        duration_ms: finished.saturating_sub(started),
                        stale: false,
                    },
                );
                self.record(Trace::Regenerated {
                    entry: id.clone(),
                    duration_ms: finished.saturating_sub(started),
                });
                Regenerated::Fresh
            }
            Err(error) => {
                self.record(Trace::Failed {
                    entry: id.clone(),
                    error,
                });
                Regenerated::Failed
            }
        }
    }

    /// Consume every committed event, invalidating what the graph says it
    /// affects. Charter §14 M6 task 5, first three lines.
    ///
    /// Invalidation and regeneration are separate on purpose: `regenerate
    /// on_invalidation` is one declared policy and `regenerate on_read` is
    /// another, and a materializer that always did both could only implement
    /// the first.
    ///
    /// Coalescing happens here, not at the reader: N events naming one entry
    /// mark it once, so a burst of `InventoryChanged` for one store is one
    /// regeneration rather than N.
    pub fn drain(&self, g: &Graph, instances: &[EntryKey]) -> Vec<(EntryKey, i64)> {
        let mut invalidated: Vec<(EntryKey, i64)> = Vec::new();
        for c in self.pending() {
            // Every listener, not only the materializations: an `EntryKey`
            // names a cached node, and a cached query is one.
            let fragments = g.listeners_for(&c.event.name);
            if fragments.is_empty() {
                self.record(Trace::NoSubscriber {
                    id: c.id,
                    event: c.event.name.clone(),
                });
                self.consume(c.id);
                continue;
            }
            for key in instances {
                if !fragments.contains(&key.fragment) {
                    continue;
                }
                // The narrowing the whole milestone is about. An event's
                // arguments are matched against the entry's key, so
                // `MenuChanged(store_47)` reaches store 47's fragment and no
                // other store's — and an event with NO arguments matches every
                // instance, which is the honest reading of "the menu changed"
                // with nothing said about which.
                if !c.event.args.is_empty() && !c.event.args.iter().all(|a| key.key.contains(a)) {
                    continue;
                }
                let already = self.entry(key).map(|e| e.stale).unwrap_or(true);
                self.invalidate(key, c.id);
                if !already {
                    invalidated.push((key.clone(), c.id));
                }
            }
            self.record(Trace::Consumed {
                id: c.id,
                event: c.event.name.clone(),
            });
            self.consume(c.id);
        }
        invalidated
    }
}

/// Clears a key from the in-flight set on the way out, panic or not.
struct FlightGuard<'a> {
    owner: &'a Materializer,
    id: String,
}

impl Drop for FlightGuard<'_> {
    fn drop(&mut self) {
        let mut flight = self
            .owner
            .in_flight
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        flight.retain(|k| k != &self.id);
        self.owner.done.notify_all();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_entry_key_is_its_dimensions_too() {
        let a = EntryKey::new("F", &["47"]).with("locale", "en");
        let b = EntryKey::new("F", &["47"]).with("locale", "fr");
        assert_ne!(a.logical(), b.logical(), "two locales are two entries");

        // Order of declaration must not make two spellings of one key differ.
        let c = EntryKey::new("F", &["47"])
            .with("locale", "en")
            .with("tenant", "acme");
        let d = EntryKey::new("F", &["47"])
            .with("tenant", "acme")
            .with("locale", "en");
        assert_eq!(c.logical(), d.logical());
    }

    /// The property that replaced `PW5102`.
    ///
    /// Two generations of the same program, the same logical key, and no shared
    /// storage — guaranteed by there being no way to build a `Materializer`
    /// without a generation, rather than by a rule asking the author to write
    /// one.
    #[test]
    fn two_generations_of_one_program_never_share_an_entry() {
        let key = EntryKey::new("F", &["47"]).with("locale", "en");
        let a = Materializer::new(Clock::new(), "build-a");
        let b = Materializer::new(Clock::new(), "build-b");

        a.regenerate(&key, None, || Ok("markup from build a".to_string()));
        assert_eq!(
            b.read(&key),
            Read::Missing,
            "build B is not served A's entry"
        );

        b.regenerate(&key, None, || Ok("markup from build b".to_string()));
        assert_eq!(a.read(&key), Read::Fresh("markup from build a".to_string()));
        assert_eq!(b.read(&key), Read::Fresh("markup from build b".to_string()));

        // The control: the same generation DOES share, or the test above holds
        // for a store that never returns anything.
        let a2 = Materializer::new(Clock::new(), "build-a");
        assert_eq!(
            a2.physical(&key),
            a.physical(&key),
            "one generation, one storage identity"
        );
    }
}

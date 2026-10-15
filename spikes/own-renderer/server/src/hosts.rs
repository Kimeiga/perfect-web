//! **Hosts sharing one database** (ADR-XXXX, track `second-host`).
//!
//! Each host on a PostgreSQL layer tells its own pages of every commit,
//! whichever host made it:
//!
//! - **A commit's outbox rows are numbered in commit order.** Before it takes
//!   its ids, a command's transaction takes the outbox's lock, held to its
//!   end ([`ordered`]): a transaction that inserts later takes its ids later
//!   and commits later, so a row a reader sees means every smaller id is
//!   final, committed or rolled back for good. PostgreSQL's sequences are
//!   not that by themselves: a value "is not reclaimed for re-use if the
//!   calling transaction later aborts", and it is taken at the insert, not
//!   at the commit.
//! - **The outbox is a log, read by each host from its cursor** ([`Tail`]):
//!   the rows after it, in id order, those of another host's commits told
//!   to this host's pages, its own passed by (its committing thread told
//!   them at once). Rows are trimmed by age, never by a reader; a host whose
//!   cursor fell below what was trimmed tells its pages to reload.
//! - **`NOTIFY` only wakes the readers.** PostgreSQL delivers it "until and
//!   unless the transaction is committed", and only to a session listening
//!   then: a host whose connection dropped misses it, and reads the rows
//!   from its cursor anyway, at least every [`WAIT_MS`].
//!
//! The in-memory layers are one host by definition: their data is the
//! process's, and they have none of this.

use crate::data::Outboxed;
use crate::feed_pg::{ident, pg};
use postgres::{Client, NoTls};
use std::sync::Mutex;

/// **How long a reader waits for a notification before it reads anyway**:
/// what a notification lost to a dropped connection costs at most.
pub(crate) const WAIT_MS: u64 = 1000;

/// **How long a commit's rows are kept** for the hosts that have not read
/// them, on the database's clock.
pub(crate) const KEPT: &str = "10 minutes";

/// **How many rows a reader takes at once.**
const BATCH: i64 = 512;

/// **A host's id**: 128 bits from the operating system's random source,
/// made when the host opens a layer and never reused, written with each
/// outbox row its commits write.
pub(crate) fn new_host() -> String {
    format!(
        "h-{}",
        crate::identity::hex(&crate::identity::random_bytes::<16>())
    )
}

/// **Where a schema's outbox is ordered and heard**: its lock's key and its
/// channel, each named by the schema, so that two schemas in one database
/// (the feed's and the store's, or two tests') neither wait on nor wake each
/// other. A channel is an identifier PostgreSQL truncates past 63 bytes, so
/// it is the schema's SHA-256's first 64 bits, not its name.
#[derive(Debug, Clone)]
pub(crate) struct Outbox {
    pub schema: String,
    pub lock: String,
    pub channel: String,
}

impl Outbox {
    pub(crate) fn of(schema: &str) -> Outbox {
        use sha2::Digest;
        let digest = sha2::Sha256::digest(schema.as_bytes());
        Outbox {
            schema: schema.to_string(),
            lock: format!("pw_outbox:{schema}"),
            channel: format!("pw_outbox_{}", crate::identity::hex(&digest[..8])),
        }
    }

    /// The outbox of the schema `c` searches first.
    pub(crate) fn current(c: &mut Client) -> Result<Outbox, String> {
        let schema: String = c
            .query_one("SELECT current_schema()::text", &[])
            .map_err(pg)?
            .get(0);
        Ok(Outbox::of(&schema))
    }
}

/// **The outbox's rows, in commit order** (ADR-XXXX): the lock taken in the
/// command's transaction, held to its end, before its rows take their ids;
/// and every host listening woken at the commit, and not before. One
/// statement, run only by a commit that hands the outbox anything.
pub(crate) fn ordered(c: &mut Client, outbox: &Outbox) -> Result<(), postgres::Error> {
    c.execute(
        "SELECT pg_advisory_xact_lock(hashtext($1)), pg_notify($2, '')",
        &[&outbox.lock, &outbox.channel],
    )
    .map(|_| ())
}

/// **What a reader heard since its cursor**: each commit of another host,
/// in commit order, with the last id it read; and whether rows it had not
/// read were trimmed first.
#[derive(Debug, Default)]
pub(crate) struct Heard {
    pub commits: Vec<(i64, Outboxed)>,
    /// Rows past the cursor were trimmed before it read them: what they
    /// said can no longer be told.
    pub missed: bool,
    /// The last id read, its own rows' among them.
    pub through: i64,
    /// Whether it read as many rows as it takes at once, so more may wait.
    pub more: bool,
}

/// **A host's reader of the outbox** (ADR-XXXX): its cursor, the last id it
/// has delivered or passed by, and a connection of its own that listens on
/// the schema's channel and reads the rows after the cursor.
pub(crate) struct Tail {
    url: String,
    outbox: Outbox,
    host: String,
    cursor: Mutex<i64>,
    /// The listening connection, made again when it drops.
    conn: Mutex<Option<Client>>,
    /// When it last trimmed.
    trimmed: Mutex<Option<std::time::Instant>>,
}

impl Tail {
    /// **A reader opened with its layer**: its cursor at the outbox's
    /// highest id. The host has no page yet, and every page it serves reads
    /// the database as it is then.
    pub(crate) fn open(url: &str, outbox: Outbox, host: &str) -> Result<Tail, String> {
        let tail = Tail {
            url: url.to_string(),
            outbox,
            host: host.to_string(),
            cursor: Mutex::new(0),
            conn: Mutex::new(None),
            trimmed: Mutex::new(None),
        };
        let highest = tail.with(|c| {
            c.query_one("SELECT coalesce(max(id), 0)::bigint FROM outbox", &[])
                .map(|r| r.get::<_, i64>(0))
        })?;
        *tail.cursor.lock().expect("cursor") = highest;
        Ok(tail)
    }

    /// This host's id, which its own rows carry.
    pub(crate) fn host(&self) -> &str {
        &self.host
    }

    /// The schema's outbox it reads.
    pub(crate) fn outbox(&self) -> &Outbox {
        &self.outbox
    }

    /// The last id delivered or passed by.
    pub(crate) fn cursor(&self) -> i64 {
        *self.cursor.lock().expect("cursor")
    }

    /// **Its connection**, listening on the schema's channel, made again
    /// when it dropped. A connection made again listens first and reads
    /// after (PostgreSQL's advice for `LISTEN`), so nothing committed
    /// between the two is lost.
    fn with<T>(
        &self,
        f: impl FnOnce(&mut Client) -> Result<T, postgres::Error>,
    ) -> Result<T, String> {
        let mut conn = self.conn.lock().expect("the tail's connection");
        if conn.as_ref().is_none_or(Client::is_closed) {
            let mut c = Client::connect(&self.url, NoTls).map_err(pg)?;
            c.batch_execute(&format!(
                "SET search_path TO {}; LISTEN {}",
                ident(&self.outbox.schema),
                ident(&self.outbox.channel)
            ))
            .map_err(pg)?;
            *conn = Some(c);
        }
        let c = conn.as_mut().expect("connected");
        let out = f(c);
        if out.is_err() && c.is_closed() {
            *conn = None;
        }
        out.map_err(pg)
    }

    /// **The rows after its cursor**: another host's commits, each one by
    /// its transaction, in commit order; its own passed by. It moves
    /// nothing: the host moves the cursor once it told each
    /// ([`Tail::passed`]).
    pub(crate) fn heard(&self) -> Result<Heard, String> {
        let cursor = self.cursor();
        let (trimmed, rows) = self.with(|c| {
            let trimmed: i64 = c
                .query_one("SELECT through FROM outbox_trimmed", &[])?
                .get(0);
            let rows = c.query(
                "SELECT id, host, committed_in::text, kind, name, args::text FROM outbox \
                 WHERE id > $1 ORDER BY id LIMIT $2",
                &[&cursor.max(trimmed), &BATCH],
            )?;
            Ok((trimmed, rows))
        })?;
        let mut heard = Heard {
            missed: trimmed > cursor,
            through: cursor.max(trimmed),
            more: rows.len() as i64 >= BATCH,
            ..Heard::default()
        };
        // A commit's rows are consecutive ids (its lock), and share its
        // transaction.
        let mut commit: Option<(String, i64, Vec<(i64, String, String, String)>)> = None;
        let done = |commit: Option<(String, i64, Vec<(i64, String, String, String)>)>,
                    heard: &mut Heard|
         -> Result<(), String> {
            if let Some((_, last, read)) = commit {
                heard
                    .commits
                    .push((last, crate::feed_pg::outboxed_of(read)?));
            }
            Ok(())
        };
        for row in &rows {
            let (id, host, txn): (i64, String, String) = (row.get(0), row.get(1), row.get(2));
            heard.through = id;
            if host == self.host {
                continue;
            }
            let same = commit.as_ref().is_some_and(|(t, _, _)| *t == txn);
            if !same {
                done(commit.take(), &mut heard)?;
                commit = Some((txn.clone(), id, Vec::new()));
            }
            let (_, last, read) = commit.as_mut().expect("a commit");
            *last = id;
            read.push((id, row.get(3), row.get(4), row.get(5)));
        }
        done(commit, &mut heard)?;
        Ok(heard)
    }

    /// **Its cursor moved past `id`**, once what it read through `id` was
    /// told. Never back.
    pub(crate) fn passed(&self, id: i64) {
        let mut cursor = self.cursor.lock().expect("cursor");
        *cursor = (*cursor).max(id);
    }

    /// **Wait for a commit**: a notification on the schema's channel, or
    /// `ms` without one. A connection that dropped is made again by the next
    /// read, which reads from the cursor: what was said meanwhile is lost,
    /// what was committed is not.
    pub(crate) fn wait(&self, ms: u64) {
        let waited = self.with(|c| {
            use postgres::fallible_iterator::FallibleIterator;
            let first = c
                .notifications()
                .timeout_iter(std::time::Duration::from_millis(ms))
                .next()?;
            // And whatever else is already waiting: one read answers all.
            if first.is_some() {
                let mut waiting = c.notifications();
                let mut waiting = waiting.iter();
                while waiting.next()?.is_some() {}
            }
            Ok(())
        });
        if let Err(why) = waited {
            eprintln!("pw dev server: the outbox's listener: {why}");
            std::thread::sleep(std::time::Duration::from_millis(ms));
        }
    }

    /// **Rows older than [`KEPT`] trimmed**, at most once a minute, and the
    /// highest id trimmed recorded, which a reader below it learns it missed.
    pub(crate) fn trim(&self) {
        {
            let mut trimmed = self.trimmed.lock().expect("trimmed");
            if trimmed.is_some_and(|t| t.elapsed() < std::time::Duration::from_secs(60)) {
                return;
            }
            *trimmed = Some(std::time::Instant::now());
        }
        if let Err(why) = self.trim_older_than(KEPT) {
            eprintln!("pw dev server: the outbox was not trimmed: {why}");
        }
    }

    /// [`Tail::trim`], now, of rows older than `age` (an interval's text).
    pub(crate) fn trim_older_than(&self, age: &str) -> Result<(), String> {
        self.with(|c| {
            c.execute(
                "WITH gone AS (DELETE FROM outbox WHERE created < clock_timestamp() - $1::text::interval \
                 RETURNING id) \
                 UPDATE outbox_trimmed SET through = greatest(through, (SELECT max(id) FROM gone))",
                &[&age],
            )
            .map(|_| ())
        })
    }
}

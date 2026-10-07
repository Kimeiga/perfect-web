//! **The feed's data in PostgreSQL** (ADR-0246), beside the in-memory layer
//! (`feed.rs`), which stays the default. A deployment names a database with
//! `PW_FEED_DATABASE_URL`; the host opens this layer on it, holds it to what
//! the feed's `source` states, and serves the same program.
//!
//! - **The schema is plain SQL** under `migrations/feed/`, applied once each
//!   and recorded in `pw_migrations`.
//! - **A command is one transaction**, opened at [`DataLayer::begin`] and
//!   serializable whatever the connection's default. Its writes, and what it
//!   handed the outbox, commit together or not at all (ADR-0208).
//! - **What is delivered is read back from the outbox** once the transaction
//!   committed, and the rows read are consumed. The host tells open sessions
//!   from them exactly as it does from the in-memory layer's events.
//!
//! [`DataLayer::begin`]: crate::data::DataLayer::begin

use super::*;
use crate::data::{Dropped, Handed, Outboxed, Provided};
use crate::feed::{PROFILE_POSTS, not_found, ok, user_of, user_record};
use postgres::{Client, NoTls};
use std::collections::BTreeSet;
use std::sync::atomic::{AtomicBool, Ordering};

/// The migrations, in the order they apply, by the name each is recorded by.
const MIGRATIONS: &[(&str, &str)] = &[
    (
        "0001_schema",
        include_str!("../migrations/feed/0001_schema.sql"),
    ),
    (
        "0002_seed",
        include_str!("../migrations/feed/0002_seed.sql"),
    ),
    (
        "0004_follows",
        include_str!("../migrations/feed/0004_follows.sql"),
    ),
];

/// **How a command's transaction is opened.** The layer sets serializable on
/// each; a deployment that leaves it to the connection gets the database's
/// `default_transaction_isolation`, read committed unless configured, and
/// the host refuses to serve a source that states more.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Isolation {
    Serializable,
    ConnectionDefault,
}

/// A database error, as one line: its SQLSTATE and message where the server
/// sent one.
fn pg(e: postgres::Error) -> String {
    match e.as_db_error() {
        Some(db) => format!("PostgreSQL {}: {}", db.code().code(), db.message()),
        None => format!("PostgreSQL: {e}"),
    }
}

/// An identifier, quoted.
fn ident(name: &str) -> String {
    format!("\"{}\"", name.replace('"', "\"\""))
}

/// **Connections, kept open between uses.** Each is in the layer's schema.
struct Pool {
    url: String,
    schema: Option<String>,
    idle: Mutex<Vec<Client>>,
}

impl Pool {
    fn connect(&self) -> Result<Client, String> {
        let mut c = Client::connect(&self.url, NoTls).map_err(pg)?;
        if let Some(schema) = &self.schema {
            c.batch_execute(&format!("SET search_path TO {}", ident(schema)))
                .map_err(pg)?;
        }
        Ok(c)
    }

    fn take(&self) -> Result<Client, String> {
        match self.idle.lock().expect("idle connections").pop() {
            Some(c) if !c.is_closed() => Ok(c),
            _ => self.connect(),
        }
    }

    fn give(&self, c: Client) {
        if !c.is_closed() {
            self.idle.lock().expect("idle connections").push(c);
        }
    }

    /// One read outside any command: a statement of its own, which reads the
    /// latest commit.
    fn with<T>(
        &self,
        f: impl FnOnce(&mut Client) -> Result<T, postgres::Error>,
    ) -> Result<T, String> {
        let mut c = self.take()?;
        let out = f(&mut c).map_err(pg);
        self.give(c);
        out
    }
}

/// **Open a command's transaction** on `c`, as `isolation` says: the one
/// place a command's transaction begins, which [`FeedPg::provides`] measures.
fn begin_command(c: &mut Client, isolation: Isolation) -> Result<(), postgres::Error> {
    match isolation {
        Isolation::Serializable => c.batch_execute("BEGIN ISOLATION LEVEL SERIALIZABLE"),
        Isolation::ConnectionDefault => c.batch_execute("BEGIN"),
    }
}

/// One post as read: its author's handle and name where it has a row, its
/// likes counted, and what it replies to.
struct Row {
    id: String,
    author: String,
    known: Option<(String, String)>,
    text: String,
    likes: i64,
    reply_to: Option<String>,
}

/// The columns every read of a post selects, `p` the post.
const POST_COLUMNS: &str = "p.id, p.author, u.handle, u.name, p.text, \
     (SELECT count(*) FROM likes l WHERE l.post = p.id), p.reply_to";

fn row_of(r: &postgres::Row) -> Row {
    let handle: Option<String> = r.get(2);
    let name: Option<String> = r.get(3);
    Row {
        id: r.get(0),
        author: r.get(1),
        known: handle.zip(name),
        text: r.get(4),
        likes: r.get(5),
        reply_to: r.get(6),
    }
}

/// A post as the program's `Post` is, `replies` inside it.
fn post_val(row: &Row, replies: Vec<Val>) -> Val {
    Val::Record(vec![
        ("id".into(), Val::String(row.id.clone())),
        ("author".into(), user_record(&row.author, row.known.clone())),
        ("text".into(), Val::String(row.text.clone())),
        ("likes".into(), Val::S64(row.likes)),
        ("replies".into(), Val::List(replies)),
    ])
}

/// **The timeline**: the newest `limit` posts that reply to none, each an
/// `Item` (ADR-0222).
fn timeline(c: &mut Client, limit: i64) -> Result<Val, postgres::Error> {
    let rows = c.query(
        &format!(
            "SELECT {POST_COLUMNS} FROM posts p LEFT JOIN users u ON u.id = p.author \
             WHERE p.reply_to IS NULL ORDER BY p.seq DESC LIMIT $1"
        ),
        &[&limit.max(0)],
    )?;
    Ok(Val::List(rows.iter().map(|r| item_of(row_of(r))).collect()))
}

/// An `Item` as the timelines show it (ADR-0222).
fn item_of(row: Row) -> Val {
    Val::Record(vec![
        ("id".into(), Val::String(row.id)),
        ("author".into(), user_record(&row.author, row.known)),
        ("text".into(), Val::String(row.text)),
        ("likes".into(), Val::S64(row.likes)),
    ])
}

/// **The timeline of those `reader` follows** (ADR-0257): its own posts and
/// theirs that reply to none, the newest `limit`.
fn following(c: &mut Client, reader: &str, limit: i64) -> Result<Val, postgres::Error> {
    let rows = c.query(
        &format!(
            "SELECT {POST_COLUMNS} FROM posts p LEFT JOIN users u ON u.id = p.author \
             WHERE p.reply_to IS NULL \
               AND (p.author = $1 \
                    OR p.author IN (SELECT followee FROM follows WHERE follower = $1)) \
             ORDER BY p.seq DESC LIMIT $2"
        ),
        &[&reader, &limit.max(0)],
    )?;
    Ok(Val::List(rows.iter().map(|r| item_of(row_of(r))).collect()))
}

/// Whether the feed knows `id` (ADR-0257), as `feed.rs` reads it.
const KNOWN: &str = "EXISTS (SELECT 1 FROM users WHERE id = $1) \
     OR EXISTS (SELECT 1 FROM posts WHERE author = $1) \
     OR EXISTS (SELECT 1 FROM follows WHERE follower = $1 OR followee = $1)";

/// **A user's page** (ADR-0257), read in one statement, so one snapshot:
/// who they are, their counts, and their newest posts that reply to none.
fn profile(c: &mut Client, id: &str) -> Result<Val, postgres::Error> {
    let row = c.query_one(
        &format!(
            "SELECT u.handle, u.name, \
                    (SELECT count(*) FROM follows WHERE followee = $1), \
                    (SELECT count(*) FROM follows WHERE follower = $1), \
                    (SELECT coalesce(json_agg(json_build_array(p.id, p.text, \
                         (SELECT count(*) FROM likes l WHERE l.post = p.id)) \
                         ORDER BY p.seq DESC), '[]')::text \
                       FROM (SELECT * FROM posts WHERE author = $1 AND reply_to IS NULL \
                             ORDER BY seq DESC LIMIT {PROFILE_POSTS}) p), \
                    {KNOWN} \
             FROM (SELECT 1) AS one LEFT JOIN users u ON u.id = $1"
        ),
        &[&id],
    )?;
    let known: bool = row.get(5);
    if !known {
        return Ok(not_found());
    }
    let (handle, name): (Option<String>, Option<String>) = (row.get(0), row.get(1));
    let (followers, following): (i64, i64) = (row.get(2), row.get(3));
    let posts: Vec<(String, String, i64)> =
        serde_json::from_str(&row.get::<_, String>(4)).unwrap_or_default();
    Ok(ok(Val::Record(vec![
        ("user".into(), user_record(id, handle.zip(name))),
        ("followers".into(), Val::S64(followers)),
        ("following".into(), Val::S64(following)),
        (
            "posts".into(),
            Val::List(
                posts
                    .into_iter()
                    .map(|(id, text, likes)| {
                        Val::Record(vec![
                            ("id".into(), Val::String(id)),
                            ("text".into(), Val::String(text)),
                            ("likes".into(), Val::S64(likes)),
                        ])
                    })
                    .collect(),
            ),
        ),
    ])))
}

/// **What `reader` is to `user`** (ADR-0257), the program's `Relation`.
fn relation(c: &mut Client, reader: &str, user: &str) -> Result<Val, postgres::Error> {
    if reader == user {
        return Ok(Val::Variant("yourself".into(), None));
    }
    let follows: bool = c
        .query_one(
            "SELECT EXISTS (SELECT 1 FROM follows WHERE follower = $1 AND followee = $2)",
            &[&reader, &user],
        )?
        .get(0);
    let case = if follows {
        "following"
    } else {
        "not-following"
    };
    Ok(Val::Variant(case.into(), None))
}

/// **A thread**: the post and its replies, as deep as they go, read in one
/// statement, so one snapshot.
fn thread(c: &mut Client, id: &str) -> Result<Val, postgres::Error> {
    let rows = c.query(
        &format!(
            "WITH RECURSIVE t AS ( \
                 SELECT * FROM posts WHERE id = $1 \
                 UNION ALL \
                 SELECT p.* FROM posts p JOIN t ON p.reply_to = t.id) \
             SELECT {POST_COLUMNS} FROM t p LEFT JOIN users u ON u.id = p.author \
             ORDER BY p.seq"
        ),
        &[&id],
    )?;
    let rows: Vec<Row> = rows.iter().map(row_of).collect();
    // Each post's replies, in the order they were written.
    fn nested(rows: &[Row], row: &Row) -> Val {
        let replies = rows
            .iter()
            .filter(|r| r.reply_to.as_deref() == Some(row.id.as_str()))
            .map(|r| nested(rows, r))
            .collect();
        post_val(row, replies)
    }
    Ok(match rows.iter().find(|r| r.id == id) {
        Some(root) => ok(nested(&rows, root)),
        None => not_found(),
    })
}

/// One post with its likes, its replies left to its thread.
fn post_alone(c: &mut Client, id: &str) -> Result<Option<Val>, postgres::Error> {
    let row = c.query_opt(
        &format!(
            "SELECT {POST_COLUMNS} FROM posts p LEFT JOIN users u ON u.id = p.author \
             WHERE p.id = $1"
        ),
        &[&id],
    )?;
    Ok(row.map(|r| post_val(&row_of(&r), Vec::new())))
}

/// One read, on a connection.
type Read<'r> = &'r mut dyn FnMut(&mut Client) -> Result<Val, String>;

/// Where a read runs: a connection of the pool's, or the command's own
/// transaction.
type Run = Arc<dyn Fn(Read<'_>) -> Result<Val, String> + Send + Sync>;

/// The reads, each through `run`.
fn reads_through(run: Run) -> crate::data::Ops {
    let mut ops: crate::data::Ops = BTreeMap::new();
    let r = run.clone();
    ops.insert(
        "feed:data/posts#timeline".to_string(),
        Arc::new(move |args: &[Val]| match args {
            [Val::String(_), Val::S64(limit)] => {
                let limit = *limit;
                Ok(vec![r(&mut |c| timeline(c, limit).map_err(pg))?])
            }
            other => Err(format!("posts#timeline received {other:?}")),
        }),
    );
    let r = run.clone();
    ops.insert(
        "feed:data/posts#thread".to_string(),
        Arc::new(move |args: &[Val]| match args {
            [Val::String(id)] => Ok(vec![r(&mut |c| thread(c, id).map_err(pg))?]),
            other => Err(format!("posts#thread received {other:?}")),
        }),
    );
    let r = run.clone();
    ops.insert(
        "feed:data/posts#following".to_string(),
        Arc::new(move |args: &[Val]| match args {
            [Val::String(session), Val::S64(limit)] => {
                let (reader, limit) = (user_of(session), *limit);
                Ok(vec![r(&mut |c| following(c, &reader, limit).map_err(pg))?])
            }
            other => Err(format!("posts#following received {other:?}")),
        }),
    );
    let r = run.clone();
    ops.insert(
        "feed:data/users#profile".to_string(),
        Arc::new(move |args: &[Val]| match args {
            [Val::String(id)] => Ok(vec![r(&mut |c| profile(c, id).map_err(pg))?]),
            other => Err(format!("users#profile received {other:?}")),
        }),
    );
    let r = run;
    ops.insert(
        "feed:data/users#relation".to_string(),
        Arc::new(move |args: &[Val]| match args {
            [Val::String(session), Val::String(user)] => {
                let reader = user_of(session);
                Ok(vec![r(&mut |c| relation(c, &reader, user).map_err(pg))?])
            }
            other => Err(format!("users#relation received {other:?}")),
        }),
    );
    ops.insert(
        "feed:data/users#of-session".to_string(),
        Arc::new(move |args: &[Val]| match args {
            [Val::String(session)] => Ok(vec![Val::String(user_of(session))]),
            other => Err(format!("users#of-session received {other:?}")),
        }),
    );
    ops
}

/// **The feed's data in one PostgreSQL database.**
pub(crate) struct FeedPg {
    pool: Arc<Pool>,
    isolation: Isolation,
    /// One command of this host at a time, from its first write to its
    /// commit, as the in-memory layer's lock is: a host never conflicts with
    /// itself. A serializable transaction is what holds against any other
    /// writer of the database, and a commit it refuses keeps nothing.
    writes: Mutex<()>,
}

impl FeedPg {
    /// **The layer on `url`**, its tables in `schema` (created if missing)
    /// or in the connection's own search path, migrated, and a command's
    /// transaction serializable.
    #[cfg(test)]
    pub(crate) fn open(url: &str, schema: Option<&str>) -> Result<FeedPg, String> {
        FeedPg::open_with(url, schema, Isolation::Serializable)
    }

    /// [`FeedPg::open`], a command's transaction opened as `isolation` says.
    pub(crate) fn open_with(
        url: &str,
        schema: Option<&str>,
        isolation: Isolation,
    ) -> Result<FeedPg, String> {
        let pool = Arc::new(Pool {
            url: url.to_string(),
            schema: schema.map(str::to_string),
            idle: Mutex::new(Vec::new()),
        });
        if let Some(schema) = schema {
            let mut c = Client::connect(url, NoTls).map_err(pg)?;
            c.batch_execute(&format!("CREATE SCHEMA IF NOT EXISTS {}", ident(schema)))
                .map_err(pg)?;
        }
        let mut c = pool.connect()?;
        migrate(&mut c)?;
        // What a host that stopped between a commit and its delivery left in
        // the outbox: a host just opened keeps no answer it could make stale,
        // and has no open session to tell.
        c.batch_execute("DELETE FROM outbox").map_err(pg)?;
        pool.give(c);
        Ok(FeedPg {
            pool,
            isolation,
            writes: Mutex::new(()),
        })
    }
}

/// **Apply each migration not yet applied**, in order, in one transaction,
/// under a lock no other host migrating the same database can share.
fn migrate(c: &mut Client) -> Result<(), String> {
    let mut tx = c.transaction().map_err(pg)?;
    tx.batch_execute(
        "SELECT pg_advisory_xact_lock(hashtext('pw_feed_migrations'));
         CREATE TABLE IF NOT EXISTS pw_migrations (
             version    text PRIMARY KEY,
             applied_at timestamptz NOT NULL DEFAULT now()
         );",
    )
    .map_err(pg)?;
    for (version, sql) in MIGRATIONS {
        let applied = tx
            .query_opt("SELECT 1 FROM pw_migrations WHERE version = $1", &[version])
            .map_err(pg)?
            .is_some();
        if applied {
            continue;
        }
        tx.batch_execute(sql)
            .map_err(|e| format!("migration {version}: {}", pg(e)))?;
        tx.execute(
            "INSERT INTO pw_migrations (version) VALUES ($1)",
            &[version],
        )
        .map_err(pg)?;
    }
    tx.commit().map_err(pg)
}

/// **A value the outbox keeps**, as the command computed it (ADR-0208): a
/// `String`, an `Int` or a `Bool`, which key an entry, and an invalidated
/// entry's `_`, every value at its position, as `null` (ADR-0256). Anything
/// else keys none and is refused, as the host's own outbox refuses it.
fn json_of(name: &str, values: &[Option<Val>]) -> Result<String, String> {
    let values = values
        .iter()
        .map(|v| match v {
            None => Ok(serde_json::Value::Null),
            Some(Val::String(s)) => Ok(serde_json::Value::String(s.clone())),
            Some(Val::S64(n)) => Ok(serde_json::Value::from(*n)),
            Some(Val::Bool(b)) => Ok(serde_json::Value::Bool(*b)),
            Some(other) => Err(format!("`{name}` carries {other:?}, which keys no entry")),
        })
        .collect::<Result<Vec<_>, String>>()?;
    Ok(serde_json::Value::Array(values).to_string())
}

/// A value read back from the outbox, as it was written.
fn val_of(json: &serde_json::Value) -> Result<Val, String> {
    match json {
        serde_json::Value::String(s) => Ok(Val::String(s.clone())),
        serde_json::Value::Bool(b) => Ok(Val::Bool(*b)),
        serde_json::Value::Number(n) => n
            .as_i64()
            .map(Val::S64)
            .ok_or_else(|| format!("the outbox kept {n}, which is no Int")),
        other => Err(format!("the outbox kept {other}, which keys no entry")),
    }
}

/// **A command's transaction** (ADR-0218, ADR-0246): its connection, in
/// the transaction `begin` opened, from the call to the commit.
pub(crate) struct Staging<'a> {
    _writes: std::sync::MutexGuard<'a, ()>,
    pool: Arc<Pool>,
    /// The connection, given back to the pool when the staging is dropped.
    conn: Arc<Mutex<Option<Client>>>,
    /// Whether the command wrote anything.
    wrote: Arc<AtomicBool>,
    /// Whether its transaction committed; dropped otherwise, it rolls back.
    committed: bool,
    ops: crate::data::Ops,
}

impl crate::data::Staged for Staging<'_> {
    fn ops(&self) -> crate::data::Ops {
        self.ops.clone()
    }

    fn rows(&self) -> Option<Vec<(String, String)>> {
        // Its rows are the database's: the host's materializer keeps none.
        self.wrote.load(Ordering::SeqCst).then(Vec::new)
    }

    fn publish(&mut self) {
        // Committed in `commit`: there is nothing left to make the layer's.
    }

    fn commit(
        &mut self,
        events: &[Handed],
        invalidated: &[Dropped],
    ) -> Result<Option<Outboxed>, String> {
        let mut conn = self.conn.lock().expect("the command's connection");
        let c = conn
            .as_mut()
            .ok_or("the command's transaction did not open")?;
        // What the command handed the outbox, in its transaction: an
        // invalidated entry's `_` as `null` (ADR-0256).
        let handed = events
            .iter()
            .map(|(name, values)| {
                (
                    "event",
                    name,
                    values.iter().cloned().map(Some).collect::<Vec<_>>(),
                )
            })
            .chain(
                invalidated
                    .iter()
                    .map(|(name, values)| ("invalidation", name, values.clone())),
            );
        let mut ids: Vec<i64> = Vec::new();
        for (kind, name, values) in handed {
            let args = json_of(name, &values)?;
            let row = c
                .query_one(
                    "INSERT INTO outbox (kind, name, args) VALUES ($1, $2, $3::text::jsonb) \
                     RETURNING id",
                    &[&kind, name, &args],
                )
                .map_err(pg)?;
            ids.push(row.get(0));
        }
        // The writes and the outbox's rows, at once or not at all. A commit
        // the database refuses (a serialization failure, a deferred
        // constraint) has rolled back, and nothing is delivered.
        c.batch_execute("COMMIT").map_err(pg)?;
        self.committed = true;
        match delivered(c, &ids) {
            Ok(read) => Ok(Some(read)),
            // Committed, and not read back: what it handed is what
            // committed. Its rows stay, consumed when a host next opens the
            // database; the command is not answered as refused.
            Err(why) => {
                eprintln!("pw dev server: the feed's outbox was not read back: {why}");
                Ok(Some((events.to_vec(), invalidated.to_vec())))
            }
        }
    }
}

/// **What the outbox committed, read back and consumed**: what is delivered
/// is what was committed, and nothing else.
fn delivered(c: &mut Client, ids: &[i64]) -> Result<Outboxed, String> {
    let rows = c
        .query(
            "DELETE FROM outbox WHERE id = ANY($1) RETURNING id, kind, name, args::text",
            &[&ids],
        )
        .map_err(pg)?;
    let mut read: Vec<(i64, String, String, String)> = rows
        .iter()
        .map(|r| (r.get(0), r.get(1), r.get(2), r.get(3)))
        .collect();
    read.sort();
    let (mut emitted, mut dropped) = (Vec::new(), Vec::new());
    for (_, kind, name, args) in read {
        let values: Vec<serde_json::Value> =
            serde_json::from_str(&args).map_err(|e| format!("the outbox's `{name}`: {e}"))?;
        match kind.as_str() {
            "event" => {
                let values = values.iter().map(val_of).collect::<Result<Vec<_>, _>>()?;
                emitted.push((name, values));
            }
            // An invalidated entry's `null` is every value at its position
            // (ADR-0256).
            _ => {
                let values = values
                    .iter()
                    .map(|v| match v {
                        serde_json::Value::Null => Ok(None),
                        v => val_of(v).map(Some),
                    })
                    .collect::<Result<Vec<_>, String>>()?;
                dropped.push((name, values));
            }
        }
    }
    Ok((emitted, dropped))
}

impl Drop for Staging<'_> {
    fn drop(&mut self) {
        let Some(mut c) = self.conn.lock().expect("the command's connection").take() else {
            return;
        };
        // A command that failed, trapped or was refused commits nothing.
        if !self.committed && c.batch_execute("ROLLBACK").is_err() {
            return;
        }
        self.pool.give(c);
    }
}

/// An operation of a command, run in its transaction: `wrote` says whether
/// it wrote.
type InTransaction =
    dyn Fn(&mut Client, &[Val]) -> Result<(Val, bool), String> + Send + Sync + 'static;

impl crate::data::DataLayer for FeedPg {
    fn reads(&self, _session: &str, _stopped: Option<Stopped>) -> crate::data::Ops {
        let pool = self.pool.clone();
        reads_through(Arc::new(move |f: Read<'_>| {
            pool.with(|c| Ok(f(c))).and_then(|r| r)
        }))
    }

    fn begin<'a>(&'a self, _session: &str) -> Box<dyn crate::data::Staged + 'a> {
        let writes = self.writes.lock().unwrap_or_else(|e| e.into_inner());
        // The transaction, open before the command runs: what it reads, it
        // reads in it. A connection or a `BEGIN` that fails leaves none, and
        // every operation of the command fails with why.
        let opened =
            self.pool
                .take()
                .and_then(|mut c| match begin_command(&mut c, self.isolation) {
                    Ok(()) => Ok(c),
                    Err(e) => Err(pg(e)),
                });
        let (conn, why) = match opened {
            Ok(c) => (Some(c), String::new()),
            Err(why) => (None, why),
        };
        let conn = Arc::new(Mutex::new(conn));
        let wrote: Arc<AtomicBool> = Arc::default();
        let run: Run = {
            let (conn, why) = (conn.clone(), why.clone());
            Arc::new(move |f: Read<'_>| {
                match conn.lock().expect("the command's connection").as_mut() {
                    Some(c) => f(c),
                    None => Err(why.clone()),
                }
            })
        };
        let mut ops = reads_through(run);
        let mut write = |key: &str, op: Arc<InTransaction>| {
            let (conn, why, wrote) = (conn.clone(), why.clone(), wrote.clone());
            ops.insert(
                key.to_string(),
                Arc::new(move |args: &[Val]| {
                    let mut conn = conn.lock().expect("the command's connection");
                    let c = conn.as_mut().ok_or_else(|| why.clone())?;
                    let (answer, written) = op(c, args)?;
                    if written {
                        wrote.store(true, Ordering::SeqCst);
                    }
                    Ok(vec![answer])
                }),
            );
        };
        write(
            "feed:data/posts#publish",
            Arc::new(|c: &mut Client, args: &[Val]| match args {
                [Val::String(session), Val::String(text)] => {
                    let row = c
                        .query_one(
                            "INSERT INTO posts (author, text) VALUES ($1, $2) RETURNING id",
                            &[&user_of(session), text],
                        )
                        .map_err(pg)?;
                    let id: String = row.get(0);
                    let post = post_alone(c, &id)
                        .map_err(pg)?
                        .ok_or("a post just written")?;
                    Ok((ok(post), true))
                }
                other => Err(format!("posts#publish received {other:?}")),
            }),
        );
        // **A reply** (ADR-0231): a post that replies to `to`. To a post that
        // is not there, none.
        write(
            "feed:data/posts#reply",
            Arc::new(|c: &mut Client, args: &[Val]| match args {
                [Val::String(session), Val::String(to), Val::String(text)] => {
                    let there = c
                        .query_opt("SELECT 1 FROM posts WHERE id = $1", &[to])
                        .map_err(pg)?
                        .is_some();
                    if !there {
                        return Ok((not_found(), false));
                    }
                    let row = c
                        .query_one(
                            "INSERT INTO posts (author, text, reply_to) VALUES ($1, $2, $3) \
                             RETURNING id",
                            &[&user_of(session), text, to],
                        )
                        .map_err(pg)?;
                    let id: String = row.get(0);
                    let post = post_alone(c, &id)
                        .map_err(pg)?
                        .ok_or("a reply just written")?;
                    Ok((ok(post), true))
                }
                other => Err(format!("posts#reply received {other:?}")),
            }),
        );
        // **A follow** (ADR-0257): once, however often; one the feed does
        // not know, or the follower itself, is not found, and nothing is
        // written. A follow again writes what is there, and commits, so its
        // event refreshes every page that shows it.
        write(
            "feed:data/users#follow",
            Arc::new(|c: &mut Client, args: &[Val]| match args {
                [Val::String(session), Val::String(user)] => {
                    let follower = user_of(session);
                    let known: bool = c
                        .query_one(&format!("SELECT {KNOWN}"), &[user])
                        .map_err(pg)?
                        .get(0);
                    if follower == *user || !known {
                        return Ok((not_found(), false));
                    }
                    c.execute(
                        "INSERT INTO follows (follower, followee) VALUES ($1, $2) \
                         ON CONFLICT DO NOTHING",
                        &[&follower, user],
                    )
                    .map_err(pg)?;
                    Ok((ok(Val::String(user.clone())), true))
                }
                other => Err(format!("users#follow received {other:?}")),
            }),
        );
        // **An unfollow** (ADR-0257): one not followed is no change.
        write(
            "feed:data/users#unfollow",
            Arc::new(|c: &mut Client, args: &[Val]| match args {
                [Val::String(session), Val::String(user)] => {
                    let known: bool = c
                        .query_one(&format!("SELECT {KNOWN}"), &[user])
                        .map_err(pg)?
                        .get(0);
                    if !known {
                        return Ok((not_found(), false));
                    }
                    c.execute(
                        "DELETE FROM follows WHERE follower = $1 AND followee = $2",
                        &[&user_of(session), user],
                    )
                    .map_err(pg)?;
                    Ok((ok(Val::String(user.clone())), true))
                }
                other => Err(format!("users#unfollow received {other:?}")),
            }),
        );
        write(
            "feed:data/posts#like",
            Arc::new(|c: &mut Client, args: &[Val]| match args {
                [Val::String(session), Val::String(id)] => {
                    let there = c
                        .query_opt("SELECT 1 FROM posts WHERE id = $1", &[id])
                        .map_err(pg)?
                        .is_some();
                    if !there {
                        return Ok((not_found(), false));
                    }
                    c.execute(
                        "INSERT INTO likes (post, liker) VALUES ($1, $2)",
                        &[id, &user_of(session)],
                    )
                    .map_err(pg)?;
                    let post = post_alone(c, id).map_err(pg)?.ok_or("a post just liked")?;
                    Ok((ok(post), true))
                }
                other => Err(format!("posts#like received {other:?}")),
            }),
        );
        Box::new(Staging {
            _writes: writes,
            pool: self.pool.clone(),
            conn,
            wrote,
            committed: false,
            ops,
        })
    }

    fn grants(&self) -> Vec<&'static str> {
        crate::feed::GRANTS.to_vec()
    }

    fn default_page(&self) -> Option<&'static str> {
        None
    }

    /// **What the database provides, measured** (ADR-0246): a transaction
    /// opened as a command's is, asked its isolation and whether it may
    /// write.
    /// - Serializable is `serializable`; PostgreSQL's repeatable read is
    ///   snapshot isolation, `snapshot`; read committed is `read_committed`.
    /// - A transaction that may write is on the primary, whose statements
    ///   read the latest commit: `strong`. A hot standby's are read only
    ///   and refuse serializable, so the layer opens on none.
    /// - It delivers no feed of its changes: the program's events are its
    ///   outbox's.
    fn provides(&self) -> Result<Provided, String> {
        let mut c = self.pool.take()?;
        let measured = begin_command(&mut c, self.isolation).and_then(|()| {
            let row = c.query_one(
                "SELECT current_setting('transaction_isolation'), \
                        current_setting('transaction_read_only')",
                &[],
            );
            // Whatever the query did, the transaction ends here.
            c.batch_execute("ROLLBACK")?;
            row
        });
        self.pool.give(c);
        let row = measured.map_err(pg)?;
        let (isolation, read_only): (String, String) = (row.get(0), row.get(1));
        if read_only == "on" {
            return Err(
                "the feed's database opens a command's transaction read only: no write commits"
                    .to_string(),
            );
        }
        let transactions = match isolation.as_str() {
            "serializable" => "serializable",
            "repeatable read" => "snapshot",
            _ => "read_committed",
        };
        Ok(Provided {
            transactions: transactions.to_string(),
            reads: BTreeSet::from(["strong".to_string()]),
            feed: false,
        })
    }
}

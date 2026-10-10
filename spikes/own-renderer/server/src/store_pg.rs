//! **The store's data in PostgreSQL** (track `store-pg`), beside the
//! in-memory layer (`store.rs`), which stays the default. A deployment names
//! a database with `PW_STORE_DATABASE_URL`; the host opens this layer on it,
//! holds it to what the store's `source` states, and serves the same
//! program. The feed's layer on PostgreSQL (ADR-0246) is its model.
//!
//! - **Its own schema** (`pw_store`, or a test's), which its connections
//!   search alone: its tables, its `outbox`, its migration table
//!   (`pw_store_migrations`) and its advisory lock are its own, and no name
//!   it writes can reach the feed's.
//! - **A command is one transaction**, opened at [`DataLayer::begin`] and
//!   serializable whatever the connection's default. Its writes, and what it
//!   handed the outbox, commit together or not at all (ADR-0208).
//! - **What is delivered is read back from the outbox** once the
//!   transaction committed, and consumed. The host records it in its
//!   materializer as the in-memory layer's commit is recorded, so a
//!   session's cart reaches its open pages as it does in memory.
//! - **Its operations are `store.rs`'s**, over [`Rows`] read and written
//!   here: the two layers cannot answer differently.
//!
//! [`DataLayer::begin`]: crate::data::DataLayer::begin

use super::*;
use crate::data::{Dropped, Handed, Ops, Outboxed, Provided};
use crate::feed_pg::{Isolation, delivered, ident, json_of, pg};
use crate::store::{AddressRow, Estimate, Faults, Item, Order, Rows, StoreRow, With, Written};
use postgres::{Client, NoTls};
use std::collections::BTreeSet;
use std::sync::atomic::Ordering;

/// The migrations, in the order they apply, by the name each is recorded by.
const MIGRATIONS: &[(&str, &str)] = &[
    (
        "0001_schema",
        include_str!("../migrations/store/0001_schema.sql"),
    ),
    (
        "0002_seed",
        include_str!("../migrations/store/0002_seed.sql"),
    ),
    // Track `store-accounts`: a cart and an order are their owner's.
    (
        "0003_owners",
        include_str!("../migrations/store/0003_owners.sql"),
    ),
    // And where a store delivers, and a reader's addresses.
    (
        "0004_addresses",
        include_str!("../migrations/store/0004_addresses.sql"),
    ),
];

/// **The schema a deployment's store is in**, unless it names another.
pub(crate) const SCHEMA: &str = "pw_store";

/// How many connections one layer holds at most: a reader past it waits for
/// one to be given back.
const CONNECTIONS: usize = 8;

/// **Connections, kept open between uses**, each searching the store's
/// schema alone, and no more than [`CONNECTIONS`] at once.
struct Pool {
    url: String,
    schema: String,
    /// The idle connections, and how many are out.
    idle: Mutex<(Vec<Client>, usize)>,
    given_back: std::sync::Condvar,
}

impl Pool {
    fn connect(&self) -> Result<Client, String> {
        let mut c = Client::connect(&self.url, NoTls).map_err(pg)?;
        c.batch_execute(&format!("SET search_path TO {}", ident(&self.schema)))
            .map_err(pg)?;
        Ok(c)
    }

    fn take(self: &Arc<Self>) -> Result<Conn, String> {
        let mut idle = self.idle.lock().expect("idle connections");
        loop {
            while let Some(c) = idle.0.pop() {
                if !c.is_closed() {
                    idle.1 += 1;
                    return Ok(Conn::of(self, c));
                }
            }
            if idle.1 < CONNECTIONS {
                idle.1 += 1;
                drop(idle);
                return match self.connect() {
                    Ok(c) => Ok(Conn::of(self, c)),
                    Err(why) => {
                        self.gone();
                        Err(why)
                    }
                };
            }
            idle = self.given_back.wait(idle).expect("idle connections");
        }
    }

    /// A connection given back, or `None` for one that is not kept.
    fn give(&self, c: Option<Client>) {
        let mut idle = self.idle.lock().expect("idle connections");
        idle.1 -= 1;
        if let Some(c) = c.filter(|c| !c.is_closed()) {
            idle.0.push(c);
        }
        self.given_back.notify_one();
    }

    fn gone(&self) {
        self.give(None);
    }
}

/// **A connection taken from the pool**, given back when dropped; one that
/// is [`Conn::discard`]ed is closed instead.
struct Conn {
    pool: Arc<Pool>,
    client: Option<Client>,
}

impl Conn {
    fn of(pool: &Arc<Pool>, c: Client) -> Conn {
        Conn {
            pool: pool.clone(),
            client: Some(c),
        }
    }

    fn client(&mut self) -> &mut Client {
        self.client.as_mut().expect("a connection taken")
    }

    /// Closed, not given back: a transaction it could not end.
    fn discard(mut self) {
        self.client = None;
    }
}

impl Drop for Conn {
    fn drop(&mut self) {
        self.pool.give(self.client.take());
    }
}

/// **Open a command's transaction** on `c`, as `isolation` says: the one
/// place a command's transaction begins, which [`StorePg::provides`]
/// measures.
fn begin_command(c: &mut Client, isolation: Isolation) -> Result<(), postgres::Error> {
    match isolation {
        Isolation::Serializable => c.batch_execute("BEGIN ISOLATION LEVEL SERIALIZABLE"),
        Isolation::ConnectionDefault => c.batch_execute("BEGIN"),
    }
}

/// **Apply each migration not yet applied**, in order, in one transaction,
/// under a lock no other host migrating the store can share. The table that
/// records them, and the lock, are the store's, never the feed's.
fn migrate(c: &mut Client) -> Result<(), String> {
    let mut tx = c.transaction().map_err(pg)?;
    tx.batch_execute(
        "SELECT pg_advisory_xact_lock(hashtext('pw_store_migrations'));
         CREATE TABLE IF NOT EXISTS pw_store_migrations (
             version    text PRIMARY KEY,
             applied_at timestamptz NOT NULL DEFAULT now()
         );",
    )
    .map_err(pg)?;
    for (version, sql) in MIGRATIONS {
        let applied = tx
            .query_opt(
                "SELECT 1 FROM pw_store_migrations WHERE version = $1",
                &[version],
            )
            .map_err(pg)?
            .is_some();
        if applied {
            continue;
        }
        tx.batch_execute(sql)
            .map_err(|e| format!("migration {version}: {}", pg(e)))?;
        tx.execute(
            "INSERT INTO pw_store_migrations (version) VALUES ($1)",
            &[version],
        )
        .map_err(pg)?;
    }
    tx.commit().map_err(pg)
}

/// **The store's rows, on a connection**, in whatever transaction it is in.
struct PgRows<'c>(&'c mut Client);

fn line_of(r: &postgres::Row) -> Line {
    Line {
        item: r.get(0),
        name: r.get(1),
        quantity: r.get(2),
        price: r.get(3),
    }
}

impl Rows for PgRows<'_> {
    fn stores(&mut self) -> Result<Vec<StoreRow>, String> {
        let rows = self
            .0
            .query(
                "SELECT id, name, description, opens_minute, closes_minute, \
                        lat_e6, lon_e6, radius_m \
                 FROM stores ORDER BY position",
                &[],
            )
            .map_err(pg)?;
        Ok(rows
            .iter()
            .map(|r| StoreRow {
                id: r.get(0),
                name: r.get(1),
                description: r.get(2),
                opens_minute: r.get(3),
                closes_minute: r.get(4),
                zone: crate::places::Zone {
                    lat_e6: r.get(5),
                    lon_e6: r.get(6),
                    radius_m: r.get(7),
                },
            })
            .collect())
    }

    fn menu(&mut self, store: &str) -> Result<Vec<Item>, String> {
        let rows = self
            .0
            .query(
                "SELECT i.id, i.name, i.description, i.price, i.category, c.name, i.served, \
                        i.available \
                 FROM menu_items i JOIN categories c ON c.store = i.store AND c.id = i.category \
                 WHERE i.store = $1 ORDER BY i.position",
                &[&store],
            )
            .map_err(pg)?;
        Ok(rows
            .iter()
            .map(|r| Item {
                id: r.get(0),
                name: r.get(1),
                description: r.get(2),
                price: r.get(3),
                category: r.get(4),
                category_name: r.get(5),
                served: r.get(6),
                available: r.get(7),
            })
            .collect())
    }

    fn set_menu(&mut self, store: &str, items: &[Item]) -> Result<(), String> {
        self.0
            .execute("DELETE FROM menu_items WHERE store = $1", &[&store])
            .map_err(pg)?;
        for (position, item) in items.iter().enumerate() {
            self.0
                .execute(
                    "INSERT INTO categories (store, id, name) VALUES ($1, $2, $3) \
                     ON CONFLICT DO NOTHING",
                    &[&store, &item.category, &item.category_name],
                )
                .map_err(pg)?;
            self.0
                .execute(
                    "INSERT INTO menu_items (store, id, position, name, description, price, \
                         category, served, available) \
                     VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)",
                    &[
                        &store,
                        &item.id,
                        &(position as i32),
                        &item.name,
                        &item.description,
                        &item.price,
                        &item.category,
                        &item.served,
                        &item.available,
                    ],
                )
                .map_err(pg)?;
        }
        Ok(())
    }

    fn stock(&mut self, item: &str, available: bool) -> Result<(), String> {
        self.0
            .execute(
                "UPDATE menu_items SET available = $2 WHERE id = $1",
                &[&item, &available],
            )
            .map(|_| ())
            .map_err(pg)
    }

    fn catalog_item(&mut self, item: &str) -> Result<Option<(String, i64, bool)>, String> {
        let row = self
            .0
            .query_opt(
                "SELECT i.name, i.price, \
                        (SELECT bool_or(available) FROM menu_items WHERE id = $1) \
                 FROM menu_items i JOIN stores s ON s.id = i.store \
                 WHERE i.id = $1 ORDER BY s.position DESC LIMIT 1",
                &[&item],
            )
            .map_err(pg)?;
        Ok(row.map(|r| (r.get(0), r.get(1), r.get(2))))
    }

    fn cart(&mut self, owner: &str) -> Result<Lines, String> {
        let rows = self
            .0
            .query(
                "SELECT item, name, quantity, price FROM cart_lines \
                 WHERE owner = $1 ORDER BY position",
                &[&owner],
            )
            .map_err(pg)?;
        Ok(rows.iter().map(line_of).collect())
    }

    fn set_cart(&mut self, owner: &str, lines: &[Line]) -> Result<(), String> {
        self.0
            .execute("DELETE FROM cart_lines WHERE owner = $1", &[&owner])
            .map_err(pg)?;
        for (position, line) in lines.iter().enumerate() {
            self.0
                .execute(
                    "INSERT INTO cart_lines (owner, position, item, name, quantity, price) \
                     VALUES ($1, $2, $3, $4, $5, $6)",
                    &[
                        &owner,
                        &(position as i32),
                        &line.item,
                        &line.name,
                        &line.quantity,
                        &line.price,
                    ],
                )
                .map_err(pg)?;
        }
        Ok(())
    }

    fn order(&mut self, owner: &str) -> Result<Option<Order>, String> {
        let Some(order) = self
            .0
            .query_opt(
                "SELECT id, status FROM orders WHERE owner = $1 ORDER BY id DESC LIMIT 1",
                &[&owner],
            )
            .map_err(pg)?
        else {
            return Ok(None);
        };
        let id: i64 = order.get(0);
        let lines = self
            .0
            .query(
                "SELECT item, name, quantity, price FROM order_lines \
                 WHERE order_id = $1 ORDER BY position",
                &[&id],
            )
            .map_err(pg)?;
        Ok(Some(Order {
            status: order.get(1),
            lines: lines.iter().map(line_of).collect(),
        }))
    }

    fn place(&mut self, owner: &str, lines: &[Line]) -> Result<(), String> {
        let id: i64 = self
            .0
            .query_one(
                "INSERT INTO orders (owner, status) VALUES ($1, 'placed') RETURNING id",
                &[&owner],
            )
            .map_err(pg)?
            .get(0);
        for (position, line) in lines.iter().enumerate() {
            self.0
                .execute(
                    "INSERT INTO order_lines (order_id, position, item, name, quantity, price) \
                     VALUES ($1, $2, $3, $4, $5, $6)",
                    &[
                        &id,
                        &(position as i32),
                        &line.item,
                        &line.name,
                        &line.quantity,
                        &line.price,
                    ],
                )
                .map_err(pg)?;
        }
        Ok(())
    }

    fn set_status(&mut self, owner: &str, status: Option<&str>) -> Result<(), String> {
        let Some(status) = status else {
            return self
                .0
                .execute("DELETE FROM orders WHERE owner = $1", &[&owner])
                .map(|_| ())
                .map_err(pg);
        };
        let moved = self
            .0
            .execute(
                "UPDATE orders SET status = $2, committed_in = pg_current_xact_id() \
                 WHERE id = (SELECT max(id) FROM orders WHERE owner = $1)",
                &[&owner, &status],
            )
            .map_err(pg)?;
        if moved == 0 {
            self.0
                .execute(
                    "INSERT INTO orders (owner, status) VALUES ($1, $2)",
                    &[&owner, &status],
                )
                .map_err(pg)?;
        }
        Ok(())
    }

    fn addresses(&mut self, owner: &str) -> Result<Vec<AddressRow>, String> {
        let rows = self
            .0
            .query(
                "SELECT id, label, place, chosen FROM addresses \
                 WHERE owner = $1 ORDER BY position",
                &[&owner],
            )
            .map_err(pg)?;
        Ok(rows
            .iter()
            .map(|r| AddressRow {
                id: r.get(0),
                label: r.get(1),
                place: r.get(2),
                chosen: r.get(3),
            })
            .collect())
    }

    fn set_addresses(&mut self, owner: &str, rows: &[AddressRow]) -> Result<(), String> {
        self.0
            .execute("DELETE FROM addresses WHERE owner = $1", &[&owner])
            .map_err(pg)?;
        for (position, a) in rows.iter().enumerate() {
            self.0
                .execute(
                    "INSERT INTO addresses (owner, id, position, label, place, chosen) \
                     VALUES ($1, $2, $3, $4, $5, $6)",
                    &[
                        &owner,
                        &a.id,
                        &(position as i32),
                        &a.label,
                        &a.place,
                        &a.chosen,
                    ],
                )
                .map_err(pg)?;
        }
        Ok(())
    }

    fn notice(&mut self, store: &str) -> Result<Option<String>, String> {
        let row = self
            .0
            .query_opt("SELECT text FROM notices WHERE store = $1", &[&store])
            .map_err(pg)?;
        Ok(row.map(|r| r.get(0)))
    }

    fn set_notice(&mut self, store: &str, text: &str) -> Result<(), String> {
        self.0
            .execute(
                "INSERT INTO notices (store, text) VALUES ($1, $2) \
                 ON CONFLICT (store) DO UPDATE SET text = EXCLUDED.text",
                &[&store, &text],
            )
            .map(|_| ())
            .map_err(pg)
    }

    fn prep(&mut self, store: &str) -> Result<Option<i64>, String> {
        let row = self
            .0
            .query_opt(
                "SELECT prep_minutes FROM kitchens WHERE store = $1",
                &[&store],
            )
            .map_err(pg)?;
        Ok(row.map(|r| r.get(0)))
    }

    fn set_prep(&mut self, store: &str, minutes: i64) -> Result<(), String> {
        self.0
            .execute(
                "INSERT INTO kitchens (store, prep_minutes) VALUES ($1, $2) \
                 ON CONFLICT (store) DO UPDATE SET prep_minutes = EXCLUDED.prep_minutes",
                &[&store, &minutes],
            )
            .map(|_| ())
            .map_err(pg)
    }

    fn curated(&mut self) -> Result<Option<Vec<(String, String)>>, String> {
        let curated: Option<String> = self
            .0
            .query_one("SELECT curated::text FROM recommender", &[])
            .map_err(pg)?
            .get(0);
        curated
            .map(|json| {
                serde_json::from_str(&json).map_err(|e| format!("the recommender's curation: {e}"))
            })
            .transpose()
    }

    fn curate(&mut self, items: Option<&[(String, String)]>) -> Result<(), String> {
        let json = items.map(|items| serde_json::to_string(items).expect("strings encode"));
        self.0
            .execute("UPDATE recommender SET curated = $1::text::jsonb", &[&json])
            .map(|_| ())
            .map_err(pg)
    }

    fn estimate(&mut self, session: &str) -> Result<Option<Estimate>, String> {
        let row = self
            .0
            .query_opt(
                "SELECT min_minutes, max_minutes FROM estimates WHERE session = $1",
                &[&session],
            )
            .map_err(pg)?;
        Ok(row.map(|r| (r.get(0), r.get(1))))
    }

    fn set_estimate(&mut self, session: &str, (min, max): Estimate) -> Result<(), String> {
        self.0
            .execute(
                "INSERT INTO estimates (session, min_minutes, max_minutes) VALUES ($1, $2, $3) \
                 ON CONFLICT (session) DO UPDATE \
                 SET min_minutes = EXCLUDED.min_minutes, max_minutes = EXCLUDED.max_minutes",
                &[&session, &min, &max],
            )
            .map(|_| ())
            .map_err(pg)
    }
}

/// **The store's data in one PostgreSQL schema.**
pub(crate) struct StorePg {
    pool: Arc<Pool>,
    isolation: Isolation,
    /// One command of this host at a time, from its first write to its
    /// commit, as the in-memory layer's lock is: a host never conflicts with
    /// itself. A serializable transaction is what holds against any other
    /// writer of the database, and a commit it refuses keeps nothing.
    writes: Mutex<()>,
    faults: Faults,
    /// A test's schema, dropped with the layer.
    #[cfg(test)]
    owned: bool,
}

impl StorePg {
    /// **The layer on `url`**, its tables in `schema` (created if missing),
    /// which its connections search alone, migrated, and a command's
    /// transaction opened as `isolation` says.
    pub(crate) fn open_with(
        url: &str,
        schema: &str,
        isolation: Isolation,
    ) -> Result<StorePg, String> {
        let mut admin = Client::connect(url, NoTls).map_err(pg)?;
        admin
            .batch_execute(&format!("CREATE SCHEMA IF NOT EXISTS {}", ident(schema)))
            .map_err(pg)?;
        let pool = Arc::new(Pool {
            url: url.to_string(),
            schema: schema.to_string(),
            idle: Mutex::new((Vec::new(), 0)),
            given_back: std::sync::Condvar::new(),
        });
        let mut c = pool.take()?;
        migrate(c.client())?;
        // What a host that stopped between a commit and its delivery left in
        // the outbox: a host just opened keeps no answer it could make stale,
        // and has no open session to tell.
        c.client().batch_execute("DELETE FROM outbox").map_err(pg)?;
        drop(c);
        Ok(StorePg {
            pool,
            isolation,
            writes: Mutex::new(()),
            faults: Faults::new(),
            #[cfg(test)]
            owned: false,
        })
    }

    /// **A test's layer**: in a schema of its own, `pw_store_test_`, the
    /// process and a number, dropped with the layer. Never any other.
    #[cfg(test)]
    pub(crate) fn for_test(url: &str) -> Result<StorePg, String> {
        StorePg::for_test_with(url, Isolation::Serializable)
    }

    /// [`StorePg::for_test`], a command's transaction opened as `isolation`
    /// says.
    #[cfg(test)]
    pub(crate) fn for_test_with(url: &str, isolation: Isolation) -> Result<StorePg, String> {
        static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let schema = format!(
            "pw_store_test_{}_{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::SeqCst)
        );
        let mut layer = StorePg::open_with(url, &schema, isolation)?;
        layer.owned = true;
        Ok(layer)
    }

    /// The test's schema's name.
    #[cfg(test)]
    pub(crate) fn schema(&self) -> &str {
        &self.pool.schema
    }

    /// **A connection in the layer's schema**, outside it, to look at what
    /// was committed and to arrange what a test needs there.
    #[cfg(test)]
    pub(crate) fn sql(&self) -> Client {
        self.pool.connect().expect("connected")
    }
}

#[cfg(test)]
impl Drop for StorePg {
    fn drop(&mut self) {
        if !self.owned {
            return;
        }
        // Every connection closed first, so none holds what is dropped.
        self.pool.idle.lock().expect("idle connections").0.clear();
        if let Ok(mut c) = Client::connect(&self.pool.url, NoTls) {
            let _ = c.batch_execute(&format!(
                "SET lock_timeout = '10s'; DROP SCHEMA IF EXISTS {} CASCADE",
                ident(&self.pool.schema)
            ));
        }
    }
}

/// **A command's transaction** (ADR-0218, ADR-0246): its connection, in
/// the transaction `begin` opened, from the call to the commit.
pub(crate) struct Staging<'a> {
    _writes: std::sync::MutexGuard<'a, ()>,
    session: String,
    /// The connection, given back to the pool when the staging is dropped.
    conn: Arc<Mutex<Option<Conn>>>,
    written: Arc<Mutex<Written>>,
    /// Whether its transaction committed; dropped otherwise, it rolls back.
    committed: bool,
    ops: Ops,
}

impl crate::data::Staged for Staging<'_> {
    fn ops(&self) -> Ops {
        self.ops.clone()
    }

    fn rows(&self) -> Option<Vec<(String, String)>> {
        // What the host's materializer records once the database committed:
        // the cart's total, as the in-memory layer's commit writes it.
        self.written.lock().expect("written").rows(&self.session)
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
            .ok_or("the command's transaction did not open")?
            .client();
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
                eprintln!("pw dev server: the store's outbox was not read back: {why}");
                Ok(Some((events.to_vec(), invalidated.to_vec())))
            }
        }
    }
}

impl Drop for Staging<'_> {
    fn drop(&mut self) {
        let Some(mut conn) = self.conn.lock().expect("the command's connection").take() else {
            return;
        };
        // A command that failed, trapped or was refused commits nothing.
        if !self.committed && conn.client().batch_execute("ROLLBACK").is_err() {
            conn.discard();
        }
    }
}

impl crate::data::DataLayer for StorePg {
    fn reads(&self, session: &str, stopped: Option<Stopped>) -> Ops {
        let pool = self.pool.clone();
        let with: With = Arc::new(move |f| {
            let mut conn = pool.take()?;
            let c = conn.client();
            c.batch_execute("BEGIN ISOLATION LEVEL REPEATABLE READ READ ONLY")
                .map_err(pg)?;
            let out = f(&mut PgRows(c));
            if c.batch_execute("COMMIT").is_err() {
                conn.discard();
            }
            out
        });
        crate::store::reads(with, session, &self.faults, stopped)
    }

    fn begin<'a>(&'a self, session: &str) -> Box<dyn crate::data::Staged + 'a> {
        let writes = self.writes.lock().unwrap_or_else(|e| e.into_inner());
        let fail = self.faults.fail_next.swap(false, Ordering::SeqCst);
        // The transaction, open before the command runs: what it reads, it
        // reads in it. A connection or a `BEGIN` that fails leaves none, and
        // every operation of the command fails with why.
        let opened = self.pool.take().and_then(|mut conn| {
            match begin_command(conn.client(), self.isolation) {
                Ok(()) => Ok(conn),
                Err(e) => Err(pg(e)),
            }
        });
        let (conn, why) = match opened {
            Ok(c) => (Some(c), String::new()),
            Err(why) => (None, why),
        };
        let conn = Arc::new(Mutex::new(conn));
        let with: With = {
            let conn = conn.clone();
            Arc::new(
                move |f| match conn.lock().expect("the command's connection").as_mut() {
                    Some(c) => f(&mut PgRows(c.client())),
                    None => Err(why.clone()),
                },
            )
        };
        let written: Arc<Mutex<Written>> = Arc::default();
        let ops = crate::store::writes(with, session, &self.faults, fail, written.clone());
        Box::new(Staging {
            _writes: writes,
            session: session.to_string(),
            conn,
            written,
            committed: false,
            ops,
        })
    }

    fn grants(&self) -> Vec<&'static str> {
        crate::store::grants().to_vec()
    }

    fn default_page(&self) -> Option<&'static str> {
        Some(crate::store::StoreData::default_page())
    }

    fn session_entry(&self) -> bool {
        true
    }

    fn style(&self) -> &'static str {
        crate::STYLE
    }

    /// **What the database provides, measured** (ADR-0246), as the feed's
    /// is: a transaction opened as a command's is, asked its isolation and
    /// whether it may write, and rolled back.
    fn provides(&self) -> Result<Provided, String> {
        let mut conn = self.pool.take()?;
        let c = conn.client();
        let measured = begin_command(c, self.isolation).and_then(|()| {
            let row = c.query_one(
                "SELECT current_setting('transaction_isolation'), \
                        current_setting('transaction_read_only')",
                &[],
            );
            // Whatever the query did, the transaction ends here.
            c.batch_execute("ROLLBACK")?;
            row
        });
        let row = measured.map_err(pg)?;
        drop(conn);
        let (isolation, read_only): (String, String) = (row.get(0), row.get(1));
        if read_only == "on" {
            return Err(
                "the store's database opens a command's transaction read only: no write commits"
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

impl crate::store::StoreLayer for StorePg {
    fn faults(&self) -> &Faults {
        &self.faults
    }

    #[cfg(test)]
    fn line_around(&self, owner: &str, at: usize, quantity: i64) -> Result<(), String> {
        let changed = self
            .sql()
            .execute(
                "UPDATE cart_lines SET quantity = $3 WHERE owner = $1 AND position = $2",
                &[&owner, &(at as i32), &quantity],
            )
            .map_err(pg)?;
        if changed == 0 {
            return Err("no such line".to_string());
        }
        Ok(())
    }
}

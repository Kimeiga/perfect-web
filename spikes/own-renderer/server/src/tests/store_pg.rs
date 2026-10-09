//! **The store on PostgreSQL** (track `store-pg`): the store served by the
//! host, its data in a real database, held to what its `source` states.
//!
//! Each test runs against `PW_STORE_DATABASE_URL`, in a schema of its own
//! (`pw_store_test_…`) that is dropped with its layer, and passes doing
//! nothing without one: `just e14-store-postgres` is what runs them, and says
//! so when it has no database. The rest of the server's suite runs on this
//! layer too there (`PW_STORE_TEST_LAYER=postgres`); these are the tests
//! that need to see the database itself.

use super::*;
use crate::feed_pg::Isolation;
use crate::store_pg::StorePg;

/// The database the tests run against, where the environment names one.
fn database() -> Option<String> {
    match std::env::var("PW_STORE_DATABASE_URL") {
        Ok(url) if !url.is_empty() => Some(url),
        _ => {
            eprintln!("skipped: PW_STORE_DATABASE_URL is not set");
            None
        }
    }
}

/// **The store, built and served on PostgreSQL**, and its layer, to look at
/// what it committed. The server is dropped before the layer, which drops
/// its schema.
struct OnPostgres {
    served: Served,
    layer: Arc<StorePg>,
}

impl std::ops::Deref for OnPostgres {
    type Target = Server;
    fn deref(&self) -> &Server {
        &self.served
    }
}

impl OnPostgres {
    fn count(&self, query: &str) -> i64 {
        self.layer
            .sql()
            .query_one(query, &[])
            .unwrap_or_else(|e| panic!("{query}: {e}"))
            .get(0)
    }
}

/// The canonical store, its `app.pw` changed by `change` and `patches`
/// applied, served on the layer `open` opens, or why the host refused it.
fn served_with(
    change: fn(&str) -> String,
    patches: &[&str],
    open: impl FnOnce() -> Result<StorePg, String>,
) -> Result<OnPostgres, String> {
    let layer = Arc::new(open()?);
    let (dir, out) = built_from_patches_in("examples", change, patches);
    let server = Server::from_build_layers(out.clone(), out, None, Some(layer.clone()))?;
    Ok(OnPostgres {
        served: Served { server, _dir: dir },
        layer,
    })
}

/// The canonical store as it is, on PostgreSQL.
fn served(url: &str) -> OnPostgres {
    served_with(|app| app.to_string(), &[], || StorePg::for_test(url))
        .expect("served on PostgreSQL")
}

/// **Every row the outbox is given is seen**, with the transaction that
/// gave it: what a test compares with the transaction that wrote the state.
/// Installed in the test's schema only.
fn audit_the_outbox(s: &OnPostgres) {
    s.layer
        .sql()
        .batch_execute(
            "CREATE TABLE outbox_seen (name text NOT NULL, committed_in xid8 NOT NULL);
             CREATE FUNCTION outbox_seen() RETURNS trigger LANGUAGE plpgsql AS $$
             BEGIN
                 INSERT INTO outbox_seen VALUES (NEW.name, NEW.committed_in);
                 RETURN NULL;
             END $$;
             CREATE TRIGGER outbox_seen AFTER INSERT ON outbox
                 FOR EACH ROW EXECUTE FUNCTION outbox_seen();",
        )
        .expect("the outbox audited");
}

fn add(s: &Server, session: &str, item: &str, quantity: i64, interaction: &str) -> Answered {
    s.command_answered(ADD, session, &add_shown(item, quantity), Some(interaction))
        .expect("runs")
}

/// **What the database provides, measured** (ADR-0246): a command's
/// transaction is serializable though the connection's default is read
/// committed, a read is of the latest commit, and no change feed. And the
/// store's connections search its own schema alone.
#[test]
fn on_postgres_the_database_provides_what_the_store_states() {
    let Some(url) = database() else { return };
    let layer = StorePg::for_test(&url).expect("opened");
    let mut sql = layer.sql();
    let default: String = sql
        .query_one("SHOW default_transaction_isolation", &[])
        .expect("shown")
        .get(0);
    assert_eq!(
        default, "read committed",
        "the control: the server's default"
    );
    let provided = crate::data::DataLayer::provides(&layer).expect("measured");
    assert_eq!(provided.transactions, "serializable");
    assert_eq!(
        provided.reads,
        std::collections::BTreeSet::from(["strong".to_string()])
    );
    assert!(!provided.feed);
    let path: String = sql
        .query_one("SHOW search_path", &[])
        .expect("shown")
        .get(0);
    assert_eq!(path, layer.schema(), "its schema alone");
}

/// **A cart's change commits with its event, in one transaction**
/// (ADR-0208): the line's row and the outbox's are written by one
/// transaction, and what was delivered was consumed.
#[test]
fn on_postgres_a_cart_change_commits_with_its_event_in_one_transaction() {
    let Some(url) = database() else { return };
    let s = served(&url);
    audit_the_outbox(&s);
    let answered = add(&s, "a", "espresso", 2, "i-1");
    assert!(answered.committed, "{:?}", answered.result);
    let mut sql = s.layer.sql();
    let rows = sql
        .query(
            "SELECT s.name, s.committed_in = c.committed_in \
             FROM outbox_seen s, cart_lines c WHERE c.owner = 'u-a'",
            &[],
        )
        .expect("read");
    let seen: Vec<(String, bool)> = rows.iter().map(|r| (r.get(0), r.get(1))).collect();
    // Its event, and the entry it drops (ADR-0209), each written by the
    // line's own transaction.
    assert_eq!(
        seen,
        [
            // The reader's, session `a`'s guest's (track `store-accounts`).
            ("Events.UserCartChanged".to_string(), true),
            ("store.page.Cart".to_string(), true)
        ],
        "written by the line's own transaction"
    );
    assert_eq!(
        s.count("SELECT count(*) FROM outbox"),
        0,
        "delivered, and consumed"
    );
}

/// **A route's change commits with its event, in one transaction** (track
/// `store-pg`): the kitchen moving an order along is a write the layer
/// stages, and its `OrderChanged` is the same transaction's.
#[test]
fn on_postgres_a_routes_change_commits_with_its_event_in_one_transaction() {
    let Some(url) = database() else { return };
    let s = served(&url);
    assert!(add(&s, "a", "espresso", 1, "i-1").committed);
    let placed = s
        .command_answered(PLACE, "a", &[], Some("i-2"))
        .expect("runs");
    assert!(placed.committed, "{:?}", placed.result);
    audit_the_outbox(&s);
    let answer = posted(&s, "/bench/order?status=preparing", "a", "", "");
    assert!(answer.starts_with("HTTP/1.1 200"), "{answer}");
    let rows = s
        .layer
        .sql()
        .query(
            "SELECT s.name, s.committed_in = o.committed_in \
             FROM outbox_seen s, orders o WHERE o.owner = 'u-a'",
            &[],
        )
        .expect("read");
    let seen: Vec<(String, bool)> = rows.iter().map(|r| (r.get(0), r.get(1))).collect();
    assert_eq!(
        seen,
        [("Events.UserOrderChanged".to_string(), true)],
        "the order's change and its event, one transaction"
    );
    assert_eq!(s.order_of("a").map(|o| o.0).as_deref(), Some("preparing"));
}

/// **A cart changed on PostgreSQL reaches the session's open page** (track
/// `store-pg`): the commit is recorded in the host's materializer, its rows
/// and its events, as the in-memory layer's is, and the session's entry
/// moves, so the page's count is patched.
#[test]
fn on_postgres_a_cart_change_reaches_the_sessions_open_page() {
    let Some(url) = database() else { return };
    let s = served(&url);
    let home = fetched_as(&s, "/", Some("a"))
        .into_iter()
        .map(|(_, c)| c)
        .collect::<String>();
    assert!(visible(&home).contains("Your cart: 0 items"), "{home}");
    let answered = add(&s, "a", "espresso", 2, "i-1");
    assert!(answered.committed, "{:?}", answered.result);
    assert_eq!(
        s.materializer.state("cart:a").as_deref(),
        Some("2"),
        "the commit's rows, recorded"
    );
    let count = s
        .template_of("store.page.HomePage")
        .manifest()
        .into_iter()
        .find(|e| e.value == "cart.line_count")
        .expect("the home page's count")
        .id;
    assert!(
        patch_sets(&s, "a")
            .iter()
            .any(|set| set.patches.iter().any(|p| p.target.part == count
                && matches!(&p.operation, PatchOp::ReplaceText { text } if text == "2"))),
        "the home page's count was not patched"
    );
}

/// **A commit the database refuses keeps neither its state nor its event**
/// (ADR-0208, ADR-0246): a deferred constraint, checked at the commit after
/// the line and its event were written, refuses it. Nothing is in the cart
/// or the outbox, the materializer records nothing, the open page is told
/// nothing, and the next add commits.
#[test]
fn on_postgres_a_refused_commit_commits_neither_its_state_nor_its_event() {
    let Some(url) = database() else { return };
    let s = served(&url);
    audit_the_outbox(&s);
    s.layer
        .sql()
        .batch_execute(
            "CREATE FUNCTION refuse_four() RETURNS trigger LANGUAGE plpgsql AS $$
             BEGIN
                 IF NEW.quantity >= 4 THEN
                     RAISE EXCEPTION 'four is refused at the commit';
                 END IF;
                 RETURN NULL;
             END $$;
             CREATE CONSTRAINT TRIGGER refuse_four AFTER INSERT OR UPDATE ON cart_lines
                 DEFERRABLE INITIALLY DEFERRED
                 FOR EACH ROW EXECUTE FUNCTION refuse_four();",
        )
        .expect("the constraint installed");
    fetched_as(&s, "/", Some("a"));
    let told = patch_sets(&s, "a").len();
    let refused = s
        .command_answered(ADD, "a", &add_shown("espresso", 4), Some("i-1"))
        .expect_err("refused at its commit");
    assert!(
        refused.contains("four is refused at the commit"),
        "{refused}"
    );
    assert!(s.cart_lines("a").is_empty(), "no line");
    assert_eq!(s.count("SELECT count(*) FROM cart_lines"), 0);
    assert_eq!(s.count("SELECT count(*) FROM outbox"), 0, "no event");
    assert_eq!(
        s.count("SELECT count(*) FROM outbox_seen"),
        0,
        "none written"
    );
    assert_eq!(s.materializer.state("cart:a"), None, "nothing recorded");
    assert_eq!(s.materializer.retained_events(), 0);
    assert_eq!(patch_sets(&s, "a").len(), told, "no page told");
    let answered = add(&s, "a", "espresso", 1, "i-2");
    assert!(answered.committed, "{:?}", answered.result);
    assert_eq!(s.cart_lines("a"), [("espresso".to_string(), 1)]);
}

/// **Eight sessions adding five times each commit all forty** (track
/// `store-pg`): one serializable transaction each, one command of the host
/// at a time, and none lost.
#[test]
fn on_postgres_eight_sessions_adding_five_times_each_commit_all_forty() {
    let Some(url) = database() else { return };
    let s = served(&url);
    std::thread::scope(|scope| {
        for session in 0..8 {
            let s = &s;
            scope.spawn(move || {
                for press in 0..5 {
                    let answered = add(
                        s,
                        &format!("s-{session}"),
                        "espresso",
                        1,
                        &format!("i-{session}-{press}"),
                    );
                    assert!(answered.committed, "{:?}", answered.result);
                }
            });
        }
    });
    for session in 0..8 {
        assert_eq!(s.cart_value(&format!("s-{session}")), 5, "s-{session}");
    }
    assert_eq!(
        s.count("SELECT sum(quantity)::bigint FROM cart_lines"),
        40,
        "all forty committed"
    );
    assert_eq!(s.count("SELECT count(*) FROM outbox"), 0);
}

/// **A retried interaction runs its command once** (ADR-0121): the second
/// request is given the first's answer, and the database holds one add and
/// saw one event.
#[test]
fn on_postgres_a_retried_interaction_runs_its_command_once() {
    let Some(url) = database() else { return };
    let s = served(&url);
    audit_the_outbox(&s);
    let request = [shown("espresso"), serde_json::json!(1)];
    let first = s
        .command_answer(ADD, "a", &request, Some("press-1"))
        .expect("a well-formed request");
    let again = s
        .command_answer(ADD, "a", &request, Some("press-1"))
        .expect("a well-formed request");
    assert!(first.committed);
    assert_eq!(again, first, "the first answer");
    assert_eq!(s.cart_lines("a"), [("espresso".to_string(), 1)]);
    assert_eq!(
        s.count("SELECT count(DISTINCT committed_in) FROM outbox_seen"),
        1,
        "one transaction gave the outbox anything"
    );
    assert_eq!(
        s.count("SELECT count(*) FROM outbox_seen WHERE name = 'Events.UserCartChanged'"),
        1,
        "one event"
    );
}

/// **An order keeps its lines on PostgreSQL** (ADR-0193), and every event
/// a commit staged is consumed once delivered: after orders are placed and
/// moved along, neither the database's outbox nor the materializer's holds
/// one.
#[test]
fn on_postgres_an_order_keeps_its_lines_and_the_outboxes_hold_nothing() {
    let Some(url) = database() else { return };
    let s = served(&url);
    for (i, session) in ["a", "b"].iter().enumerate() {
        assert!(add(&s, session, "espresso", 2, &format!("i-{i}-1")).committed);
        assert!(add(&s, session, "cortado", 1, &format!("i-{i}-2")).committed);
        let placed = s
            .command_answered(PLACE, session, &[], Some(&format!("i-{i}-3")))
            .expect("runs");
        assert!(placed.committed, "{:?}", placed.result);
        let answer = posted(&s, "/bench/order?status=delivered", session, "", "");
        assert!(answer.starts_with("HTTP/1.1 200"), "{answer}");
    }
    let rows = s
        .layer
        .sql()
        .query(
            "SELECT o.owner, l.item, l.quantity FROM order_lines l \
             JOIN orders o ON o.id = l.order_id ORDER BY o.owner, l.position",
            &[],
        )
        .expect("read");
    let lines: Vec<(String, String, i64)> = rows
        .iter()
        .map(|r| (r.get(0), r.get(1), r.get(2)))
        .collect();
    let want = |session: &str| {
        [
            (session.to_string(), "espresso".to_string(), 2),
            (session.to_string(), "cortado".to_string(), 1),
        ]
    };
    // Each the reader's, its session's guest's (track `store-accounts`).
    assert_eq!(lines, [want("u-a"), want("u-b")].concat());
    assert_eq!(
        s.count("SELECT count(*) FROM cart_lines"),
        0,
        "carts emptied"
    );
    assert_eq!(s.count("SELECT count(*) FROM outbox"), 0);
    assert_eq!(s.materializer.retained_events(), 0);
}

/// **The two layers answer alike** (track `store-pg`): every read the
/// store's queries make, of the first data and after the same writes, from
/// the in-memory layer and from PostgreSQL. `migrations/store/0002_seed.sql`
/// cannot drift from `store.rs`'s seed unseen.
#[test]
fn on_postgres_the_two_layers_answer_alike() {
    let Some(url) = database() else { return };
    // In memory whatever layer the rest of the suite is on.
    let in_memory = {
        let (dir, out) = built_from_patches_in("examples", |app| app.to_string(), &[]);
        let layer: Arc<dyn crate::store::StoreLayer> = Arc::new(crate::store::StoreData::new());
        Served {
            server: Server::from_build_layers(out.clone(), out, None, Some(layer))
                .expect("served in memory"),
            _dir: dir,
        }
    };
    let on_pg = served(&url);
    let servers: [&Server; 2] = [&in_memory, &on_pg];
    for s in servers {
        // The slow sources' delays are test controls, not data.
        s.store
            .faults()
            .recommender
            .lock()
            .expect("recommender")
            .delay_ms = 0;
        s.store
            .faults()
            .estimators
            .lock()
            .expect("estimators")
            .insert(
                "a".to_string(),
                Estimator {
                    delay_ms: 0,
                    fail: None,
                },
            );
    }
    let s = |v: &str| Val::String(v.to_string());
    let reads: Vec<(&str, Vec<Val>)> = vec![
        ("store:data/stores#list", vec![]),
        ("store:data/stores#get", vec![s("47")]),
        ("store:data/stores#get", vec![s("48")]),
        ("store:data/stores#get", vec![s("49")]),
        ("store:data/menus#for-store", vec![s("47")]),
        ("store:data/menus#for-store", vec![s("48")]),
        ("store:data/menus#for-store", vec![s("49")]),
        ("store:data/menus#sections", vec![s("47")]),
        ("store:data/menus#sections", vec![s("48")]),
        ("store:data/menus#in-category", vec![s("47"), s("hot")]),
        ("store:data/menus#in-category", vec![s("47"), s("cold")]),
        ("store:data/menus#in-category", vec![s("47"), s("all")]),
        ("store:data/notices#current", vec![s("47")]),
        ("store:data/notices#current", vec![s("48")]),
        ("store:data/kitchen#prep-minutes", vec![s("47")]),
        ("store:data/kitchen#prep-minutes", vec![s("48")]),
        ("store:data/recommendations#for-store", vec![s("47")]),
        ("store:data/recommendations#for-store", vec![s("48")]),
        ("store:data/carts#current", vec![s("a")]),
        ("store:data/orders#current", vec![s("a")]),
        ("store:data/orders#placed", vec![s("a")]),
        ("store:data/estimates#current", vec![s("a")]),
    ];
    // Each read's answer, its estimate's `generated-at` (a clock reading)
    // left out.
    let answers = |server: &Server| -> Vec<String> {
        let ops = server.data.reads("a", None);
        reads
            .iter()
            .map(|(op, args)| {
                let answer = ops.get(*op).unwrap_or_else(|| panic!("no {op}"))(args)
                    .unwrap_or_else(|e| panic!("{op}: {e}"));
                let text = format!("{answer:?}");
                match text.find("\"generated-at\"") {
                    Some(at) => text[..at].to_string(),
                    None => text,
                }
            })
            .collect()
    };
    let first: Vec<Vec<String>> = servers.iter().map(|s| answers(s)).collect();
    assert_eq!(first[0], first[1], "the first data");
    // The same writes, through each layer: commands, and the routes'.
    for server in servers {
        assert!(add(server, "a", "espresso", 2, "i-1").committed);
        assert!(add(server, "a", "cold-brew", 1, "i-2").committed);
        server
            .broadcast_menu(MenuOp::Insert {
                id: "flat-white".into(),
                name: "Flat White".into(),
                at: Some("cortado".into()),
                before: false,
            })
            .expect("inserted");
        server
            .broadcast_menu(MenuOp::Rename {
                id: "espresso".into(),
                name: "Espresso Doppio".into(),
            })
            .expect("renamed");
        server
            .broadcast_menu(MenuOp::Stock {
                id: "cortado".into(),
                available: false,
            })
            .expect("sold out");
        server.stock_untold("scone", false);
        server.set_estimate("a", 15, Some(45)).expect("estimated");
        server
            .curate(Some(&[("drip".to_string(), "Drip Coffee".to_string())]))
            .expect("curated");
        server.untold(
            "",
            "store:host/notices#post",
            &[s("47"), s("Closing early")],
        );
        server.untold("", "store:host/kitchen#set-prep", &[s("47"), Val::S64(20)]);
        let placed = server
            .command_answered(PLACE, "a", &[], Some("i-3"))
            .expect("runs");
        assert!(placed.committed, "{:?}", placed.result);
        server.order_set_untold("a", "preparing");
        assert!(add(server, "a", "drip", 3, "i-4").committed);
    }
    let after: Vec<Vec<String>> = servers.iter().map(|s| answers(s)).collect();
    assert_eq!(after[0], after[1], "after the same writes");
    assert_ne!(
        first[0], after[0],
        "the control: the writes changed what is read"
    );
    // And the same pages, to the byte: their headers and documents.
    let pages = |server: &Server| -> Vec<String> {
        ["/", "/stores/47", "/stores/48", "/cart", "/order"]
            .iter()
            .map(|path| {
                fetched_as(server, path, Some("b"))
                    .into_iter()
                    .map(|(_, c)| c)
                    .collect::<String>()
            })
            .collect()
    };
    for server in servers {
        assert!(add(server, "b", "matcha", 1, "j-1").committed);
    }
    let (memory_pages, pg_pages) = (pages(&in_memory), pages(&on_pg));
    for (m, p) in memory_pages.iter().zip(&pg_pages) {
        assert!(m.starts_with("HTTP/1.1 200"), "{m}");
        assert_eq!(m, p, "to the byte");
    }
}

/// **The store's schema is its own** (track `store-pg`): in one database
/// with the feed's, each keeps its own migration table, outbox and lock,
/// and a write of one reaches nothing of the other's.
#[test]
fn on_postgres_the_stores_schema_is_its_own_beside_the_feeds() {
    let Some(url) = database() else { return };
    /// The feed's schema, created here and dropped here.
    struct FeedSchema(String, String);
    impl Drop for FeedSchema {
        fn drop(&mut self) {
            if let Ok(mut c) = postgres::Client::connect(&self.0, postgres::NoTls) {
                let _ = c.batch_execute(&format!("DROP SCHEMA IF EXISTS \"{}\" CASCADE", self.1));
            }
        }
    }
    let feed_schema = FeedSchema(
        url.clone(),
        format!("pw_store_test_{}_beside_feed", std::process::id()),
    );
    let feed =
        crate::feed_pg::FeedPg::open_with(&url, Some(&feed_schema.1), Isolation::Serializable)
            .expect("the feed opened");
    let s = served(&url);
    let tables = |schema: &str| -> Vec<String> {
        s.layer
            .sql()
            .query(
                "SELECT table_name::text FROM information_schema.tables \
                 WHERE table_schema = $1 ORDER BY 1",
                &[&schema],
            )
            .expect("read")
            .iter()
            .map(|r| r.get(0))
            .collect()
    };
    let (store_tables, feed_tables) = (tables(s.layer.schema()), tables(&feed_schema.1));
    assert!(
        store_tables.contains(&"pw_store_migrations".to_string()),
        "{store_tables:?}"
    );
    assert!(
        !store_tables.contains(&"pw_migrations".to_string()),
        "{store_tables:?}"
    );
    assert!(
        feed_tables.contains(&"pw_migrations".to_string()),
        "{feed_tables:?}"
    );
    assert!(
        !feed_tables.contains(&"pw_store_migrations".to_string()),
        "{feed_tables:?}"
    );
    assert!(
        store_tables.contains(&"outbox".to_string()) && feed_tables.contains(&"outbox".to_string())
    );
    // A store's write is the store's: the feed's tables are as they were.
    let feed_posts = |c: &mut postgres::Client| -> i64 {
        c.query_one(
            &format!("SELECT count(*) FROM \"{}\".posts", feed_schema.1),
            &[],
        )
        .expect("read")
        .get(0)
    };
    let before = feed_posts(&mut s.layer.sql());
    assert!(add(&s, "a", "espresso", 1, "i-1").committed);
    assert_eq!(feed_posts(&mut s.layer.sql()), before);
    assert!(s.count("SELECT count(*) FROM cart_lines") == 1);
    // Its connections cannot name the feed's tables unqualified.
    let unqualified = s.layer.sql().query_one("SELECT count(*) FROM posts", &[]);
    assert!(
        unqualified.is_err(),
        "the feed's `posts` resolved from the store's schema"
    );
    drop(feed);
}

/// **A source stating a feed of its changes is refused** (ADR-0246's
/// negative control): the layer delivers none, so the host does not serve,
/// and says which clause.
#[test]
fn on_postgres_a_stated_change_feed_the_layer_cannot_deliver_is_refused() {
    let Some(url) = database() else { return };
    const FEED: &str = "--- a/lib/StoreData.pw\n+++ b/lib/StoreData.pw\n@@ -34,3 +34,3 @@\n     transactions serializable\n     reads        strong\n-    changes      none\n+    changes      feed\n";
    let Err(why) = served_with(|app| app.to_string(), &[FEED], || StorePg::for_test(&url)) else {
        panic!("a change feed the layer cannot deliver was served");
    };
    assert!(why.contains("`StoreData` states `changes feed`"), "{why}");
}

/// **Serializable, stated against a connection that opens read committed,
/// is refused**, unless the layer sets it on each transaction, or the
/// connection's default gives it (ADR-0246). What the host compares is
/// measured, not assumed.
#[test]
fn on_postgres_serializable_against_a_read_committed_default_is_refused() {
    let Some(url) = database() else { return };
    let as_is = |app: &str| app.to_string();
    let Err(why) = served_with(as_is, &[], || {
        StorePg::for_test_with(&url, Isolation::ConnectionDefault)
    }) else {
        panic!("serializable was stated, and read committed served");
    };
    assert!(
        why.contains(
            "`StoreData` states `transactions serializable`, and the database's are read_committed"
        ),
        "{why}"
    );
    // The controls: the same connection's default, made serializable, gives
    // it; and the layer setting it on each transaction gives it.
    let separator = if url.contains('?') { '&' } else { '?' };
    let serializable =
        format!("{url}{separator}options=-c%20default_transaction_isolation%3Dserializable");
    served_with(as_is, &[], || {
        StorePg::for_test_with(&serializable, Isolation::ConnectionDefault)
    })
    .expect("a serializable default gives serializable");
    served_with(as_is, &[], || StorePg::for_test(&url)).expect("serializable on each transaction");
}

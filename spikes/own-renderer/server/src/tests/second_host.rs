//! **Hosts sharing one database** (ADR-XXXX, track `second-host`): two hosts
//! in one process, each its own `Server` on a layer of its own opened on one
//! schema, as two processes on one PostgreSQL are. Each test passes doing
//! nothing without a database: `just e14-second-host` runs them.

use super::sign_in::{REDIRECT, TestProvider, command, exchanged, get, location, set_cookie};
use super::*;
use crate::feed_pg::{FeedPg, Isolation};
use crate::hosts::Tail;
use crate::store_pg::StorePg;

/// The database the feed's tests run against, where the environment names
/// one.
fn database() -> Option<String> {
    match std::env::var("PW_FEED_DATABASE_URL") {
        Ok(url) if !url.is_empty() => Some(url),
        _ => {
            eprintln!("skipped: PW_FEED_DATABASE_URL is not set");
            None
        }
    }
}

/// **A schema of the test's own**, dropped when the test is done with it.
struct Schema {
    url: String,
    name: String,
}

impl Schema {
    fn new(url: &str, test: &str) -> Schema {
        let schema = Schema {
            url: url.to_string(),
            name: format!("pw_hosts_{}_{test}", std::process::id()),
        };
        schema
            .admin()
            .batch_execute(&format!("DROP SCHEMA IF EXISTS {} CASCADE", schema.name))
            .expect("a stale schema dropped");
        schema
    }

    fn admin(&self) -> postgres::Client {
        postgres::Client::connect(&self.url, postgres::NoTls).expect("connected")
    }

    fn sql(&self) -> postgres::Client {
        let mut c = self.admin();
        c.batch_execute(&format!("SET search_path TO {}", self.name))
            .expect("the test's schema");
        c
    }

    fn count(&self, query: &str) -> i64 {
        self.sql().query_one(query, &[]).expect(query).get(0)
    }
}

impl Drop for Schema {
    fn drop(&mut self) {
        if let Ok(mut c) = postgres::Client::connect(&self.url, postgres::NoTls) {
            let _ = c.batch_execute(&format!(
                "SET lock_timeout = '10s'; DROP SCHEMA IF EXISTS {} CASCADE",
                self.name
            ));
        }
    }
}

/// **A host of the feed**, started on the schema as a process is: its own
/// layer, its own id, its own cursor.
fn feed_host(url: &str, schema: &Schema) -> Served {
    let layer = FeedPg::open(url, Some(&schema.name)).expect("the layer");
    let (dir, out) = built_feed_with(|app| app.to_string());
    let server = Server::from_build_with(out.clone(), out, Some(Arc::new(layer))).expect("served");
    Served { server, _dir: dir }
}

fn tail(s: &Server) -> &Tail {
    s.data
        .tail()
        .expect("a layer on a shared database reads the outbox")
}

fn post(s: &Server, session: &str, text: &str, interaction: &str) -> Answered {
    s.command_answered(
        "feed.app.post",
        session,
        &[Val::String(text.into())],
        Some(interaction),
    )
    .expect("runs")
}

/// `session`'s home page, served by `s`: its document.
fn home(s: &Server, session: &str) -> Doc {
    let (html, _, _, _) = s
        .serve_document_settled(session, "feed.app.Home", &Params::new(), &[])
        .expect("served");
    assert!(html.contains("Hello, feed."), "{html}");
    latest(&s.pending.lock().expect("pending"), session)
}

/// Every frame waiting for `doc`, as text.
fn frames(s: &Server, doc: &Doc) -> Vec<String> {
    s.pending
        .lock()
        .expect("pending")
        .get(doc)
        .map(|w| w.frames.iter().map(|(_, f)| format!("{f:?}")).collect())
        .unwrap_or_default()
}

/// **A command through A is told to a page on B** (ADR-XXXX): B's reader
/// finds A's commit in the outbox, after its cursor, and tells B's session
/// that reads what it dropped. Not before B reads it: nothing else carries a
/// commit from one host to the other. Until ADR-XXXX the committing host
/// consumed its rows, and B heard nothing.
#[test]
fn a_post_through_one_host_is_told_to_a_page_on_another() {
    let Some(url) = database() else { return };
    let db = Schema::new(&url, "told");
    let a = feed_host(&url, &db);
    let b = feed_host(&url, &db);
    let theirs = home(&b, "b");
    let before = sets_of(&b, &theirs).len();
    let answered = post(&a, "a", "Posted through A, read on B", "i-1");
    assert!(answered.committed, "{:?}", answered.result);
    assert_eq!(sets_of(&b, &theirs).len(), before, "not before B reads it");
    b.hear();
    let set = format!("{:?}", sets_of(&b, &theirs).pop().expect("a patch set"));
    assert!(set.contains("Posted through A, read on B"), "{set}");
    assert_eq!(
        tail(&b).cursor(),
        db.count("SELECT max(id) FROM outbox"),
        "B's cursor past A's commit"
    );
}

/// **Each commit is told once on each host**: the committing host tells its
/// own pages at once and its reader passes its rows by; another host tells
/// its pages when it reads them, and not again.
#[test]
fn a_commit_is_told_once_on_each_host() {
    let Some(url) = database() else { return };
    let db = Schema::new(&url, "once");
    let a = feed_host(&url, &db);
    let b = feed_host(&url, &db);
    let mine = home(&a, "a2");
    let theirs = home(&b, "b");
    post(&a, "a", "Told once on each host", "i-1");
    a.tell_waiting();
    let told_on_a = frames(&a, &mine).len();
    assert!(told_on_a > 0, "A's own page told at once");
    a.hear();
    assert_eq!(
        frames(&a, &mine).len(),
        told_on_a,
        "A's reader passes A's own rows by"
    );
    b.hear();
    let told_on_b = frames(&b, &theirs).len();
    assert!(told_on_b > 0, "B's page told");
    b.hear();
    assert_eq!(frames(&b, &theirs).len(), told_on_b, "and not again");
}

/// **A host whose listening connection drops catches up from its cursor**:
/// the notification is lost with the connection (PostgreSQL keeps none for
/// a session not listening); the rows are not, and B reads them once its
/// connection is made again.
#[test]
fn a_host_that_misses_its_notification_catches_up_from_the_outbox() {
    let Some(url) = database() else { return };
    let db = Schema::new(&url, "missed");
    let a = feed_host(&url, &db);
    let b = feed_host(&url, &db);
    let theirs = home(&b, "b");
    let pid = tail(&b).backend_pid().expect("B listens");
    let ended: bool = db
        .admin()
        .query_one("SELECT pg_terminate_backend($1)", &[&pid])
        .expect("terminated")
        .get(0);
    assert!(ended, "B's listening connection ended");
    post(&a, "a", "Committed while B was not listening", "i-1");
    // The first read finds the connection gone; a read after it is made
    // again finds the rows.
    for _ in 0..3 {
        b.hear();
        if !sets_of(&b, &theirs).is_empty() {
            break;
        }
    }
    let set = format!("{:?}", sets_of(&b, &theirs).pop().expect("a patch set"));
    assert!(set.contains("Committed while B was not listening"), "{set}");
    assert_ne!(
        tail(&b).backend_pid().expect("B listens again"),
        pid,
        "on a connection made again"
    );
}

/// **A host started after commits starts past them**: its pages read the
/// database as it is; nothing before its start is told again.
#[test]
fn a_host_started_after_commits_starts_past_them() {
    let Some(url) = database() else { return };
    let db = Schema::new(&url, "started");
    let a = feed_host(&url, &db);
    post(&a, "a", "Before B started", "i-1");
    let b = feed_host(&url, &db);
    assert_eq!(
        tail(&b).cursor(),
        db.count("SELECT max(id) FROM outbox"),
        "B starts at the highest id"
    );
    let theirs = home(&b, "b");
    b.hear();
    assert!(
        frames(&b, &theirs).is_empty(),
        "told nothing it already read"
    );
    assert_eq!(
        db.count("SELECT count(*) FROM outbox"),
        1,
        "A's row kept, its post's event: B deleted nothing at its start"
    );
}

/// **A host whose cursor fell below what was trimmed reloads its pages**:
/// what the trimmed rows said can no longer be told, so each document is
/// told to reload, and the host goes on from the trimmed id, never skipping
/// in silence.
#[test]
fn a_host_behind_the_trim_tells_its_pages_to_reload() {
    let Some(url) = database() else { return };
    let db = Schema::new(&url, "trimmed");
    let a = feed_host(&url, &db);
    let b = feed_host(&url, &db);
    let theirs = home(&b, "b");
    post(&a, "a", "Trimmed before B read it", "i-1");
    tail(&a).trim_older_than("0 seconds").expect("trimmed");
    assert_eq!(db.count("SELECT count(*) FROM outbox"), 0, "trimmed");
    b.hear();
    let told = frames(&b, &theirs);
    assert!(
        told.iter().any(|f| f.contains("Reload")),
        "told to reload: {told:?}"
    );
    assert_eq!(
        tail(&b).cursor(),
        db.count("SELECT through FROM outbox_trimmed"),
        "on from the trimmed id"
    );
}

/// **A notification wakes a waiting host at the commit**, long before the
/// wait's end: what a page on B waits for a commit through A is the
/// database's delivery, not a poll's period.
#[test]
fn a_commit_wakes_a_waiting_host() {
    let Some(url) = database() else { return };
    let db = Schema::new(&url, "woken");
    let a = feed_host(&url, &db);
    let b = feed_host(&url, &db);
    tail(&b).backend_pid().expect("B listens");
    let waited = std::thread::scope(|scope| {
        let waiting = scope.spawn(|| {
            let started = std::time::Instant::now();
            tail(&b).wait(20_000);
            started.elapsed()
        });
        std::thread::sleep(std::time::Duration::from_millis(300));
        post(&a, "a", "Wakes B", "i-1");
        waiting.join().expect("waited")
    });
    assert!(
        waited < std::time::Duration::from_secs(10),
        "woken by the commit, not the wait's end: {waited:?}"
    );
}

/// **Ids are taken in commit order**: a transaction that takes the
/// outbox's lock waits for one that holds it to commit, so its rows take
/// later ids and commit later, and a reader that has seen an id has seen
/// every smaller one that will ever commit.
#[test]
fn outbox_ids_are_taken_in_commit_order() {
    let Some(url) = database() else { return };
    let db = Schema::new(&url, "ordered");
    let a = feed_host(&url, &db);
    let outbox = tail(&a).outbox().clone();
    let insert = |c: &mut postgres::Client| -> i64 {
        crate::hosts::ordered(c, &outbox).expect("ordered");
        c.query_one(
            "INSERT INTO outbox (kind, name, args, host) \
             VALUES ('event', 'feed.app.Posted', '[]', 'h-test') RETURNING id",
            &[],
        )
        .expect("inserted")
        .get(0)
    };
    let mut first = db.sql();
    first.batch_execute("BEGIN").expect("begun");
    let first_id = insert(&mut first);
    let (second_id, second_done) = std::thread::scope(|scope| {
        let second = scope.spawn(|| {
            let mut c = db.sql();
            c.batch_execute("BEGIN").expect("begun");
            let id = insert(&mut c);
            c.batch_execute("COMMIT").expect("committed");
            (id, std::time::Instant::now())
        });
        std::thread::sleep(std::time::Duration::from_millis(500));
        assert!(
            !second.is_finished(),
            "the second waits for the first's lock"
        );
        let committed = std::time::Instant::now();
        first.batch_execute("COMMIT").expect("committed");
        let (id, done) = second.join().expect("the second");
        assert!(done >= committed, "the second commits after the first");
        (id, done)
    });
    let _ = second_done;
    assert!(second_id > first_id, "{second_id} after {first_id}");
}

// ---------------------------------------------------------------------------
// Sessions kept where the hosts share them
// ---------------------------------------------------------------------------

/// [`feed_host`], its readers signed in through `provider`.
fn feed_host_signing_in(url: &str, schema: &Schema, provider: &Arc<TestProvider>) -> Served {
    let s = feed_host(url, schema);
    s.identity.use_provider(provider.clone(), REDIRECT);
    s
}

/// A post through `s` by `session`, over HTTP, as a browser sends it: the
/// request reads its session as the hosts keep it.
fn posted_by(s: &Server, session: &str, interaction: &str, text: &str) -> String {
    command(
        s,
        "feed.app.post",
        session,
        interaction,
        &serde_json::json!([text]).to_string(),
    )
}

/// **A sign-in begun on one host completes on another, and its session holds
/// on both** (ADR-XXXX): the sign-in under way is kept in the database, so
/// the provider's callback completes it on whichever host it reaches; the
/// session it opens is kept there by its id's hash, so each host's next
/// request finds its reader. Until ADR-XXXX both were the first host's
/// memory: the callback on B was refused, and a session made on A was no one
/// on B.
#[test]
fn a_sign_in_begun_on_one_host_completes_on_another_and_holds_on_both() {
    let Some(url) = database() else { return };
    let db = Schema::new(&url, "signed");
    let provider = Arc::new(TestProvider::default());
    let a = feed_host_signing_in(&url, &db, &provider);
    let b = feed_host_signing_in(&url, &db, &provider);
    let started = get(&a, "/sign-in", "pw-session=s-came");
    let (state, _) = set_cookie(&started, "pw-sign-in").expect("the sign-in's cookie");
    let back = provider.authenticate(&location(&started), "ada", "Ada");
    let answer = get(
        &b,
        &format!("/sign-in/callback?{back}"),
        &format!("pw-session=s-came; pw-sign-in={state}"),
    );
    assert!(answer.starts_with("HTTP/1.1 303"), "{answer}");
    let ada = set_cookie(&answer, "pw-session").expect("a session").0;
    for (host, text) in [(&a, "Signed in on B, posting on A"), (&b, "And on B")] {
        let answer = posted_by(host, &ada, &format!("i-{}", text.len()), text);
        assert!(answer.contains("\"committed\":true"), "{answer}");
    }
    assert_eq!(
        db.count("SELECT count(*) FROM posts WHERE author = 'ada'"),
        2,
        "both ada's"
    );
    // Kept by its hash alone: no column holds the id itself.
    let mut c = db.sql();
    let hashed: i64 = c
        .query_one(
            "SELECT count(*) FROM sessions WHERE hash = $1",
            &[&crate::identity::kept_hash(&ada)],
        )
        .expect("counted")
        .get(0);
    assert_eq!(hashed, 1, "the session kept by its hash");
    let plain: i64 = c
        .query_one(
            "SELECT count(*) FROM sessions WHERE hash = convert_to($1, 'UTF8') \
             OR user_id = $1 OR handle = $1 OR name = $1 OR issuer = $1",
            &[&ada],
        )
        .expect("counted")
        .get(0);
    assert_eq!(plain, 0, "never the id itself");
    assert_eq!(
        db.count("SELECT count(*) FROM sign_ins"),
        0,
        "the sign-in taken once"
    );
}

/// **A session signed out on one host is no one on another** (ADR-0258's
/// sign-out, held across hosts): B's next request reads no session, and its
/// post is refused for `SignedIn`.
#[test]
fn a_session_signed_out_on_one_host_is_no_one_on_another() {
    let Some(url) = database() else { return };
    let db = Schema::new(&url, "signed_out");
    let provider = Arc::new(TestProvider::default());
    let a = feed_host_signing_in(&url, &db, &provider);
    let b = feed_host_signing_in(&url, &db, &provider);
    let ada = super::sign_in::signed_in(&a, &provider, "s-came", "ada");
    let answer = posted_by(&b, &ada, "i-1", "Signed in on A, posting on B");
    assert!(answer.contains("\"committed\":true"), "{answer}");
    let out = exchanged(
        &a,
        &format!(
            "POST /sign-out HTTP/1.1\r\nHost: t\r\nCookie: pw-session={ada}\r\n\
             Sec-Fetch-Site: same-origin\r\ncontent-length: 0\r\n\r\n"
        ),
    );
    assert!(out.starts_with("HTTP/1.1 303"), "{out}");
    let refused = posted_by(&b, &ada, "i-2", "Signed out on A");
    assert!(refused.starts_with("HTTP/1.1 403"), "{refused}");
    assert!(refused.contains("SignedIn"), "{refused}");
    assert_eq!(db.count("SELECT count(*) FROM sessions"), 0, "none kept");
}

/// **A restarted host keeps its readers signed in** (ADR-XXXX): a host
/// started again on the database finds the session a sign-in made before.
/// Until ADR-XXXX a restart signed everyone out.
#[test]
fn a_restarted_host_keeps_its_readers_signed_in() {
    let Some(url) = database() else { return };
    let db = Schema::new(&url, "restarted");
    let provider = Arc::new(TestProvider::default());
    let ada = {
        let a = feed_host_signing_in(&url, &db, &provider);
        super::sign_in::signed_in(&a, &provider, "s-came", "ada")
    };
    let again = feed_host_signing_in(&url, &db, &provider);
    let answer = posted_by(&again, &ada, "i-1", "After a restart");
    assert!(answer.contains("\"committed\":true"), "{answer}");
}

/// **A request reads its session once**: one statement, whatever the page
/// asks of its reader; none for a request with no session yet.
#[test]
fn a_request_reads_its_session_once() {
    let Some(url) = database() else { return };
    let db = Schema::new(&url, "read_once");
    let provider = Arc::new(TestProvider::default());
    let a = feed_host_signing_in(&url, &db, &provider);
    let ada = super::sign_in::signed_in(&a, &provider, "s-came", "ada");
    let kept = a.data.sessions().expect("sessions kept");
    let before = kept.ran();
    let page = get(&a, "/", &format!("pw-session={ada}"));
    assert!(page.starts_with("HTTP/1.1 200"), "{page}");
    assert_eq!(kept.ran() - before, 1, "one read for the request");
    let before = kept.ran();
    let fresh = get(&a, "/", "");
    assert!(fresh.starts_with("HTTP/1.1 200"), "{fresh}");
    assert_eq!(kept.ran(), before, "none for a request with no session");
}

// ---------------------------------------------------------------------------
// The store on two hosts
// ---------------------------------------------------------------------------

/// The database the store's tests run against.
fn store_database() -> Option<String> {
    match std::env::var("PW_STORE_DATABASE_URL") {
        Ok(url) if !url.is_empty() => Some(url),
        _ => {
            eprintln!("skipped: PW_STORE_DATABASE_URL is not set");
            None
        }
    }
}

/// **Two hosts of the canonical store on one schema**, each its own layer,
/// their readers signed in through `provider`; dropped B, then A, then
/// their layers, A's dropping the schema.
struct Stores {
    b: Server,
    a: Server,
    _b_layer: Arc<StorePg>,
    _a_layer: Arc<StorePg>,
    _dir: tempfile::TempDir,
}

fn stores(url: &str, provider: &Arc<TestProvider>) -> Stores {
    let a_layer = Arc::new(StorePg::for_test(url).expect("A's layer"));
    let b_layer = Arc::new(
        StorePg::open_with(url, a_layer.schema(), Isolation::Serializable).expect("B's layer"),
    );
    let (dir, out) = built_from_patches_in("examples", |app| app.to_string(), &[]);
    let host = |layer: &Arc<StorePg>| {
        let s = Server::from_build_layers(out.clone(), out.clone(), None, Some(layer.clone()))
            .expect("served on PostgreSQL");
        s.identity.use_provider(provider.clone(), REDIRECT);
        s
    };
    Stores {
        a: host(&a_layer),
        b: host(&b_layer),
        _b_layer: b_layer,
        _a_layer: a_layer,
        _dir: dir,
    }
}

const CART: &str = "store.page.CartPage";

/// **A reader's cart changed through one host is told to their page on
/// another**: ada, signed in through A, reads her cart on B (her session
/// found in the database by B's request); her add through A reaches B's page
/// when B reads the outbox, by principal, as her other session on one host
/// is told (ADR-0318).
#[test]
fn a_readers_cart_changed_through_one_host_is_told_to_their_page_on_another() {
    let Some(url) = store_database() else { return };
    let provider = Arc::new(TestProvider::default());
    let hosts = stores(&url, &provider);
    let ada = super::sign_in::signed_in(&hosts.a, &provider, "came-ada", "ada");
    // What B's request for the page does first.
    hosts.b.identity.principals().refresh(&ada);
    hosts
        .b
        .serve_document_settled(&ada, CART, &Params::new(), &[])
        .expect("served on B");
    let doc = latest(&hosts.b.pending.lock().expect("pending"), &ada);
    let before = sets_of(&hosts.b, &doc).len();
    hosts
        .a
        .command(ADD, &ada, &add_shown("espresso", 1), false)
        .expect("ada adds through A");
    assert_eq!(sets_of(&hosts.b, &doc).len(), before, "not before B reads it");
    hosts.b.hear();
    let told: Vec<String> = sets_of(&hosts.b, &doc)[before..]
        .iter()
        .map(written)
        .collect();
    assert!(
        told.iter().any(|w| w.contains("$3.50")),
        "her page on B is told: {told:?}"
    );
}

/// **A retry through another host of a press one host committed runs once**
/// (ADR-0325, held across two live hosts): the same interaction sent to B
/// is answered from A's record, committed with A's writes, and the cart
/// holds the one line.
#[test]
fn a_retry_through_another_host_is_answered_from_the_first_hosts_record() {
    let Some(url) = store_database() else { return };
    let provider = Arc::new(TestProvider::default());
    let hosts = stores(&url, &provider);
    let ada = super::sign_in::signed_in(&hosts.a, &provider, "came-ada", "ada");
    let request = [shown("espresso"), serde_json::json!(1)];
    let first = hosts
        .a
        .command_answer(ADD, &ada, &request, Some("press-1"))
        .expect("a well-formed request");
    assert!(first.committed, "{first:?}");
    hosts.b.identity.principals().refresh(&ada);
    let again = hosts
        .b
        .command_answer(ADD, &ada, &request, Some("press-1"))
        .expect("a well-formed request");
    assert_eq!(again, first, "A's answer");
    assert_eq!(hosts.b.cart_lines(&ada), [("espresso".to_string(), 1)]);
    assert_eq!(hosts.a.cart_lines(&ada), [("espresso".to_string(), 1)]);
}

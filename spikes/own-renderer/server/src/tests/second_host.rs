//! **Hosts sharing one database** (ADR-XXXX, track `second-host`): two hosts
//! in one process, each its own `Server` on a layer of its own opened on one
//! schema, as two processes on one PostgreSQL are. Each test passes doing
//! nothing without a database: `just e14-second-host` runs them.

use super::*;
use crate::feed_pg::FeedPg;
use crate::hosts::Tail;

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

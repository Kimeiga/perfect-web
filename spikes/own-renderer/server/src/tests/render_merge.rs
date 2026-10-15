//! **One render's identical calls made once, in one snapshot** (NEXT 4c's
//! first step, track `render-merge`; W9's design note, its second case):
//! the host's render merge served, on the store and on the feed.
//!
//! - **Each identical call once**: a session's documents read again after a
//!   commit are one render, so the cart every page's layout shows is read
//!   once, and the feed's timeline once for two of its documents; the
//!   control, each store page's reach by its own store, read for each.
//! - **One snapshot**: another writer's commit landing between two
//!   bindings' reads is seen by neither, on the store's page in memory and
//!   on PostgreSQL (the layer `PW_STORE_TEST_LAYER` says), and on the feed's
//!   page on PostgreSQL; the control, the same two bindings read in two
//!   renders, which straddle it.
//! - **Each layer's snapshot** (`DataLayer::snapshot`, `reads_in`): the
//!   store's and the feed's, in memory and on PostgreSQL; on PostgreSQL, a
//!   render's read that fails ends its transaction, and a render's
//!   connection does not wait on the pool's bound, which a single read does.

use super::*;
use crate::data::{DataLayer, Ops};
use std::time::Duration;

fn text(v: &str) -> Val {
    Val::String(v.to_string())
}

/// The store, served on the layer the server's tests serve it on.
fn store() -> Served {
    served_from_patches_in("examples", |app| app.to_string(), &[])
}

/// Each data-layer operation's calls so far (`/metrics`).
fn calls(s: &Server) -> BTreeMap<String, u64> {
    s.calls.lock().expect("calls").clone()
}

/// The calls made since `before`, by operation.
fn since(s: &Server, before: &BTreeMap<String, u64>) -> BTreeMap<String, u64> {
    calls(s)
        .into_iter()
        .filter_map(|(op, n)| {
            let made = n - before.get(&op).copied().unwrap_or_default();
            (made > 0).then_some((op, made))
        })
        .collect()
}

/// **What a cart's lines cost together**, as the cart shows them.
fn cart_subtotal(cart: &Val) -> i64 {
    let Some(Val::List(lines)) = val_at(cart, &["lines"]) else {
        panic!("no cart: {cart:?}");
    };
    lines
        .iter()
        .map(
            |l| match (val_at(l, &["quantity"]), val_at(l, &["unit-price"])) {
                (Some(Val::S64(quantity)), Some(Val::S64(price))) => quantity * price,
                other => panic!("no line: {other:?}"),
            },
        )
        .sum()
}

/// **The subtotal the checkout charges**, as the cart page shows it.
fn checkout_subtotal(checkout: &Val) -> i64 {
    match val_at(checkout, &["charges", "subtotal"]) {
        Some(Val::S64(n)) => *n,
        other => panic!("no subtotal: {other:?} in {checkout:?}"),
    }
}

/// **A session's documents read again after a commit are one render, and
/// each identical call of it is made once**: the cart every page's layout
/// shows, which the drain reads and each of three documents reads, is read
/// once. The control: each store page's reach, one operation called with
/// two stores, is read for each.
#[test]
fn a_commits_documents_are_read_in_one_render_each_identical_call_once() {
    let s = store();
    s.command(
        "store.page.add_to_cart",
        "a",
        &add_shown("espresso", 1),
        false,
    )
    .expect("added");
    for path in ["/stores/47", "/stores/48", "/cart"] {
        fetched_as(&s, path, Some("a"));
    }
    let before = calls(&s);
    s.command("store.page.add_to_cart", "a", &add_shown("drip", 1), false)
        .expect("added");
    let made = since(&s, &before);
    assert_eq!(
        made.get("store:data/user-carts#current"),
        Some(&1),
        "the cart, once for the drain and three documents: {made:?}"
    );
    assert_eq!(
        made.get("store:data/coverage#for-store"),
        Some(&2),
        "each store's reach, by its own arguments: {made:?}"
    );
}

/// **A render merges the layer's calls, never the platform's**: two queries
/// of one render that compare the clock each note what the clock holds them
/// until (ADR-0304), which a call answered again would not.
#[test]
fn a_renders_queries_each_note_what_the_clock_holds_them_until() {
    let s = store();
    let _render = crate::render_merge::begin(&*s.data, "a");
    let status = || {
        clock::noting(|| {
            s.answer("store.hours.StoreStatus", "a", &[text("47")])
                .expect("answered")
        })
        .1
    };
    let (one, two) = (status(), status());
    assert!(one.is_some(), "the status compares the clock");
    assert_eq!(one, two, "each query notes it");
}

/// The reader's cart page, served, and its document: a line of store 47's,
/// one espresso.
fn cart_page(s: &Server) -> u64 {
    s.command(
        "store.page.add_to_cart",
        "a",
        &add_shown("espresso", 1),
        false,
    )
    .expect("added");
    let (_, doc, _, _) = s
        .serve_document_settled("a", "store.page.CartPage", &Params::new(), &[])
        .expect("served");
    doc
}

/// **Two bindings of one render read one snapshot**: another writer's
/// commit landing after the render read the cart, five espressos where
/// there was one, is not what its checkout prices either; the next render
/// reads it in both.
#[test]
fn two_bindings_of_one_render_read_one_snapshot() {
    let s = store();
    let doc = cart_page(&s);
    let owner = s.owner("a");
    *s.between_bindings.lock().expect("between") = Some(Box::new(move |s: &Server| {
        s.store
            .line_around(&owner, 0, 5)
            .expect("another writer's commit");
    }));
    let read = s.bindings("a", doc).expect("read");
    assert_eq!(lines_of(&read["cart"]), [("espresso".to_string(), 1)]);
    assert_eq!(
        checkout_subtotal(&read["checkout"]),
        cart_subtotal(&read["cart"]),
        "the checkout prices the cart the page shows"
    );
    let read = s.bindings("a", doc).expect("read");
    assert_eq!(lines_of(&read["cart"]), [("espresso".to_string(), 5)]);
    assert_eq!(
        checkout_subtotal(&read["checkout"]),
        cart_subtotal(&read["cart"])
    );
}

/// **The control: two renders straddle a commit between them.** The same
/// two bindings, each read in a render of its own, the commit between: the
/// checkout prices five espressos, and the cart showed one.
#[test]
fn two_renders_straddle_a_commit_between_them() {
    let s = store();
    let doc = cart_page(&s);
    let one = |binding: &str| {
        s.bindings_where("a", Some(doc), |b| b == binding, &Keys::Shown)
            .expect("read")
            .remove(binding)
            .expect("the binding")
    };
    let cart = one("cart");
    s.store
        .line_around(&s.owner("a"), 0, 5)
        .expect("another writer's commit");
    let checkout = one("checkout");
    assert_eq!(lines_of(&cart), [("espresso".to_string(), 1)]);
    assert_eq!(checkout_subtotal(&checkout), 5 * cart_subtotal(&cart));
}

/// **The feed's documents read again after a commit are one render**: a
/// session's two timelines, read again after its post, read its timeline
/// once. Each document's alone, it was read twice.
#[test]
fn a_feed_sessions_documents_are_read_in_one_render_each_identical_call_once() {
    let s = served_feed();
    for _ in 0..2 {
        s.serve_document_settled("a", "feed.app.Home", &Params::new(), &[])
            .expect("served");
    }
    let before = calls(&s);
    let answered = s
        .command_answered("feed.app.post", "a", &[text("One render.")], Some("i-1"))
        .expect("runs");
    assert!(answered.committed, "{:?}", answered.result);
    let made = since(&s, &before);
    assert_eq!(
        made.get("feed:data/posts#timeline"),
        Some(&1),
        "one timeline for two documents: {made:?}"
    );
}

fn feed_database() -> Option<String> {
    match std::env::var("PW_FEED_DATABASE_URL") {
        Ok(url) if !url.is_empty() => Some(url),
        _ => {
            eprintln!("skipped: PW_FEED_DATABASE_URL is not set");
            None
        }
    }
}

fn store_database() -> Option<String> {
    match std::env::var("PW_STORE_DATABASE_URL") {
        Ok(url) if !url.is_empty() => Some(url),
        _ => {
            eprintln!("skipped: PW_STORE_DATABASE_URL is not set");
            None
        }
    }
}

/// A notification for the guest `a` reads as, as another writer commits it.
const NOTIFIED: &str =
    "INSERT INTO notifications (recipient, actor, act) VALUES ('u-a', 'u-grace', 'followed')";

/// How many notifications a list binding shows.
fn listed(notes: &Val) -> usize {
    match notes {
        Val::List(items) => items.len(),
        other => panic!("no list: {other:?}"),
    }
}

/// **On PostgreSQL, the feed's render reads one snapshot**: a notification
/// committed after the render read the reader's list is not counted unread
/// either; the next render reads it in both.
#[test]
fn on_postgres_a_feed_renders_bindings_read_one_snapshot() {
    let Some(url) = feed_database() else { return };
    let s = super::feed_pg::served_on(&url, "render_merge_one");
    let (_, doc, _, _) = s
        .serve_document_settled("a", "feed.app.NotificationsPage", &Params::new(), &[])
        .expect("served");
    *s.between_bindings.lock().expect("between") = Some(Box::new(|s: &Server| {
        s.data
            .execute_for_test(NOTIFIED)
            .expect("another writer's commit");
    }));
    let read = s.bindings("a", doc).expect("read");
    assert_eq!(
        (listed(&read["notes"]), &read["unread"]),
        (0, &Val::S64(0)),
        "neither sees it"
    );
    let read = s.bindings("a", doc).expect("read");
    assert_eq!((listed(&read["notes"]), &read["unread"]), (1, &Val::S64(1)));
}

/// **The control, on PostgreSQL: the feed's two renders straddle a commit
/// between them**, the list read before it and the count after.
#[test]
fn on_postgres_a_feeds_two_renders_straddle_a_commit_between_them() {
    let Some(url) = feed_database() else { return };
    let s = super::feed_pg::served_on(&url, "render_merge_two");
    let (_, doc, _, _) = s
        .serve_document_settled("a", "feed.app.NotificationsPage", &Params::new(), &[])
        .expect("served");
    let one = |binding: &str| {
        s.bindings_where("a", Some(doc), |b| b == binding, &Keys::Shown)
            .expect("read")
            .remove(binding)
            .expect("the binding")
    };
    let notes = one("notes");
    s.data
        .execute_for_test(NOTIFIED)
        .expect("another writer's commit");
    let unread = one("unread");
    assert_eq!((listed(&notes), unread), (0, Val::S64(1)));
}

/// The first line's quantity of `ada`'s cart, as `ops` read it.
fn ada_quantity(ops: Ops) -> i64 {
    match &ops["store:data/user-carts#current"](&[text("ada")]).expect("read")[..] {
        [Val::Result(Ok(Some(cart)))] => lines_of(cart)[0].1,
        other => panic!("no cart: {other:?}"),
    }
}

/// `ada`'s espresso added, in a command of the memory store's, committed.
fn ada_adds(layer: &crate::store::StoreData) {
    let mut staging = DataLayer::begin(layer, "s");
    staging.ops()["store:data/user-carts#add"](&[text("ada"), text("espresso"), Val::S64(1)])
        .expect("added");
    staging.publish();
}

/// **The store's snapshot in memory** is the store as the render first read
/// it: a commit after is read by a read of its own, not by the render's.
#[test]
fn the_memory_stores_snapshot_is_the_store_as_a_render_first_read_it() {
    let layer = crate::store::StoreData::new();
    ada_adds(&layer);
    let snapshot = layer.snapshot().expect("a render's snapshot");
    assert_eq!(ada_quantity(layer.reads_in(&snapshot, "s", None)), 1);
    ada_adds(&layer);
    assert_eq!(
        ada_quantity(layer.reads_in(&snapshot, "s", None)),
        1,
        "the render's"
    );
    assert_eq!(ada_quantity(layer.reads("s", None)), 2, "a read's own");
}

/// The store on PostgreSQL, `ada` with one espresso in her cart.
fn store_on_postgres(url: &str) -> (crate::store_pg::StorePg, postgres::Client) {
    let layer = crate::store_pg::StorePg::for_test(url).expect("the layer");
    let mut c = layer.sql();
    c.execute(
        "INSERT INTO cart_lines (owner, position, item, name, quantity, price) \
         VALUES ('ada', 0, 'espresso', 'Espresso', 1, 350)",
        &[],
    )
    .expect("a cart");
    (layer, c)
}

/// **The store's snapshot on PostgreSQL** is a render's read transaction,
/// repeatable read: another writer's commit after its first read is read by
/// a read of its own, not by the render's.
#[test]
fn on_postgres_the_stores_snapshot_is_a_renders_read_transaction() {
    let Some(url) = store_database() else { return };
    let (layer, mut c) = store_on_postgres(&url);
    let snapshot = layer.snapshot().expect("a render's snapshot");
    assert_eq!(ada_quantity(layer.reads_in(&snapshot, "s", None)), 1);
    c.execute(
        "UPDATE cart_lines SET quantity = 5 WHERE owner = 'ada'",
        &[],
    )
    .expect("another writer's commit");
    assert_eq!(
        ada_quantity(layer.reads_in(&snapshot, "s", None)),
        1,
        "the render's"
    );
    assert_eq!(ada_quantity(layer.reads("s", None)), 5, "a read's own");
}

/// **On PostgreSQL, a render's read that fails ends its transaction**, which
/// it aborted: the render's next read opens another, and is answered.
#[test]
fn on_postgres_a_renders_failed_read_ends_its_transaction() {
    let Some(url) = store_database() else { return };
    let (layer, mut c) = store_on_postgres(&url);
    let snapshot = layer.snapshot().expect("a render's snapshot");
    let read = |op: &str| layer.reads_in(&snapshot, "s", None)[op](&[text("47")]);
    read("store:data/stores#get").expect("the render's first read");
    c.batch_execute("ALTER TABLE notices RENAME TO notices_away")
        .expect("renamed");
    let failed = read("store:data/notices#current");
    c.batch_execute("ALTER TABLE notices_away RENAME TO notices")
        .expect("renamed back");
    assert!(failed.is_err(), "a read that fails: {failed:?}");
    read("store:data/kitchen#prep-minutes").expect("the render's next read");
}

/// One read of store 47 in a render of its own, on `layer`.
fn read_in_a_render(layer: &crate::store_pg::StorePg) -> Result<Vec<Val>, String> {
    let snapshot = layer.snapshot().expect("a render's snapshot");
    layer.reads_in(&snapshot, "s", None)["store:data/stores#get"](&[text("47")])
}

/// **On PostgreSQL, a render's connection does not wait on the pool's
/// bound**: with every connection the bound allows held, a render reads.
/// The control: a single read waits, and reads once one is given back; the
/// render's connection, given back past the bound, was not kept for it.
#[test]
fn on_postgres_a_renders_connection_does_not_wait_on_the_bound() {
    let Some(url) = store_database() else { return };
    let (layer, _c) = store_on_postgres(&url);
    let (tx, rx) = std::sync::mpsc::channel();
    let held = layer.hold_the_bound();
    std::thread::scope(|scope| {
        let layer = &layer;
        scope.spawn(move || tx.send(read_in_a_render(layer)));
        let read = rx.recv_timeout(Duration::from_secs(20));
        drop(held);
        assert!(
            matches!(read, Ok(Ok(_))),
            "a render's read, the bound held: {read:?}"
        );
    });
    let (tx, rx) = std::sync::mpsc::channel();
    let held = layer.hold_the_bound();
    std::thread::scope(|scope| {
        let layer = &layer;
        scope.spawn(move || {
            tx.send(layer.reads("s", None)["store:data/stores#get"](&[text(
                "47",
            )]))
        });
        let early = rx.recv_timeout(Duration::from_millis(500));
        drop(held);
        let late = rx.recv_timeout(Duration::from_secs(20));
        assert!(early.is_err(), "a single read waits: {early:?}");
        assert!(matches!(late, Ok(Ok(_))), "and then reads: {late:?}");
    });
}

/// **The feed's snapshot in memory** is the feed as the render first read
/// it: a post committed after is read by a read of its own, not by the
/// render's.
#[test]
fn the_memory_feeds_snapshot_is_the_feed_as_a_render_first_read_it() {
    let layer = crate::feed::FeedData::new();
    let timeline = |ops: Ops| match &ops["feed:data/posts#timeline"](&[text("a"), Val::S64(20)])
        .expect("read")[..]
    {
        [Val::List(posts)] => posts.len(),
        other => panic!("no timeline: {other:?}"),
    };
    let snapshot = layer.snapshot().expect("a render's snapshot");
    let before = timeline(layer.reads_in(&snapshot, "a", None));
    {
        let mut staging = DataLayer::begin(&layer, "a");
        staging.ops()["feed:data/posts#publish"](&[
            text("a"),
            text("After the render's first read."),
        ])
        .expect("posted");
        staging.publish();
    }
    assert_eq!(
        timeline(layer.reads_in(&snapshot, "a", None)),
        before,
        "the render's"
    );
    assert_eq!(timeline(layer.reads("a", None)), before + 1, "a read's own");
}

/// **The feed's snapshot on PostgreSQL** is a render's read transaction: a
/// notification committed after its first read is counted by a read of its
/// own, not by the render's.
#[test]
fn on_postgres_the_feeds_snapshot_is_a_renders_read_transaction() {
    let Some(url) = feed_database() else { return };
    let s = super::feed_pg::served_on(&url, "render_merge_layer");
    let unread = |ops: Ops| ops["feed:data/notifications#unread"](&[text("u-a")]).expect("read");
    let snapshot = s.data.snapshot().expect("a render's snapshot");
    assert_eq!(unread(s.data.reads_in(&snapshot, "a", None)), [Val::S64(0)]);
    s.data
        .execute_for_test(NOTIFIED)
        .expect("another writer's commit");
    assert_eq!(
        unread(s.data.reads_in(&snapshot, "a", None)),
        [Val::S64(0)],
        "the render's"
    );
    assert_eq!(
        unread(s.data.reads("a", None)),
        [Val::S64(1)],
        "a read's own"
    );
}

/// **On PostgreSQL, the feed's render read that fails ends its
/// transaction**, as the store's does: the render's next read opens another,
/// and is answered.
#[test]
fn on_postgres_a_feed_renders_failed_read_ends_its_transaction() {
    let Some(url) = feed_database() else { return };
    let s = super::feed_pg::served_on(&url, "render_merge_failed");
    let snapshot = s.data.snapshot().expect("a render's snapshot");
    let read = |op: &str, arg: &str| s.data.reads_in(&snapshot, "a", None)[op](&[text(arg)]);
    read("feed:data/posts#author", "p1").expect("the render's first read");
    s.data
        .execute_for_test("ALTER TABLE notifications RENAME TO notifications_away")
        .expect("renamed");
    let failed = read("feed:data/notifications#unread", "u-a");
    s.data
        .execute_for_test("ALTER TABLE notifications_away RENAME TO notifications")
        .expect("renamed back");
    assert!(failed.is_err(), "a read that fails: {failed:?}");
    read("feed:data/messages#unread", "u-a").expect("the render's next read");
}

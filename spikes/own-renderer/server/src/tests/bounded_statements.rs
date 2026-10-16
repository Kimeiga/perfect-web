//! **A read is a bounded number of statements on PostgreSQL** (NEXT 4b, W9,
//! the integrator's finding of 2026-10-12): every operation of the store's
//! data layer, and every page and command of the store, counted (as Django's
//! `assertNumQueries` and Rails' `assert_queries_count` count them) at the
//! seed and with fifty stores more, each count pinned and equal at both.
//! Each operation's count is "operation X makes k statements per call": the
//! shape a page's or a command's budget composes from (NEXT 4c), and the
//! conformance a layer's adapter is held to. On PostgreSQL alone, which
//! counts its statements (`store_pg.rs`' `Counted`); skipped without
//! `PW_STORE_DATABASE_URL`.

use super::*;

/// The reader the operations act for, a user's handle.
const ADA: &str = "ada";

fn database() -> Option<String> {
    match std::env::var("PW_STORE_DATABASE_URL") {
        Ok(url) if !url.is_empty() => Some(url),
        _ => {
            eprintln!("skipped: PW_STORE_DATABASE_URL is not set");
            None
        }
    }
}

/// **Fifty stores more**, each as a seeded one is: a category, two items,
/// open the day round, San Francisco's fees. Listed before the seeded ones,
/// so a loop over the stores that stops at the store it wants still meets
/// every one of them.
const FIFTY_MORE: &str = "
    INSERT INTO stores (id, position, name, description, cuisine, opens_minute,
                        closes_minute, lat_e6, lon_e6, radius_m)
    SELECT 'more-' || n, -n, 'More ' || n, 'One more store.', 'Coffee', 420, 1140,
           37780000 + n, -122400000 - n, 3000
    FROM generate_series(1, 50) AS n;
    INSERT INTO categories (store, id, name)
    SELECT 'more-' || n, 'coffee', 'Coffee' FROM generate_series(1, 50) AS n;
    INSERT INTO menu_items (store, id, position, name, description, price, category,
                            served, available)
    SELECT 'more-' || n, 'more-' || n || '-' || k, k, 'Coffee ' || k, 'A coffee.', 300 + k,
           'coffee', 'hot', true
    FROM generate_series(1, 50) AS n, generate_series(1, 2) AS k;
    INSERT INTO store_hours (store, weekday, opens_minute, closes_minute)
    SELECT 'more-' || n, d, 0, 0 FROM generate_series(1, 50) AS n, generate_series(1, 7) AS d;
    INSERT INTO store_fees
    SELECT 'more-' || n, 299, 150000, 300, 1200, 1000, 250, 86250, false, false
    FROM generate_series(1, 50) AS n;
";

/// The reader's state every operation is read against: a cart with a line
/// from each seeded store, and an address chosen.
fn readers_state(c: &mut postgres::Client) {
    let place = crate::places::PLACES[0].id;
    c.execute(
        "INSERT INTO cart_lines (owner, position, item, name, quantity, price) VALUES \
         ($1, 0, 'espresso', 'Espresso', 2, 350), ($1, 1, 'drip', 'Drip Coffee', 1, 300)",
        &[&ADA],
    )
    .expect("a cart");
    c.execute(
        "INSERT INTO addresses (owner, id, position, label, place, chosen) \
         VALUES ($1, 'addr-1', 0, 'Home', $2, true)",
        &[&ADA, &place],
    )
    .expect("an address");
    // TRACK SEAM (reviews): another reader, whose order from store 47 was
    // delivered, so a rating is read and written whole.
    c.batch_execute(&format!(
        "INSERT INTO orders (owner, status) VALUES ('{BEA}', 'delivered');
         INSERT INTO order_parts (order_id, position, store, subtotal, discount, funded_by,
                                  delivery, service, small_order, taxable, tax_rate, tax)
         SELECT id, 0, '47', 350, 0, NULL, 299, 300, 250, 350, 86250, 30
         FROM orders WHERE owner = '{BEA}';"
    ))
    .expect("a delivered order");
}

/// TRACK SEAM (reviews): the reader who rates, her order delivered.
const BEA: &str = "bea";

/// Every store's id, as a list a set read takes.
fn all_stores(c: &mut postgres::Client) -> Val {
    Val::List(
        c.query("SELECT id FROM stores ORDER BY position", &[])
            .expect("the stores")
            .iter()
            .map(|r| Val::String(r.get(0)))
            .collect(),
    )
}

fn s(v: &str) -> Val {
    Val::String(v.to_string())
}

/// **Each operation's arguments**, by its name: what a program passes it.
/// An operation not named here fails the test: every one is counted.
fn arguments(op: &str, stores: &Val) -> Option<Vec<Val>> {
    let charged = crate::checkout::charged_val(&crate::checkout::Charged {
        parts: Vec::new(),
        tip: 0,
        total: 1000,
        promo: None,
    });
    let op = op.strip_prefix("store:")?;
    Some(match op {
        "data/stores#get"
        | "data/menus#for-store"
        | "data/menus#sections"
        | "data/notices#current"
        | "data/kitchen#prep-minutes"
        | "data/recommendations#for-store"
        | "data/hours#for-store" => vec![s("47")],
        "data/stores#list" | "data/places#list" => vec![],
        "data/stores#nowhere" => vec![s("47")],
        "data/menus#is-available" => vec![s("espresso")],
        "data/menus#in-category" => vec![s("48"), s("hot")],
        "data/carts#current"
        | "data/orders#current"
        | "data/orders#placed"
        | "data/estimates#current"
        | "data/carts#clear"
        | "data/orders#place" => vec![s("s")],
        "data/user-carts#current"
        | "data/user-orders#current"
        | "data/user-orders#placed"
        | "data/user-orders#charged"
        | "data/user-orders#awaited"
        | "data/addresses#list"
        | "data/hours#for-cart"
        | "data/checkout#parts"
        | "data/checkout#chosen"
        | "data/user-carts#clear" => vec![s(ADA)],
        "data/user-estimates#for-store" | "data/coverage#for-store" | "data/coverage#reaches" => {
            vec![s("47"), s(ADA)]
        }
        "data/user-estimates#for-stores" => vec![stores.clone(), s(ADA)],
        "data/hours#for-stores" | "data/fees#for-stores" => vec![stores.clone()],
        "data/promos#find" => vec![s(ADA), s("WELCOME10")],
        "data/search#find" => vec![s("coffee")],
        "data/search#find-for" => vec![s("coffee"), s(ADA)],
        "data/carts#add" => vec![s("s"), s("espresso"), Val::S64(1)],
        "data/carts#increase" | "data/carts#decrease" | "data/carts#remove" => {
            vec![s("s"), s("espresso")]
        }
        "data/user-carts#add" => vec![s(ADA), s("espresso"), Val::S64(1)],
        "data/user-carts#increase" | "data/user-carts#decrease" | "data/user-carts#remove" => {
            vec![s(ADA), s("espresso")]
        }
        "data/addresses#add" => vec![s(ADA), s("Work"), s(crate::places::PLACES[1].id)],
        "data/addresses#rename" => vec![s(ADA), s("addr-1"), s("House")],
        "data/addresses#remove" | "data/addresses#choose" => vec![s(ADA), s("addr-1")],
        "data/checkout#set-tip" => vec![s(ADA), Val::Variant("fifteen".into(), None)],
        "data/checkout#set-promo" => vec![s(ADA), Val::Option(None)],
        // Track `scheduled-orders`: the time chosen, none; an order sent; and
        // an order awaiting its payment is given its time, none.
        "data/checkout#set-time" => vec![s(ADA), Val::Option(None)],
        "data/user-orders#send" => vec![s(ADA), s("ref-1")],
        "data/user-orders#await-payment" => {
            vec![s(ADA), charged, Val::Option(None), Val::Option(None)]
        }
        "data/user-orders#place" => vec![s(ADA), charged, Val::Option(None)],
        "data/user-orders#paying" => vec![
            s(ADA),
            s("ref-1"),
            Val::Record(vec![
                ("id".into(), s("pay-1")),
                ("page".into(), s("https://pay")),
            ]),
        ],
        "data/user-orders#settle" => {
            vec![s(ADA), s("ref-1"), Val::Variant("succeeded".into(), None)]
        }
        "host/carts#join" => vec![s("guest-1"), s(ADA)],
        "host/estimates#set" => vec![s("s"), Val::S64(20), Val::Option(None)],
        "host/kitchen#set-prep" => vec![s("47"), Val::S64(15)],
        "host/menus#change" => vec![s("47"), s("remove"), s("cortado")],
        "host/menus#replace" => vec![s("47"), Val::S64(3)],
        "host/menus#stock" => vec![s("espresso"), Val::Bool(false)],
        "host/notices#post" => vec![s("47"), s("Closed early.")],
        "host/orders#set-status" => vec![s(ADA), Val::Option(None)],
        "host/recommendations#curate" => vec![Val::Option(None)],
        "host/fees#set" => vec![
            s("47"),
            crate::checkout::fees_val(&crate::checkout::seed_fees()),
        ],
        "host/hours#set" => vec![s("47"), crate::hours::hours_val(&crate::hours::seed())],
        "host/promos#set" => vec![crate::checkout::promo_val(
            &crate::checkout::seed_promos()[0],
        )],
        // TRACK SEAM (reviews): every store's ratings at once, a store's
        // reviews, and the reader's order rated.
        "data/ratings#for-stores" => vec![stores.clone()],
        "data/ratings#reviews" => vec![s("47")],
        "data/ratings#for-order" => vec![s(BEA)],
        "data/ratings#rate" => vec![s(BEA), s("47"), Val::S64(4), s("Fine.")],
        _ => return None,
    })
}

/// **Each operation's statements per call**, at one size of the data: every
/// read, each in its own read transaction, and every write, each in a
/// command's transaction that is then dropped, so nothing it writes stays.
fn counts(layer: &crate::store_pg::StorePg, stores: &Val) -> BTreeMap<String, u64> {
    use crate::data::DataLayer;
    let mut out = BTreeMap::new();
    let counted = |name: &str, op: &HostFn| -> u64 {
        let args = arguments(name, stores).unwrap_or_else(|| panic!("`{name}` is not counted"));
        crate::store_pg::STATEMENTS.with(|n| n.set(0));
        // An operation may answer a refusal: what it ran is still counted.
        let _ = op(&args);
        crate::store_pg::STATEMENTS.with(|n| n.get())
    };
    for (name, op) in layer.reads("s", None) {
        let n = counted(&name, &op);
        out.insert(name, n);
    }
    let names: Vec<String> = layer.begin("s").ops().into_keys().collect();
    for name in names {
        if out.contains_key(&name) {
            continue;
        }
        let staged = layer.begin("s");
        let ops = staged.ops();
        let n = counted(&name, &ops[&name]);
        drop(staged);
        out.insert(name, n);
    }
    out
}

/// **Every operation makes as many statements per call with fifty stores
/// more as at the seed**, and as many as pinned here: one call is a bounded
/// number of statements, whatever the number of stores, and a set read
/// (`hours#for-stores`, `fees#for-stores`) whatever the number of keys.
#[test]
fn each_operation_is_a_bounded_number_of_statements_per_call() {
    let Some(url) = database() else { return };
    let layer = crate::store_pg::StorePg::for_test(&url).expect("the layer");
    let mut c = layer.sql();
    readers_state(&mut c);
    let seed = counts(&layer, &all_stores(&mut c));
    c.batch_execute(FIFTY_MORE).expect("fifty stores more");
    let more = counts(&layer, &all_stores(&mut c));
    let table: Vec<String> = seed
        .iter()
        .map(|(op, n)| format!("{op}: {n} at the seed, {} with fifty more", more[op]))
        .collect();
    eprintln!("statements per call:\n  {}", table.join("\n  "));
    assert_eq!(seed, more, "equal at both sizes");
    let pinned: BTreeMap<&str, u64> = PINNED.iter().copied().collect();
    let found: BTreeMap<&str, u64> = seed.iter().map(|(k, v)| (k.as_str(), *v)).collect();
    assert_eq!(found, pinned, "as pinned");
}

/// **What each operation makes per call**, on PostgreSQL: the store's
/// adapter's conformance (NEXT 4c).
// A write's statements grow with what it writes (a line, a period, an
// item), never with the number of stores: `user-orders#await-payment` writes
// the cart's two lines, `hours#set` a week's seven periods, `menus#change`
// the menu's items. `places#list` is the fixed table, no statement.
const PINNED: &[(&str, u64)] = &[
    ("store:data/addresses#add", 4),
    ("store:data/addresses#choose", 3),
    ("store:data/addresses#list", 1),
    ("store:data/addresses#remove", 2),
    ("store:data/addresses#rename", 3),
    ("store:data/carts#add", 4),
    ("store:data/carts#clear", 2),
    ("store:data/carts#current", 1),
    ("store:data/carts#decrease", 2),
    ("store:data/carts#remove", 2),
    ("store:data/checkout#chosen", 1),
    ("store:data/checkout#parts", 2),
    ("store:data/checkout#set-promo", 2),
    // Track `scheduled-orders`: the time chosen, read and written as the
    // tip is.
    ("store:data/checkout#set-time", 2),
    ("store:data/checkout#set-tip", 2),
    ("store:data/coverage#for-store", 2),
    ("store:data/coverage#reaches", 2),
    ("store:data/estimates#current", 1),
    ("store:data/fees#for-stores", 1),
    ("store:data/hours#for-cart", 6),
    ("store:data/hours#for-store", 3),
    ("store:data/hours#for-stores", 3),
    ("store:data/kitchen#prep-minutes", 1),
    ("store:data/menus#for-store", 2),
    ("store:data/menus#in-category", 2),
    ("store:data/menus#is-available", 1),
    ("store:data/menus#sections", 2),
    ("store:data/notices#current", 1),
    ("store:data/orders#current", 1),
    ("store:data/orders#place", 1),
    ("store:data/orders#placed", 1),
    ("store:data/places#list", 0),
    ("store:data/promos#find", 1),
    // TRACK SEAM (reviews): each one statement, `#rate` its order read again
    // and its row written.
    ("store:data/ratings#for-order", 1),
    ("store:data/ratings#for-stores", 1),
    ("store:data/ratings#rate", 2),
    ("store:data/ratings#reviews", 1),
    ("store:data/recommendations#for-store", 3),
    ("store:data/search#find", 2),
    ("store:data/search#find-for", 4),
    ("store:data/stores#get", 1),
    ("store:data/stores#list", 1),
    ("store:data/user-carts#add", 4),
    ("store:data/user-carts#clear", 2),
    ("store:data/user-carts#current", 1),
    ("store:data/user-carts#decrease", 4),
    ("store:data/user-carts#remove", 3),
    ("store:data/user-estimates#for-store", 3),
    ("store:data/user-estimates#for-stores", 3),
    ("store:data/user-orders#await-payment", 14),
    ("store:data/user-orders#awaited", 2),
    ("store:data/user-orders#charged", 1),
    ("store:data/user-orders#current", 1),
    ("store:data/user-orders#paying", 2),
    ("store:data/user-orders#place", 11),
    ("store:data/user-orders#placed", 1),
    // Track `scheduled-orders`: the reader's order read, where it is not
    // there (this reader has none); with an order, its lines and its time
    // too, and its status written.
    ("store:data/user-orders#send", 1),
    ("store:data/user-orders#settle", 2),
    ("store:host/carts#join", 1),
    ("store:host/estimates#set", 1),
    ("store:host/fees#set", 1),
    ("store:host/hours#set", 13),
    ("store:host/kitchen#set-prep", 1),
    ("store:host/menus#change", 6),
    ("store:host/menus#replace", 1),
    ("store:host/menus#stock", 1),
    ("store:host/notices#post", 1),
    ("store:host/orders#set-status", 2),
    ("store:host/promos#set", 1),
    ("store:host/recommendations#curate", 1),
];

/// **What a page or a command ran**, each in turn, on a store served on
/// PostgreSQL, with fifty stores more where `more` says: the reader adds a
/// line from each seeded store, saves an address, chooses a tip, and reads
/// every page; then places the order.
fn steps(more: bool) -> Vec<(String, u64)> {
    let s = served_from_patches_in("examples", |app| app.to_string(), &[]);
    if more {
        s.data
            .execute_for_test(FIFTY_MORE)
            .expect("fifty stores more");
    }
    let mut out = Vec::new();
    let mut step = |name: &str, run: &dyn Fn()| {
        let before = s.data.statements().expect("a layer that counts");
        run();
        out.push((
            name.to_string(),
            s.data.statements().expect("counted") - before,
        ));
    };
    step("add_to_cart (47)", &|| {
        s.command(
            "store.page.add_to_cart",
            "a",
            &add_shown("espresso", 1),
            false,
        )
        .expect("added")
    });
    step("add_to_cart (48)", &|| {
        s.command("store.page.add_to_cart", "a", &add_shown("drip", 1), false)
            .expect("added")
    });
    step("add_address", &|| {
        s.command(
            "store.page.add_address",
            "a",
            &[s_val("Home"), s_val(crate::places::PLACES[0].id)],
            false,
        )
        .expect("saved")
    });
    step("choose_tip", &|| {
        s.command(
            "store.page.choose_tip",
            "a",
            &[Val::Variant("fifteen".into(), None)],
            false,
        )
        .expect("chosen")
    });
    // Track `scheduled-orders`: as soon as possible chosen, which this
    // scenario's order is for; and the page of times read below.
    step("choose_time (as soon as possible)", &|| {
        s.command("store.page.choose_time", "a", &[Val::Option(None)], false)
            .expect("chosen")
    });
    for path in [
        "/",
        "/stores/47",
        "/cart",
        "/search?q=coffee",
        "/stores/47/address",
        "/cart/when",
    ] {
        step(path, &|| {
            fetched_as(&s, path, Some("a"));
        });
    }
    // `place_order`, pinned apart (the integrator's ruling on NEXT 4b): its
    // transaction's operations, its commit (what it hands the outbox, its
    // interaction's record, the outbox read back), and the session's
    // queries read again after its commit.
    let total = s.shown_total("a");
    let (txns, all, by_op) = (
        s.data.statements_by_transaction().len(),
        s.data.statements().expect("counted"),
        s.data.statements_by_op(),
    );
    s.command_answered(
        "store.page.place_order",
        "a",
        std::slice::from_ref(&total),
        None,
    )
    .expect("answered");
    let ran = s.data.statements_by_transaction()[txns..].to_vec();
    let after = s.data.statements().expect("counted") - all;
    // One transaction: this server takes no payments, so `place_order`
    // places the order at once, `user-orders#settle` in its own (ADR-0332).
    let [own] = ran[..] else {
        panic!("place_order's one transaction: {ran:?}")
    };
    let mut in_ops = 0;
    for (op, (calls, statements)) in s.data.statements_by_op() {
        let (c0, r0) = by_op.get(&op).copied().unwrap_or_default();
        if calls > c0 {
            // What its statements are made of, by operation (NEXT 4c's
            // baseline).
            eprintln!(
                "place_order: {op}: {} calls, {} statements",
                calls - c0,
                statements - r0
            );
            if op.starts_with("command ") {
                in_ops += statements - r0;
            }
        }
    }
    out.push((
        "place_order, taking no payments: its operations".to_string(),
        in_ops,
    ));
    out.push((
        "place_order, taking no payments: its commit".to_string(),
        own - in_ops,
    ));
    out.push((
        "place_order, taking no payments: its queries read again".to_string(),
        after - own,
    ));
    let mut step = |name: &str, run: &dyn Fn()| {
        let before = s.data.statements().expect("a layer that counts");
        run();
        out.push((
            name.to_string(),
            s.data.statements().expect("counted") - before,
        ));
    };
    step("/order", &|| {
        fetched_as(&s, "/order", Some("a"));
    });
    // TRACK SEAM (reviews): the order delivered by the kitchen's route, no
    // reader's step; then store 47 rated, and its rate page read.
    let delivered = posted(&s, "/bench/order?status=delivered", "a", "", "");
    assert!(delivered.starts_with("HTTP/1.1 200"), "{delivered}");
    // `rate_order` apart, as `place_order` is: its transaction (its
    // operations and its commit), and the reader's open pages read again
    // after it, which its cart's event regenerates (ADR-0218's session
    // entry): every page the scenario left open.
    let (txns, all) = (
        s.data.statements_by_transaction().len(),
        s.data.statements().expect("counted"),
    );
    let rated = s
        .command_answered(
            "store.rate.rate_order",
            "a",
            &[s_val("47"), Val::S64(4), s_val("Fine.")],
            None,
        )
        .expect("answered");
    assert!(rated.committed, "{rated:?}");
    let ran = s.data.statements_by_transaction()[txns..].to_vec();
    let after = s.data.statements().expect("counted") - all;
    let [own] = ran[..] else {
        panic!("rate_order's one transaction: {ran:?}")
    };
    out.push(("rate_order: its transaction".to_string(), own));
    out.push((
        "rate_order: its reader's open pages read again".to_string(),
        after - own,
    ));
    let mut step = |name: &str, run: &dyn Fn()| {
        let before = s.data.statements().expect("a layer that counts");
        run();
        out.push((
            name.to_string(),
            s.data.statements().expect("counted") - before,
        ));
    };
    step("/order/rate/47", &|| {
        fetched_as(&s, "/order/rate/47", Some("a"));
    });
    out
}

fn s_val(v: &str) -> Val {
    Val::String(v.to_string())
}

/// **Every page and command of the store runs as many statements with fifty
/// stores more as at the seed**, and as many as pinned: what a reader's
/// visit costs the database does not grow with the stores it does not read.
#[test]
fn each_page_and_command_is_a_bounded_number_of_statements() {
    if std::env::var("PW_STORE_TEST_LAYER").as_deref() != Ok("postgres") {
        eprintln!("skipped: the store is not served on PostgreSQL");
        return;
    }
    let seed = steps(false);
    let more = steps(true);
    eprintln!("statements per page and command:\n  {seed:?}\n  {more:?}");
    assert_eq!(seed, more, "equal at both sizes");
    let pinned: Vec<(String, u64)> = PAGES.iter().map(|(k, v)| (k.to_string(), *v)).collect();
    assert_eq!(seed, pinned, "as pinned");
}

/// `choose_tip` emitting one event more (`UserOrderChanged`), for the
/// control below.
fn one_more_event(app: &str) -> String {
    let at = app.find("command choose_tip(").expect("choose_tip");
    let emits = "    emits         UserCartChanged(current_user())\n";
    let found = at + app[at..].find(emits).expect("its emits");
    format!(
        "{}    emits         UserCartChanged(current_user()), UserOrderChanged(current_user())\n{}",
        &app[..found],
        &app[found + emits.len()..]
    )
}

/// `choose_tip`'s transaction's statements, on a store built with `change`.
fn choose_tip_counted(change: fn(&str) -> String) -> u64 {
    let s = served_from_patches_in("examples", change, &[]);
    s.command(
        "store.page.choose_tip",
        "a",
        &[Val::Variant("fifteen".into(), None)],
        false,
    )
    .expect("chosen");
    *s.data
        .statements_by_transaction()
        .last()
        .expect("its transaction")
}

/// **A command's commit is counted with it** (the integrator's ruling on
/// NEXT 4b): what it hands the outbox is a statement each, so the same
/// command emitting one event more makes one statement more.
#[test]
fn a_commands_commit_is_counted_with_it() {
    if std::env::var("PW_STORE_TEST_LAYER").as_deref() != Ok("postgres") {
        eprintln!("skipped: the store is not served on PostgreSQL");
        return;
    }
    let plain = choose_tip_counted(|app| app.to_string());
    let more = choose_tip_counted(one_more_event);
    assert_eq!(more, plain + 1, "one event more, one statement more");
}

/// **What each page and command runs**, on PostgreSQL, in the order
/// [`steps`] runs them.
// A command's count is its transaction's, its commit's statements with it
// (what it hands the outbox, its interaction's record, the outbox read
// back). `place_order`'s three apart, where the deployment takes no
// payments: `user-orders#settle` (7) is among its operations, the order
// placed at once (ADR-0332).
const PAGES: &[(&str, u64)] = &[
    ("add_to_cart (47)", 10),
    ("add_to_cart (48)", 11),
    ("add_address", 6),
    ("choose_tip", 6),
    ("choose_time (as soon as possible)", 6),
    // TRACK SEAM (reviews): each list's ratings one read more, the store's
    // page two (its ratings and its reviews), the order page one.
    ("/", 6),
    ("/stores/47", 18),
    ("/cart", 16),
    ("/search?q=coffee", 9),
    ("/stores/47/address", 2),
    // Track `scheduled-orders`: the page of times, its cart's stores' hours
    // (6), the time chosen (1), and its layout's cart (1); and the windows
    // wait for all stores' estimates in one set read (ADR-0348): the
    // address, store rows and estimate, 3 total regardless of key count.
    // cart's own (16) asks none: a cart for as soon as possible has no lead.
    ("/cart/when", 11),
    // Track `scheduled-orders`: 33 and 2 more, the time chosen
    // (`checkout#chosen`) and, where the order is settled, its time
    // (`user-orders#settle` 8, was 7).
    ("place_order, taking no payments: its operations", 35),
    ("place_order, taking no payments: its commit", 7),
    // Track `scheduled-orders`: 21 and 11 more, the checkout's stores'
    // hours (3), and the open page of times read again: its time chosen
    // (1), its stores' hours (6) and its layout's cart (1).
    (
        "place_order, taking no payments: its queries read again",
        32,
    ),
    ("/order", 6),
    // TRACK SEAM (reviews): `rate_order`'s transaction (`MayRate`'s read,
    // `#rate`'s read and its row, and its commit); the reader's open pages
    // read again after it, including scheduling's extra cart-hours read
    // (3) and the times page with an empty cart (8): 42 + 11 = 53. The rate
    // page reads its store and the layout's cart separately.
    ("rate_order: its transaction", 8),
    ("rate_order: its reader's open pages read again", 53),
    ("/order/rate/47", 2),
];

/// **The transactions so far, once none is still running**: the provider
/// tells the host of a cancel as it answers it, on a connection of its own,
/// so a `settle_payment` may still be counting when the command returns.
fn quiet(s: &Server) -> Vec<u64> {
    let mut last = s.data.statements_by_transaction();
    loop {
        std::thread::sleep(std::time::Duration::from_millis(300));
        let now = s.data.statements_by_transaction();
        if now == last {
            return now;
        }
        last = now;
    }
}

/// **Scenario (b), a deployment that takes payments** (ADR-0332, the
/// production path): `place_order` leaves the order awaiting its payment,
/// no settle among its operations; the provider's word, a card paid on its
/// page, runs `settle_payment` in a transaction of its own. Each
/// transaction's statements, with fifty stores more where `more` says.
fn awaiting(more: bool) -> Vec<(String, u64)> {
    let mut out = Vec::new();
    super::store_payments::paying(|s, _| {
        if more {
            s.data
                .execute_for_test(FIFTY_MORE)
                .expect("fifty stores more");
        }
        s.command(
            "store.page.add_to_cart",
            "a",
            &add_shown("espresso", 1),
            false,
        )
        .expect("added");
        s.command("store.page.add_to_cart", "a", &add_shown("drip", 1), false)
            .expect("added");
        let total = s.shown_total("a");
        let before = s.data.statements_by_transaction().len();
        s.command_answered(
            "store.page.place_order",
            "a",
            std::slice::from_ref(&total),
            // A payment is begun by an interaction (ADR-0332).
            Some("place-1"),
        )
        .expect("answered");
        let Some((status, Some(page))) = super::store_payments::status_of(s, "a") else {
            panic!("an order awaiting its payment")
        };
        assert_eq!(status, "awaiting-payment");
        let placed = s.data.statements_by_transaction()[before..].to_vec();
        let paid = super::store_payments::pay(&page, "4242424242424242");
        assert!(paid.contains("Paid."), "{paid}");
        let settled = s.data.statements_by_transaction()[before + placed.len()..].to_vec();
        let ([own], [settle]) = (&placed[..], &settled[..]) else {
            panic!("place_order's transaction, then settle_payment's: {placed:?} {settled:?}")
        };
        out.push((
            "place_order, taking payments: its transaction".to_string(),
            *own,
        ));
        out.push((
            "settle_payment, on the provider's word: its transaction".to_string(),
            *settle,
        ));
        // An order awaiting, taken back by its reader: `cancel_order`'s
        // transaction (ADR-0332's step eight).
        s.command(
            "store.page.add_to_cart",
            "a",
            &add_shown("espresso", 1),
            false,
        )
        .expect("added");
        let total = s.shown_total("a");
        s.command_answered(
            "store.page.place_order",
            "a",
            std::slice::from_ref(&total),
            Some("place-2"),
        )
        .expect("answered");
        let before = quiet(s).len();
        let canceled = s
            .command_answered("store.page.cancel_order", "a", &[], Some("cancel-1"))
            .expect("answered");
        assert!(canceled.committed, "{:?}", canceled.result);
        let ran = quiet(s)[before..].to_vec();
        // Its own, and `settle_payment` on the provider's word of the
        // cancel, which the provider tells the host as it answers the
        // cancel (ADR-0332's step eight).
        let [cancel, told] = ran[..] else {
            panic!("cancel_order's transaction, then settle_payment's: {ran:?}")
        };
        out.push(("cancel_order: its transaction".to_string(), cancel));
        out.push((
            "settle_payment, on the provider's word of the cancel: its transaction".to_string(),
            told,
        ));
        // An order no one pays, swept thirty minutes on (ADR-0334): the clock
        // moved, then the run that is due, `expire_order`'s transaction.
        let total = s.shown_total("a");
        s.command_answered(
            "store.page.place_order",
            "a",
            std::slice::from_ref(&total),
            Some("place-3"),
        )
        .expect("answered");
        let before = quiet(s).len();
        s.query_clock.0.advance(30 * 60 * 1000);
        assert_eq!(s.run_due().0, 1, "the sweep, run");
        let ran = quiet(s)[before..].to_vec();
        // Its own, and `settle_payment` on the provider's word of the cancel
        // the sweep asks of it.
        let [expire, told] = ran[..] else {
            panic!("expire_order's transaction, then settle_payment's: {ran:?}")
        };
        out.push((
            "expire_order, run thirty minutes on: its transaction".to_string(),
            expire,
        ));
        out.push((
            "settle_payment, on the provider's word of the sweep's cancel: its transaction"
                .to_string(),
            told,
        ));
        // Track `scheduled-orders`: the swept order's lines, back in the
        // cart, ordered for a time, a quarter hour past the first offered;
        // paid on the provider's page; and sent to its stores when its
        // preparation is due, the window less the estimate's most (35
        // minutes, no address chosen) and five more.
        let window = ((s.now() + 3_600_000) / 900_000 + 2) * 900_000;
        let before = quiet(s).len();
        let chosen = s
            .command_answered(
                "store.page.choose_time",
                "a",
                &[Val::Option(Some(Box::new(Val::S64(window))))],
                Some("time-1"),
            )
            .expect("answered");
        assert!(chosen.committed, "{:?}", chosen.result);
        let total = s.shown_total("a");
        s.command_answered(
            "store.page.place_order",
            "a",
            std::slice::from_ref(&total),
            Some("place-4"),
        )
        .expect("answered");
        let ran = quiet(s)[before..].to_vec();
        let [choose, place] = ran[..] else {
            panic!("choose_time's transaction, then place_order's: {ran:?}")
        };
        out.push((
            "choose_time, for a time: its transaction".to_string(),
            choose,
        ));
        out.push((
            "place_order, for a time, taking payments: its transaction".to_string(),
            place,
        ));
        let Some((status, Some(page))) = super::store_payments::status_of(s, "a") else {
            panic!("an order awaiting its payment")
        };
        assert_eq!(status, "awaiting-payment");
        let before = quiet(s).len();
        let paid = super::store_payments::pay(&page, "4242424242424242");
        assert!(paid.contains("Paid."), "{paid}");
        let ran = quiet(s)[before..].to_vec();
        let [settle] = ran[..] else {
            panic!("settle_payment's transaction: {ran:?}")
        };
        out.push((
            "settle_payment, on the provider's word, for a time: its transaction".to_string(),
            settle,
        ));
        let sends = window - 40 * 60_000;
        s.query_clock
            .0
            .advance(u64::try_from(sends - s.now()).expect("ahead") + 1000);
        let before = quiet(s).len();
        assert_eq!(s.run_due().0, 1, "the send, run");
        let ran = quiet(s)[before..].to_vec();
        let [send] = ran[..] else {
            panic!("send_to_store's transaction: {ran:?}")
        };
        out.push((
            "send_to_store, run when its preparation is due: its transaction".to_string(),
            send,
        ));
    });
    out
}

/// **Where the deployment takes payments, `place_order` and the
/// `settle_payment` the provider's word runs are each a bounded number of
/// statements**, equal at the seed and with fifty stores more, and pinned.
#[test]
fn placing_and_settling_where_payments_are_taken_is_bounded() {
    if std::env::var("PW_STORE_TEST_LAYER").as_deref() != Ok("postgres") {
        eprintln!("skipped: the store is not served on PostgreSQL");
        return;
    }
    let seed = awaiting(false);
    let more = awaiting(true);
    eprintln!("statements where payments are taken:\n  {seed:?}\n  {more:?}");
    assert_eq!(seed, more, "equal at both sizes");
    let pinned: Vec<(String, u64)> = AWAITING.iter().map(|(k, v)| (k.to_string(), *v)).collect();
    assert_eq!(seed, pinned, "as pinned");
}

/// Scenario (b)'s transactions, each with its commit.
// Since ADR-0334 a command's commit writes its scheduled runs too: a delete
// for each, and an insert for each run due. `place_order` schedules its
// sweep (2), `settle_payment` and `cancel_order` cancel it (1).
// Track `scheduled-orders`: `place_order` reads the time chosen (1 more),
// and `settle_payment` the order's time where it settles one paid (1 more).
const AWAITING: &[(&str, u64)] = &[
    ("place_order, taking payments: its transaction", 38),
    (
        "settle_payment, on the provider's word: its transaction",
        18,
    ),
    ("cancel_order: its transaction", 25),
    (
        "settle_payment, on the provider's word of the cancel: its transaction",
        14,
    ),
    ("expire_order, run thirty minutes on: its transaction", 26),
    (
        "settle_payment, on the provider's word of the sweep's cancel: its transaction",
        12,
    ),
    // Track `scheduled-orders`: an order for a time. Its time chosen, held
    // to the windows its cart's stores take an order at when they are told:
    // their hours and the cart's one store's estimate (2: no address is
    // chosen here, so none to read; 3 where one is); placed, the estimate
    // read and the time written with it; paid, its send scheduled in the
    // commit (a delete and an insert); and sent when its preparation is due.
    ("choose_time, for a time: its transaction", 13),
    (
        "place_order, for a time, taking payments: its transaction",
        40,
    ),
    (
        "settle_payment, on the provider's word, for a time: its transaction",
        21,
    ),
    (
        "send_to_store, run when its preparation is due: its transaction",
        9,
    ),
];

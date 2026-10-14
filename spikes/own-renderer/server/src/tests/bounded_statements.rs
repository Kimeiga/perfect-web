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
}

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
        "data/user-orders#await-payment" | "data/user-orders#place" => {
            vec![s(ADA), charged, Val::Option(None)]
        }
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
    ("store:data/user-orders#await-payment", 14),
    ("store:data/user-orders#awaited", 2),
    ("store:data/user-orders#charged", 1),
    ("store:data/user-orders#current", 1),
    ("store:data/user-orders#paying", 2),
    ("store:data/user-orders#place", 11),
    ("store:data/user-orders#placed", 1),
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
    for path in [
        "/",
        "/stores/47",
        "/cart",
        "/search?q=coffee",
        "/stores/47/address",
    ] {
        step(path, &|| {
            fetched_as(&s, path, Some("a"));
        });
    }
    let total = s.shown_total("a");
    let before = s.data.statements_by_op();
    step("place_order", &|| {
        s.command_answered(
            "store.page.place_order",
            "a",
            std::slice::from_ref(&total),
            None,
        )
        .expect("answered");
    });
    // What `place_order`'s statements are made of, by operation (NEXT 4c's
    // measured baseline): each operation's calls and statements in it.
    let after = s.data.statements_by_op();
    for (op, (calls, ran)) in &after {
        let (c0, r0) = before.get(op).copied().unwrap_or_default();
        if *calls > c0 {
            eprintln!(
                "place_order: {op}: {} calls, {} statements",
                calls - c0,
                ran - r0
            );
        }
    }
    step("/order", &|| {
        fetched_as(&s, "/order", Some("a"));
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

/// **What each page and command runs**, on PostgreSQL, in the order
/// [`steps`] runs them.
const PAGES: &[(&str, u64)] = &[
    ("add_to_cart (47)", 7),
    ("add_to_cart (48)", 8),
    ("add_address", 3),
    ("choose_tip", 3),
    ("/", 5),
    ("/stores/47", 16),
    ("/cart", 13),
    ("/search?q=coffee", 8),
    ("/stores/47/address", 2),
    ("place_order", 54),
    ("/order", 5),
];

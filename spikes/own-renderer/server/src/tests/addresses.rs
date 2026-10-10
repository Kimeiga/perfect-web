//! **Delivery addresses** (track `store-accounts`, ADR-XXXX, milestone 2): a
//! reader's saved addresses, added, renamed, removed and chosen, a user's or
//! a guest's session's guest's; whether a store reaches the chosen one, by
//! the one rule (`places::delivers_to`) its page, its estimate, `add_to_cart`
//! and `place_order` all read; and the estimate gaining the courier's minutes.
//!
//! Each runs on the layer the server's tests serve the store on: in memory,
//! or on PostgreSQL where `PW_STORE_TEST_LAYER=postgres` names one.

use super::sign_in::{REDIRECT, TestProvider, signed_in};
use super::*;
use crate::places;

const ADD_ADDRESS: &str = "store.page.add_address";
const RENAME_ADDRESS: &str = "store.page.rename_address";
const REMOVE_ADDRESS: &str = "store.page.remove_address";
const CHOOSE_ADDRESS: &str = "store.page.choose_address";
const PLACE_ORDER: &str = "store.page.place_order";

fn store_with(provider: Arc<TestProvider>) -> Served {
    let s = served_from_patches_in("examples", |app| app.to_string(), &[]);
    s.identity.use_provider(provider, REDIRECT);
    s
}

fn text(v: &str) -> Val {
    Val::String(v.to_string())
}

/// A command's answer, committed or its declared error's case.
fn run(s: &Server, command: &str, session: &str, args: &[Val]) -> Result<(), String> {
    let answered = s
        .command_answered(command, session, args, None)
        .unwrap_or_else(|e| panic!("{command}: {e}"));
    if answered.committed {
        return Ok(());
    }
    Err(answered.result.map(|r| r.to_string()).unwrap_or_default())
}

/// The reader's addresses, as the layer answers them: (id, label, place
/// name, chosen).
fn addresses(s: &Server, session: &str) -> Vec<(String, String, String, bool)> {
    let owner = s.owner(session);
    match s.layer_read(session, "store:data/addresses#list", &owner) {
        Val::List(rows) => rows
            .iter()
            .map(|r| {
                let get = |k: &str| match val_at(r, &[k]) {
                    Some(Val::String(v)) => v.clone(),
                    other => panic!("{k}: {other:?}"),
                };
                let chosen = matches!(val_at(r, &["chosen"]), Some(Val::Bool(true)));
                (get("id"), get("label"), get("place"), chosen)
            })
            .collect(),
        other => panic!("addresses#list answered {other:?}"),
    }
}

/// One of the layer's operations that takes a store and the reader.
fn of_store(s: &Server, session: &str, op: &str, store: &str) -> Vec<Val> {
    // A command's operations, its staging held while they run (on
    // PostgreSQL, its transaction), and given up uncommitted.
    let staging = s.data.begin(session);
    let ops = staging.ops();
    let run = ops.get(op).unwrap_or_else(|| panic!("no {op}"));
    run(&[text(store), Val::String(s.owner(session))]).expect(op)
}

/// The store's page's delivery slot, as the session is sent it: its
/// estimate, once streamed.
fn delivery(s: &Server, session: &str, store: &str) -> String {
    let whole: String = fetched_as(s, &format!("/stores/{store}"), Some(session))
        .into_iter()
        .map(|(_, c)| c)
        .collect();
    slot(&whole, "Delivery")
}

/// What the store's page shows the session, as text.
fn store_page(s: &Server, session: &str, store: &str) -> String {
    let (html, ..) = s
        .serve_document_settled(session, "store.page.StorePage", &store_params(store), &[])
        .expect("served");
    visible(&html)
}

/// **A reader's addresses are added, renamed, removed and chosen, and are
/// theirs**: each saved address is chosen as it is saved; a place the table
/// does not hold is refused; removing the chosen one leaves none chosen;
/// another user's are their own, and a guest's are its session's guest's.
#[test]
fn a_readers_addresses_are_theirs_to_add_rename_remove_and_choose() {
    let provider = Arc::new(TestProvider::default());
    let s = store_with(provider.clone());
    let ada = signed_in(&s, &provider, "came-ada", "ada");
    let ben = signed_in(&s, &provider, "came-ben", "ben");
    run(&s, ADD_ADDRESS, &ada, &[text("Home"), text("dolores-park")]).expect("saved");
    run(
        &s,
        ADD_ADDRESS,
        &ada,
        &[text("Work"), text("ferry-building")],
    )
    .expect("saved");
    assert_eq!(
        addresses(&s, &ada),
        [
            (
                "addr-1".into(),
                "Home".into(),
                "Dolores Park, San Francisco".into(),
                false
            ),
            (
                "addr-2".into(),
                "Work".into(),
                "Ferry Building, San Francisco".into(),
                true
            ),
        ],
        "the last saved is chosen"
    );
    run(&s, CHOOSE_ADDRESS, &ada, &[text("addr-1")]).expect("chosen");
    run(&s, RENAME_ADDRESS, &ada, &[text("addr-2"), text("Office")]).expect("renamed");
    assert_eq!(
        addresses(&s, &ada)
            .iter()
            .map(|a| (a.1.as_str(), a.3))
            .collect::<Vec<_>>(),
        [("Home", true), ("Office", false)]
    );
    // A place the table does not hold.
    let refused = run(&s, ADD_ADDRESS, &ada, &[text("Moon"), text("the-moon")]);
    assert!(refused.unwrap_err().contains("unknown-place"));
    // The chosen one removed, none is chosen.
    run(&s, REMOVE_ADDRESS, &ada, &[text("addr-1")]).expect("removed");
    assert_eq!(addresses(&s, &ada).len(), 1);
    assert!(addresses(&s, &ada).iter().all(|a| !a.3), "none chosen");
    // An address that is not there.
    let refused = run(&s, CHOOSE_ADDRESS, &ada, &[text("addr-9")]);
    assert!(refused.unwrap_err().contains("no-such-address"));
    // Another user's, and a guest's.
    assert!(addresses(&s, &ben).is_empty(), "ben's are his");
    run(
        &s,
        ADD_ADDRESS,
        "a-guest",
        &[text("Hotel"), text("union-square")],
    )
    .expect("saved");
    assert_eq!(
        addresses(&s, "a-guest").len(),
        1,
        "a guest's are its session's guest's"
    );
    assert_eq!(addresses(&s, &ada).len(), 1);
    // A label past forty code points is refused at the boundary, before the
    // command runs (ADR-0225), as the browser's request is decoded.
    let long = "x".repeat(41);
    let refused = s.command_answer(
        ADD_ADDRESS,
        &ada,
        &[serde_json::json!(long), serde_json::json!("chinatown")],
        Some("i-long"),
    );
    assert!(refused.is_err(), "{refused:?}");
    assert_eq!(addresses(&s, &ada).len(), 1, "nothing saved");
}

/// **At most ten addresses** (`store::MOST_ADDRESSES`).
#[test]
fn a_reader_keeps_at_most_ten_addresses() {
    let s = served_from_patches_in("examples", |app| app.to_string(), &[]);
    for n in 0..store::MOST_ADDRESSES {
        run(
            &s,
            ADD_ADDRESS,
            "g",
            &[text(&format!("A{n}")), text("chinatown")],
        )
        .expect("saved");
    }
    let refused = run(&s, ADD_ADDRESS, "g", &[text("One more"), text("chinatown")]);
    assert!(refused.unwrap_err().contains("too-many"));
    assert_eq!(addresses(&s, "g").len(), store::MOST_ADDRESSES);
}

/// **One rule, everywhere it is read**: for each store and each place in the
/// table, the page's reach, the estimate's `no-coverage` and the command's
/// check all answer as `places::delivers_to` does, on this layer.
#[test]
fn the_page_the_estimate_and_the_commands_read_one_rule() {
    let s = served_from_patches_in("examples", |app| app.to_string(), &[]);
    for (n, place) in places::PLACES.iter().enumerate() {
        run(
            &s,
            ADD_ADDRESS,
            "g",
            &[text(&format!("P{n}")), text(place.id)],
        )
        .ok();
        if n >= store::MOST_ADDRESSES - 1 {
            // Make room for the next.
            let oldest = addresses(&s, "g")[0].0.clone();
            run(&s, REMOVE_ADDRESS, "g", &[text(&oldest)]).expect("removed");
        }
        let chosen = addresses(&s, "g");
        assert!(
            chosen.iter().any(|a| a.3 && a.2 == place.name),
            "{} chosen: {chosen:?}",
            place.id
        );
        for store in [STORE_ID, SECOND_STORE.0] {
            let rule = places::delivers_to(&store::zone_of(store), place);
            let reach = of_store(&s, "g", "store:data/coverage#for-store", store);
            assert!(
                matches!(val_at(&reach[0], &["delivers"]), Some(Val::Bool(b)) if *b == rule),
                "{store} {}: the page's {reach:?}",
                place.id
            );
            let reaches = of_store(&s, "g", "store:data/coverage#reaches", store);
            assert_eq!(
                reaches,
                [Val::Bool(rule)],
                "{store} {}: the command's",
                place.id
            );
            let estimate = of_store(&s, "g", "store:data/user-estimates#for-store", store);
            let refused = matches!(
                &estimate[..],
                [Val::Result(Err(Some(e)))] if matches!(e.as_ref(), Val::Variant(c, _) if c == "no-coverage")
            );
            assert_eq!(
                refused, !rule,
                "{store} {}: the estimate's {estimate:?}",
                place.id
            );
        }
    }
}

/// **A store's estimate gains the courier's minutes to the chosen address**,
/// as served on its page; with no address chosen, the kitchen's alone.
#[test]
fn the_estimate_gains_the_travel_to_the_chosen_address() {
    let s = served_from_patches_in("examples", |app| app.to_string(), &[]);
    let base = store::DEFAULT_ESTIMATE.0;
    assert_eq!(
        delivery(&s, "g", STORE_ID),
        format!("Delivery in {base} to {} min", base + 10)
    );
    let page = store_page(&s, "g", STORE_ID);
    assert!(page.contains("No delivery address yet"), "{page}");
    run(&s, ADD_ADDRESS, "g", &[text("Home"), text("dolores-park")]).expect("saved");
    let travel = places::travel_minutes(
        &store::zone_of(STORE_ID),
        places::place("dolores-park").expect("a place"),
    );
    assert!(travel > 1, "{travel}");
    assert_eq!(
        delivery(&s, "g", STORE_ID),
        format!(
            "Delivery in {} to {} min",
            base + travel,
            base + 10 + travel
        )
    );
    let page = store_page(&s, "g", STORE_ID);
    assert!(page.contains("Delivering to Home"), "{page}");
    assert!(!page.contains("doesn't deliver"), "{page}");
}

/// **A store that does not reach the chosen address says so where the menu
/// is, and refuses its Add**; and an order whose cart holds an item from it
/// is refused too, though the item was added before the address changed.
#[test]
fn a_store_out_of_reach_says_so_and_refuses_the_add_and_the_order() {
    let provider = Arc::new(TestProvider::default());
    let s = store_with(provider.clone());
    let ada = signed_in(&s, &provider, "came-ada", "ada");
    // Store 48 at the Ferry Building reaches the Ferry Building.
    run(
        &s,
        ADD_ADDRESS,
        &ada,
        &[text("Work"), text("ferry-building")],
    )
    .expect("saved");
    s.command(ADD, &ada, &add_shown("drip", 1), false)
        .expect("store 48 reaches the Ferry Building");
    // Dolores Park is out of its reach, and inside store 47's.
    run(&s, ADD_ADDRESS, &ada, &[text("Home"), text("dolores-park")]).expect("saved");
    let page = store_page(&s, &ada, SECOND_STORE.0);
    assert!(
        page.contains("Harbor Coffee doesn't deliver to Home."),
        "{page}"
    );
    let answered = s
        .command_answered(ADD, &ada, &add_shown("drip", 1), None)
        .expect("runs");
    assert!(!answered.committed);
    assert!(
        answered
            .result
            .unwrap_or_default()
            .to_string()
            .contains("out-of-range"),
        "the add refused by name"
    );
    assert_eq!(
        s.cart_lines(&ada),
        [("drip".to_string(), 1)],
        "nothing added"
    );
    // The cart, from before, holds store 48's drip: the order is refused.
    let refused = run(&s, PLACE_ORDER, &ada, &[]);
    assert!(refused.unwrap_err().contains("no-coverage"));
    assert_eq!(s.order_of(&ada), None, "no order placed");
    assert_eq!(s.cart_lines(&ada).len(), 1, "the cart is as it was");
    // Store 47 reaches Dolores Park: its own items add and order.
    assert!(!store_page(&s, &ada, STORE_ID).contains("doesn't deliver"));
    run(&s, "store.page.remove_from_cart", &ada, &[text("drip")]).expect("removed");
    s.command(ADD, &ada, &add_shown("espresso", 1), false)
        .expect("store 47 reaches Dolores Park");
    run(&s, PLACE_ORDER, &ada, &[]).expect("placed");
}

/// **A reader's addresses are told to each of their sessions, and no one
/// else's** (telling by principal): saving one in one session tells the
/// user's other session's store page, its estimate's travel and its reach,
/// and sends another user's page no frame.
#[test]
fn an_address_saved_reaches_the_users_other_sessions_and_no_one_elses() {
    let provider = Arc::new(TestProvider::default());
    let s = store_with(provider.clone());
    let ada = signed_in(&s, &provider, "came-ada", "ada");
    let ada_again = signed_in(&s, &provider, "came-ada-2", "ada");
    let ben = signed_in(&s, &provider, "came-ben", "ben");
    for session in [&ada, &ada_again, &ben] {
        store_page(&s, session, SECOND_STORE.0);
    }
    s.tell_waiting();
    let docs = {
        let pending = s.pending.lock().expect("pending");
        [&ada_again, &ben].map(|session| latest(&pending, session))
    };
    let frames = |doc: &Doc| s.pending.lock().expect("pending")[doc].frames.len();
    let before = docs.clone().map(|doc| frames(&doc));
    run(&s, ADD_ADDRESS, &ada, &[text("Home"), text("dolores-park")]).expect("saved");
    s.tell_waiting();
    let told: String = sets_of(&s, &docs[0])
        .iter()
        .map(|set| visible(&written(set)))
        .collect();
    assert!(
        told.contains("doesn't deliver to Home"),
        "ada's other session: {told}"
    );
    assert_eq!(frames(&docs[1]), before[1], "ben is sent no frame");
}

/// **The places an address may name are the fixed table's**, every one, on
/// this layer (each store's zone on it is held by the rule's test above).
#[test]
fn the_places_are_the_fixed_tables() {
    let s = served_from_patches_in("examples", |app| app.to_string(), &[]);
    let ops = s.data.reads("g", None);
    let list = ops.get("store:data/places#list").expect("places#list");
    match &list(&[]).expect("places")[..] {
        [Val::List(listed)] => {
            let ids: Vec<&str> = listed
                .iter()
                .map(|p| match val_at(p, &["id"]) {
                    Some(Val::String(id)) => id.as_str(),
                    other => panic!("{other:?}"),
                })
                .collect();
            let table: Vec<&str> = places::PLACES.iter().map(|p| p.id).collect();
            assert_eq!(ids, table);
        }
        other => panic!("places#list answered {other:?}"),
    }
}

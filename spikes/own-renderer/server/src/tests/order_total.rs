//! **An order's total** (track `order-total`, W9): each store's fees and
//! tax, a promo code and a tip, priced by the program (`checkout.pw`) from
//! the store's data, shown by the cart page and charged by `place_order`,
//! which prices it again in its own transaction. On the layer the server's
//! tests run on: memory, or PostgreSQL under `PW_STORE_TEST_LAYER=postgres`.
//! Each rule's test has its control: the case on the rule's other side.

use super::*;
use crate::checkout::{Fees, Off, Promo};

const ADD: &str = "store.page.add_to_cart";
const PLACE: &str = "store.page.place_order";
const TIP: &str = "store.page.choose_tip";
const APPLY: &str = "store.page.apply_promo";

fn served() -> Served {
    served_from_patches_in("examples", |app| app.to_string(), &[])
}

/// No fees and no tax: what a test changes one rule of.
fn bare() -> Fees {
    Fees {
        delivery_fee: 0,
        service_rate: 0,
        service_min: 0,
        service_max: 0,
        small_order_under: 0,
        small_order_fee: 0,
        tax_rate: 0,
        tax_delivery: false,
        tax_service: false,
    }
}

/// A store's fees set, with the `StoreChanged` that drops what read them.
fn set_fees(s: &Server, store: &str, fees: &Fees) {
    s.store_write(
        "",
        "store:host/fees#set",
        &[
            Val::String(store.to_string()),
            crate::checkout::fees_val(fees),
        ],
        &[(
            "Events.StoreChanged".to_string(),
            vec![Val::String(store.to_string())],
        )],
        |delivered| {
            s.invalidate_queries("", &[], delivered);
        },
    )
    .expect("set");
}

fn set_promo(s: &Server, promo: &Promo) {
    s.store_write(
        "",
        "store:host/promos#set",
        &[crate::checkout::promo_val(promo)],
        &[],
        |_| {},
    )
    .expect("set");
}

/// A code of store 47's, until an hour from now.
fn promo(s: &Server, code: &str, off: Off, by_store: bool) -> Promo {
    Promo {
        code: code.into(),
        store: STORE_ID.into(),
        off,
        min_subtotal: 0,
        expires: s.now() + 3_600_000,
        by_store,
    }
}

fn add(s: &Server, session: &str, item: &str, quantity: i64) {
    s.command(ADD, session, &add_shown(item, quantity), false)
        .expect("added");
}

/// The session's checkout, as the cart page reads it.
fn checkout(s: &Server, session: &str) -> Val {
    let owner = s.owner(session);
    s.query("store.page.Checkout", session, &[Val::String(owner)])
        .expect("the checkout")
}

/// One of the checkout's charges, in cents.
fn charge(priced: &Val, name: &str) -> i64 {
    match val_at(priced, &["charges", name]) {
        Some(Val::S64(n)) => *n,
        other => panic!("{name}: {other:?} in {priced:?}"),
    }
}

/// A command's answer, as the page's JSON has it.
fn answered(s: &Server, command: &str, session: &str, args: &[Val]) -> serde_json::Value {
    s.command_answered(command, session, args, None)
        .expect("answered")
        .result
        .expect("a result")
}

fn err(case: &str) -> serde_json::Value {
    serde_json::json!({ "$case": "err", "value": { "$case": case } })
}

/// **Texas's half cent is a cent** (Tax Code §151.053(a); 34 TAC §3.286):
/// 8.25% of $250.00 is $20.625, charged $20.63, once on the order, not on
/// each line; the control, $245.75 is $20.274375, charged $20.27.
#[test]
fn texas_half_a_cent_is_a_cent_once_on_the_order() {
    let s = served();
    set_fees(
        &s,
        STORE_ID,
        &Fees {
            tax_rate: 82_500,
            ..bare()
        },
    );
    // 50 × $3.50, 2 × $4.25 and 14 × $4.75: $250.00.
    add(&s, "a", "espresso", 50);
    add(&s, "a", "cortado", 2);
    add(&s, "a", "cold-brew", 14);
    let priced = checkout(&s, "a");
    assert_eq!(charge(&priced, "subtotal"), 25_000);
    assert_eq!(charge(&priced, "tax"), 2063, "$20.625, half up");
    assert_eq!(charge(&priced, "total"), 27_063);
    add(&s, "b", "espresso", 50);
    add(&s, "b", "cold-brew", 14);
    add(&s, "b", "cortado", 1);
    let control = checkout(&s, "b");
    assert_eq!(charge(&control, "subtotal"), 24_575);
    assert_eq!(charge(&control, "tax"), 2027, "$20.274375, under a half");
}

/// **New York City's 8.875%, on delivery where the schedule taxes it**: $7.00
/// of food and a $2.99 delivery fee taxed, $9.99 at 8.875% is $0.886...,
/// $0.89; the control, delivery not taxed, $7.00 is $0.62125, $0.62.
#[test]
fn new_york_citys_rate_on_delivery_where_it_is_taxed() {
    let s = served();
    let nyc = Fees {
        delivery_fee: 299,
        tax_rate: 88_750,
        tax_delivery: true,
        ..bare()
    };
    set_fees(&s, STORE_ID, &nyc);
    add(&s, "a", "espresso", 2);
    let priced = checkout(&s, "a");
    assert_eq!(charge(&priced, "tax"), 89, "on $9.99");
    assert_eq!(charge(&priced, "total"), 700 + 299 + 89);
    set_fees(
        &s,
        STORE_ID,
        &Fees {
            tax_delivery: false,
            ..nyc
        },
    );
    assert_eq!(charge(&checkout(&s, "a"), "tax"), 62, "on $7.00");
}

/// **The service fee, clamped**: 15%, at least $3.00 and at most $12.00. On
/// $7.00 it is $3.00; on $250.00, $12.00; the control, on $34.00 it is
/// $5.10. Taxed where the schedule taxes service, the small-order fee with
/// it; the control, not.
#[test]
fn the_service_fee_is_clamped_at_its_least_and_its_most() {
    let s = served();
    let fees = Fees {
        service_rate: 150_000,
        service_min: 300,
        service_max: 1200,
        small_order_under: 1000,
        small_order_fee: 250,
        tax_rate: 100_000,
        tax_service: true,
        ..bare()
    };
    set_fees(&s, STORE_ID, &fees);
    add(&s, "a", "espresso", 2);
    let least = checkout(&s, "a");
    assert_eq!(charge(&least, "service"), 300, "15% of $7.00 is $1.05");
    // 10% of $7.00, the $3.00 fee and the $2.50 small-order fee: $1.25.
    assert_eq!(charge(&least, "tax"), 125, "service taxed");
    add(&s, "b", "espresso", 50);
    add(&s, "b", "cortado", 2);
    add(&s, "b", "cold-brew", 14);
    assert_eq!(
        charge(&checkout(&s, "b"), "service"),
        1200,
        "15% of $250.00"
    );
    add(&s, "c", "cortado", 8);
    assert_eq!(charge(&checkout(&s, "c"), "service"), 510, "15% of $34.00");
    set_fees(
        &s,
        STORE_ID,
        &Fees {
            tax_service: false,
            ..fees
        },
    );
    assert_eq!(charge(&checkout(&s, "a"), "tax"), 70, "service not taxed");
}

/// **The small-order fee, under its threshold and not at it**: $2.50 under
/// $7.00. At $3.50 it is charged; the control, at $7.00 exactly, it is not.
#[test]
fn the_small_order_fee_is_charged_under_its_threshold_and_not_at_it() {
    let s = served();
    set_fees(
        &s,
        STORE_ID,
        &Fees {
            small_order_under: 700,
            small_order_fee: 250,
            ..bare()
        },
    );
    add(&s, "a", "espresso", 1);
    assert_eq!(charge(&checkout(&s, "a"), "small_order"), 250, "$3.50");
    add(&s, "b", "espresso", 2);
    assert_eq!(charge(&checkout(&s, "b"), "small_order"), 0, "$7.00");
}

/// **Each code that does not apply is refused by name**: unknown, expired,
/// below its least subtotal, used, and its store not in the cart; the
/// control, a code that applies is taken off, 15% of $3.50 half up, $0.53.
#[test]
fn each_promo_refusal_is_by_name() {
    let s = served();
    set_fees(&s, STORE_ID, &bare());
    let code = |c: &str| [Val::String(c.into())];
    add(&s, "a", "espresso", 1);
    assert_eq!(answered(&s, APPLY, "a", &code("NOPE")), err("unknown-code"));
    let mut late = promo(&s, "LATE", Off::Percent(150_000), false);
    late.expires = s.now() + 60_000;
    set_promo(&s, &late);
    s.query_clock.0.advance(120_000);
    assert_eq!(answered(&s, APPLY, "a", &code("LATE")), err("expired"));
    let mut big = promo(&s, "BIG", Off::Amount(500), false);
    big.min_subtotal = 2000;
    set_promo(&s, &big);
    assert_eq!(
        answered(&s, APPLY, "a", &code("BIG")),
        serde_json::json!({ "$case": "err", "value": { "$case": "below-minimum", "value": 2000 } })
    );
    let mut other = promo(&s, "HARBOR", Off::Amount(100), false);
    other.store = SECOND_STORE.0.into();
    set_promo(&s, &other);
    assert_eq!(
        answered(&s, APPLY, "a", &code("HARBOR")),
        err("not-in-cart")
    );
    set_promo(&s, &promo(&s, "FIFTEEN", Off::Percent(150_000), false));
    let applied = answered(&s, APPLY, "a", &code("FIFTEEN"));
    assert_eq!(applied["$case"], "ok", "{applied}");
    let priced = checkout(&s, "a");
    assert_eq!(charge(&priced, "discount"), 53, "15% of $3.50, half up");
    let placed = s
        .command_answered(PLACE, "a", &[s.shown_total("a")], None)
        .expect("answered");
    assert!(placed.committed, "{:?}", placed.result);
    add(&s, "a", "espresso", 1);
    assert_eq!(
        answered(&s, APPLY, "a", &code("FIFTEEN")),
        err("already-used")
    );
}

/// **A tip is never taxed, and never chosen for the reader**: none until
/// they choose; 20% of $7.00 is $1.40, the tax the same as without it; an
/// amount below nothing refused. Each pressed as a browser sends its case
/// (ADR-0331).
#[test]
fn a_tip_is_never_taxed_and_never_a_default() {
    let s = served();
    set_fees(
        &s,
        STORE_ID,
        &Fees {
            tax_rate: 86_250,
            ..bare()
        },
    );
    add(&s, "a", "espresso", 2);
    let none = checkout(&s, "a");
    assert_eq!(charge(&none, "tip"), 0, "no tip until one is chosen");
    // The case as a browser sends it (ADR-0331).
    let tip = format!("/command/{TIP}");
    let twenty = posted(&s, &tip, "a", "tip-1", r#"[{ "$case": "twenty" }]"#);
    assert!(twenty.contains("\"$case\":\"ok\""), "{twenty}");
    let tipped = checkout(&s, "a");
    assert_eq!(charge(&tipped, "tip"), 140);
    assert_eq!(
        charge(&tipped, "tax"),
        charge(&none, "tax"),
        "the tip untaxed"
    );
    assert_eq!(charge(&tipped, "total"), charge(&none, "total") + 140);
    let negative = posted(
        &s,
        &tip,
        "a",
        "tip-2",
        r#"[{ "$case": "tip-amount", "value": -100 }]"#,
    );
    assert!(negative.contains("\"negative-tip\""), "{negative}");
    assert_eq!(charge(&checkout(&s, "a"), "tip"), 140, "unchanged");
    let three = posted(
        &s,
        &tip,
        "a",
        "tip-3",
        r#"[{ "$case": "tip-amount", "value": 300 }]"#,
    );
    assert!(three.contains("\"$case\":\"ok\""), "{three}");
    assert_eq!(charge(&checkout(&s, "a"), "tip"), 300);
}

/// **A discount the store funds lowers what is taxed; one the platform
/// reimburses does not** (California's Regulation 1671.1, New York's
/// TB-ST-806): 10% off $19.00 at 8.625%. The store's: $17.10 taxed, $1.47.
/// The control, the platform's: $19.00 taxed, $1.64.
#[test]
fn a_store_funded_discount_lowers_the_tax_and_a_platform_funded_one_does_not() {
    let s = served();
    set_fees(
        &s,
        STORE_ID,
        &Fees {
            tax_rate: 86_250,
            ..bare()
        },
    );
    set_promo(&s, &promo(&s, "STORE10", Off::Percent(100_000), true));
    set_promo(&s, &promo(&s, "PLATFORM10", Off::Percent(100_000), false));
    for (session, code, tax) in [("a", "STORE10", 147), ("b", "PLATFORM10", 164)] {
        add(&s, session, "cold-brew", 4);
        let applied = answered(&s, APPLY, session, &[Val::String(code.into())]);
        assert_eq!(applied["$case"], "ok", "{applied}");
        let priced = checkout(&s, session);
        assert_eq!(charge(&priced, "discount"), 190, "{code}");
        assert_eq!(charge(&priced, "tax"), tax, "{code}");
    }
}

/// **The order is charged the total its page showed, as the page showed
/// it**: the order's charges, each store's part with the rate applied, are
/// the checkout's; the control, a total the page no longer shows is refused
/// with the total now.
#[test]
fn the_order_is_charged_the_total_its_page_showed() {
    let s = served();
    add(&s, "a", "espresso", 2);
    add(&s, "a", "drip", 1);
    let shown = checkout(&s, "a");
    let total = charge(&shown, "total");
    // The seed's: San Francisco's fees and tax on each store's part.
    assert_eq!(
        charge(&shown, "delivery"),
        2 * 299,
        "a delivery fee a store"
    );
    let stale = Val::S64(total - 1);
    let refused = answered(&s, PLACE, "a", &[stale]);
    assert_eq!(
        refused,
        serde_json::json!({ "$case": "err", "value": { "$case": "total-changed", "value": total } })
    );
    let placed = s
        .command_answered(PLACE, "a", &[Val::S64(total)], None)
        .expect("answered");
    assert!(placed.committed, "{:?}", placed.result);
    let owner = s.owner("a");
    let charged = match s.layer_read("a", "store:data/user-orders#charged", &owner) {
        Val::Option(Some(c)) => *c,
        other => panic!("charged: {other:?}"),
    };
    assert_eq!(val_at(&charged, &["total"]), Some(&Val::S64(total)));
    let Some(Val::List(parts)) = val_at(&charged, &["parts"]) else {
        panic!("parts: {charged:?}")
    };
    assert_eq!(parts.len(), 2, "a part a store");
    assert_eq!(val_at(&parts[0], &["tax-rate"]), Some(&Val::S64(86_250)));
    assert_eq!(val_at(&parts[0], &["subtotal"]), Some(&Val::S64(700)));
    assert_eq!(val_at(&parts[1], &["subtotal"]), Some(&Val::S64(300)));
}

/// **The fees as the stores are first are alike on both layers**, and a
/// code reads back as it was set.
#[test]
fn the_fees_and_codes_read_back_alike_on_this_layer() {
    let s = served();
    let owner = s.owner("a");
    let fees = s.layer_read_args(
        "a",
        "store:data/fees#for-stores",
        &[Val::List(vec![
            Val::String(STORE_ID.into()),
            Val::String(SECOND_STORE.0.into()),
        ])],
    );
    let seed = crate::checkout::fees_val(&crate::checkout::seed_fees());
    let Val::List(found) = fees else {
        panic!("{fees:?}")
    };
    assert_eq!(found.len(), 2);
    for f in &found {
        assert_eq!(val_at(f, &["fees"]), Some(&seed));
    }
    let welcome = s.layer_read_args(
        "a",
        "store:data/promos#find",
        &[Val::String(owner), Val::String("WELCOME10".into())],
    );
    let Val::Option(Some(found)) = welcome else {
        panic!("{welcome:?}")
    };
    assert_eq!(
        val_at(&found, &["promo"]),
        Some(&crate::checkout::promo_val(
            &crate::checkout::seed_promos()[0]
        ))
    );
}

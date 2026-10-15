//! **An order's total, the store's data for it** (track `order-total`,
//! NEXT's item 6, step 6b): each store's fee schedule, the promo codes and
//! who has used each, what each reader chose at checkout (a tip and a code),
//! and each order's charges as charged, per store. Held by both layers to
//! one meaning; the total is the program's (`examples/store/checkout.pw`),
//! computed by ADR-0327's money, and recorded here as the program computed
//! it.
//!
//! - In memory: `State`'s `fees`, `promos`, `promo_uses`, `choices` and
//!   `charged`, first [`seed_fees`] and [`seed_promos`].
//! - On PostgreSQL: `store_fees`, `promos`, `promo_uses`,
//!   `checkout_choices`, `orders`' `tip`, `total` and `promo`, and
//!   `order_parts` (`migrations/store/0013_order_total.sql`).
//!
//! A code is used once per reader, by the data's own uniqueness:
//! `promo_uses`' key on PostgreSQL, a set in memory. A use that conflicts in
//! `place_order`'s transaction is the refusal `Promo(AlreadyUsed)`, so two
//! orders at once cannot both use one code.

use super::*;
use crate::data::Ops;
use crate::store::{Rows, With, Written};
use crate::store_pg::Counted;

/// **A store's fee schedule**: amounts in cents, rates in millionths
/// (ADR-0327).
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Fees {
    pub(crate) delivery_fee: i64,
    pub(crate) service_rate: i64,
    pub(crate) service_min: i64,
    pub(crate) service_max: i64,
    pub(crate) small_order_under: i64,
    pub(crate) small_order_fee: i64,
    pub(crate) tax_rate: i64,
    pub(crate) tax_delivery: bool,
    pub(crate) tax_service: bool,
}

/// **What a promo takes off**: a rate in millionths, or an amount in cents.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Off {
    Percent(i64),
    Amount(i64),
}

/// **A promo code**: one store's, at least a subtotal, until an instant (in
/// milliseconds), funded by the store or the platform.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Promo {
    pub(crate) code: String,
    pub(crate) store: String,
    pub(crate) off: Off,
    pub(crate) min_subtotal: i64,
    pub(crate) expires: i64,
    pub(crate) by_store: bool,
}

/// **The tip a reader chose**: none, a percent, or an amount in cents.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub(crate) enum Tip {
    #[default]
    None,
    Fifteen,
    Eighteen,
    Twenty,
    Amount(i64),
}

/// **What a reader chose at checkout**: a tip, none at first, and a code.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub(crate) struct Chosen {
    pub(crate) tip: Tip,
    pub(crate) promo: Option<String>,
}

/// **One store's part of an order, as charged** (the integrator's ruling of
/// 2026-10-13): with the rate applied, so a later rate never rewrites it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Part {
    pub(crate) store: String,
    pub(crate) subtotal: i64,
    pub(crate) discount: i64,
    pub(crate) funded_by: Option<String>,
    pub(crate) delivery: i64,
    pub(crate) service: i64,
    pub(crate) small_order: i64,
    pub(crate) taxable: i64,
    pub(crate) tax_rate: i64,
    pub(crate) tax: i64,
}

/// **An order as charged**: its parts, the tip and the total, and the code
/// it used.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Charged {
    pub(crate) parts: Vec<Part>,
    pub(crate) tip: i64,
    pub(crate) total: i64,
    pub(crate) promo: Option<String>,
}

/// **Each store's fees, as the store is first**: San Francisco's, the
/// stores' city. Delivery $2.99; service 15%, at least $3.00 and at most
/// $12.00; $2.50 under a $10.00 subtotal; sales tax 8.625%, CDTFA's table
/// for the City and County of San Francisco, effective 2026-10-01 (read
/// 2026-10-13). Neither delivery nor service taxed: the platform charges
/// them for its own services, as the store's agent, and the store is the
/// retailer of its food (the integrator's ruling of 2026-10-13; the ADR has
/// the sources). `0013_order_total.sql` is the same rows.
pub(crate) fn seed_fees() -> Fees {
    Fees {
        delivery_fee: 299,
        service_rate: 150_000,
        service_min: 300,
        service_max: 1200,
        small_order_under: 1000,
        small_order_fee: 250,
        tax_rate: 86_250,
        tax_delivery: false,
        tax_service: false,
    }
}

/// **The codes, as the store is first**: `WELCOME10`, 10% off Blue Bottle's
/// food over $15.00, the platform's, until 2030. `0013_order_total.sql` is
/// the same row.
pub(crate) fn seed_promos() -> Vec<Promo> {
    vec![Promo {
        code: "WELCOME10".into(),
        store: STORE_ID.into(),
        off: Off::Percent(100_000),
        min_subtotal: 1500,
        expires: 1_893_456_000_000,
        by_store: false,
    }]
}

fn int(v: i64) -> Val {
    Val::S64(v)
}

fn case(name: &str, v: Option<Val>) -> Val {
    Val::Variant(name.into(), v.map(Box::new))
}

/// **The fees, as the program's `FeeSchedule` is.**
pub(crate) fn fees_val(f: &Fees) -> Val {
    Val::Record(vec![
        ("delivery-fee".into(), int(f.delivery_fee)),
        ("service-rate".into(), int(f.service_rate)),
        ("service-min".into(), int(f.service_min)),
        ("service-max".into(), int(f.service_max)),
        ("small-order-under".into(), int(f.small_order_under)),
        ("small-order-fee".into(), int(f.small_order_fee)),
        ("tax-rate".into(), int(f.tax_rate)),
        ("tax-delivery".into(), Val::Bool(f.tax_delivery)),
        ("tax-service".into(), Val::Bool(f.tax_service)),
    ])
}

fn s64_at(v: &Val, k: &str) -> Result<i64, String> {
    match val_at(v, &[k]) {
        Some(Val::S64(n)) => Ok(*n),
        other => Err(format!("`{k}` is {other:?}")),
    }
}

fn bool_at(v: &Val, k: &str) -> Result<bool, String> {
    match val_at(v, &[k]) {
        Some(Val::Bool(b)) => Ok(*b),
        other => Err(format!("`{k}` is {other:?}")),
    }
}

/// [`fees_val`], read back: what `fees#set` is passed.
pub(crate) fn fees_from(v: &Val) -> Result<Fees, String> {
    Ok(Fees {
        delivery_fee: s64_at(v, "delivery-fee")?,
        service_rate: s64_at(v, "service-rate")?,
        service_min: s64_at(v, "service-min")?,
        service_max: s64_at(v, "service-max")?,
        small_order_under: s64_at(v, "small-order-under")?,
        small_order_fee: s64_at(v, "small-order-fee")?,
        tax_rate: s64_at(v, "tax-rate")?,
        tax_delivery: bool_at(v, "tax-delivery")?,
        tax_service: bool_at(v, "tax-service")?,
    })
}

/// **A promo, as the program's `Promo` is.**
pub(crate) fn promo_val(p: &Promo) -> Val {
    Val::Record(vec![
        ("code".into(), Val::String(p.code.clone())),
        ("store".into(), Val::String(p.store.clone())),
        (
            "off".into(),
            match p.off {
                Off::Percent(r) => case("percent-off", Some(int(r))),
                Off::Amount(a) => case("amount-off", Some(int(a))),
            },
        ),
        ("min-subtotal".into(), int(p.min_subtotal)),
        ("expires".into(), int(p.expires)),
        (
            "funded-by".into(),
            case(
                if p.by_store {
                    "by-store"
                } else {
                    "by-platform"
                },
                None,
            ),
        ),
    ])
}

/// [`promo_val`], read back: what `promos#set` is passed.
pub(crate) fn promo_from(v: &Val) -> Result<Promo, String> {
    let string = |k: &str| match val_at(v, &[k]) {
        Some(Val::String(s)) => Ok(s.clone()),
        other => Err(format!("`{k}` is {other:?}")),
    };
    let off = match val_at(v, &["off"]) {
        Some(Val::Variant(name, Some(n))) => match (name.as_str(), n.as_ref()) {
            ("percent-off", Val::S64(r)) => Off::Percent(*r),
            ("amount-off", Val::S64(a)) => Off::Amount(*a),
            other => return Err(format!("`off` is {other:?}")),
        },
        other => return Err(format!("`off` is {other:?}")),
    };
    let by_store = match val_at(v, &["funded-by"]) {
        Some(Val::Variant(name, None)) => name == "by-store",
        other => return Err(format!("`funded-by` is {other:?}")),
    };
    Ok(Promo {
        code: string("code")?,
        store: string("store")?,
        off,
        min_subtotal: s64_at(v, "min-subtotal")?,
        expires: s64_at(v, "expires")?,
        by_store,
    })
}

/// **A tip, as the program's `Tip` is.**
pub(crate) fn tip_val(t: &Tip) -> Val {
    match t {
        Tip::None => case("no-tip", None),
        Tip::Fifteen => case("fifteen", None),
        Tip::Eighteen => case("eighteen", None),
        Tip::Twenty => case("twenty", None),
        Tip::Amount(a) => case("tip-amount", Some(int(*a))),
    }
}

pub(crate) fn tip_from(v: &Val) -> Result<Tip, String> {
    Ok(match v {
        Val::Variant(name, None) if name == "no-tip" => Tip::None,
        Val::Variant(name, None) if name == "fifteen" => Tip::Fifteen,
        Val::Variant(name, None) if name == "eighteen" => Tip::Eighteen,
        Val::Variant(name, None) if name == "twenty" => Tip::Twenty,
        Val::Variant(name, Some(a)) if name == "tip-amount" => match a.as_ref() {
            Val::S64(a) => Tip::Amount(*a),
            other => return Err(format!("a tip of {other:?}")),
        },
        other => return Err(format!("a tip of {other:?}")),
    })
}

/// **What a reader chose, as the program's `Chosen` is.**
pub(crate) fn chosen_val(c: &Chosen) -> Val {
    Val::Record(vec![
        ("tip".into(), tip_val(&c.tip)),
        (
            "promo".into(),
            Val::Option(c.promo.clone().map(|p| Box::new(Val::String(p)))),
        ),
    ])
}

/// **An order as charged, as the program's `OrderCharged` is.**
pub(crate) fn charged_val(c: &Charged) -> Val {
    let parts = c
        .parts
        .iter()
        .map(|p| {
            Val::Record(vec![
                ("store".into(), Val::String(p.store.clone())),
                ("subtotal".into(), int(p.subtotal)),
                ("discount".into(), int(p.discount)),
                (
                    "funded-by".into(),
                    Val::Option(p.funded_by.as_ref().map(|f| Box::new(case(f, None)))),
                ),
                ("delivery".into(), int(p.delivery)),
                ("service".into(), int(p.service)),
                ("small-order".into(), int(p.small_order)),
                ("taxable".into(), int(p.taxable)),
                ("tax-rate".into(), int(p.tax_rate)),
                ("tax".into(), int(p.tax)),
            ])
        })
        .collect();
    Val::Record(vec![
        ("parts".into(), Val::List(parts)),
        ("tip".into(), int(c.tip)),
        ("total".into(), int(c.total)),
    ])
}

/// [`charged_val`], read back, with the code the order used: what
/// `user-orders#place` is passed.
pub(crate) fn charged_from(v: &Val, promo: Option<String>) -> Result<Charged, String> {
    let Some(Val::List(parts)) = val_at(v, &["parts"]) else {
        return Err(format!("an order charged {v:?}"));
    };
    let parts = parts
        .iter()
        .map(|p| {
            Ok(Part {
                store: match val_at(p, &["store"]) {
                    Some(Val::String(s)) => s.clone(),
                    other => return Err(format!("`store` is {other:?}")),
                },
                subtotal: s64_at(p, "subtotal")?,
                discount: s64_at(p, "discount")?,
                funded_by: match val_at(p, &["funded-by"]) {
                    Some(Val::Option(None)) => None,
                    Some(Val::Option(Some(f))) => match f.as_ref() {
                        Val::Variant(name, None) => Some(name.clone()),
                        other => return Err(format!("`funded-by` is {other:?}")),
                    },
                    other => return Err(format!("`funded-by` is {other:?}")),
                },
                delivery: s64_at(p, "delivery")?,
                service: s64_at(p, "service")?,
                small_order: s64_at(p, "small-order")?,
                taxable: s64_at(p, "taxable")?,
                tax_rate: s64_at(p, "tax-rate")?,
                tax: s64_at(p, "tax")?,
            })
        })
        .collect::<Result<Vec<_>, String>>()?;
    Ok(Charged {
        parts,
        tip: s64_at(v, "tip")?,
        total: s64_at(v, "total")?,
        promo,
    })
}

/// **A reader's cart, each store's lines**, as the program's `CartPart`s
/// are, in the order the cart first lists a store's line.
fn parts_val(r: &mut dyn Rows, reader: &str) -> Result<Val, String> {
    let lines = r.cart(reader)?;
    // By set (NEXT 4b): each line's store in one read.
    let items: Vec<String> = lines.iter().map(|l| l.item.clone()).collect();
    let of_item = r.stores_of_items(&items)?;
    let mut parts: Vec<(String, Lines)> = Vec::new();
    for line in lines {
        let store = of_item.get(&line.item).cloned().unwrap_or_default();
        match parts.iter_mut().find(|(s, _)| *s == store) {
            Some((_, lines)) => lines.push(line),
            None => parts.push((store, vec![line])),
        }
    }
    Ok(Val::List(
        parts
            .iter()
            .map(|(store, lines)| {
                let Val::Record(mut cart) = cart_value(lines) else {
                    unreachable!("a record")
                };
                let (_, lines) = cart.remove(0);
                Val::Record(vec![
                    ("store".into(), Val::String(store.clone())),
                    ("lines".into(), lines),
                ])
            })
            .collect(),
    ))
}

/// **The checkout's reads**, over `with`, the layer's rows: the fees of a
/// list of stores, in one statement on PostgreSQL; a reader's cart by store;
/// what a reader chose; and a code, with whether the reader used it.
pub(crate) fn ops(with: With) -> Ops {
    let mut ops: Ops = BTreeMap::new();
    let w = with.clone();
    ops.insert(
        "store:data/fees#for-stores".to_string(),
        Arc::new(move |args: &[Val]| match args {
            [Val::List(stores)] => {
                let stores = stores
                    .iter()
                    .map(|s| match s {
                        Val::String(s) => Ok(s.clone()),
                        other => Err(format!("fees#for-stores received {other:?}")),
                    })
                    .collect::<Result<Vec<_>, String>>()?;
                w(&mut |r| {
                    let found = r.fees(&stores)?;
                    Ok(vec![Val::List(
                        found
                            .iter()
                            .map(|(store, f)| {
                                Val::Record(vec![
                                    ("store".into(), Val::String(store.clone())),
                                    ("fees".into(), fees_val(f)),
                                ])
                            })
                            .collect(),
                    )])
                })
            }
            other => Err(format!("fees#for-stores received {other:?}")),
        }),
    );
    let w = with.clone();
    ops.insert(
        "store:data/checkout#parts".to_string(),
        Arc::new(move |args: &[Val]| match args {
            [Val::String(reader)] => w(&mut |r| Ok(vec![parts_val(r, reader)?])),
            other => Err(format!("checkout#parts received {other:?}")),
        }),
    );
    let w = with.clone();
    ops.insert(
        "store:data/checkout#chosen".to_string(),
        Arc::new(move |args: &[Val]| match args {
            [Val::String(reader)] => w(&mut |r| Ok(vec![chosen_val(&r.chosen(reader)?)])),
            other => Err(format!("checkout#chosen received {other:?}")),
        }),
    );
    let w = with.clone();
    ops.insert(
        "store:data/promos#find".to_string(),
        Arc::new(move |args: &[Val]| match args {
            [Val::String(reader), Val::String(code)] => w(&mut |r| {
                let found = r.promo(reader, code.trim())?.map(|(p, used)| {
                    Box::new(Val::Record(vec![
                        ("promo".into(), promo_val(&p)),
                        ("used".into(), Val::Bool(used)),
                    ]))
                });
                Ok(vec![Val::Option(found)])
            }),
            other => Err(format!("promos#find received {other:?}")),
        }),
    );
    ops.insert(
        "store:data/user-orders#charged".to_string(),
        Arc::new(move |args: &[Val]| match args {
            [Val::String(reader)] => with(&mut |r| {
                Ok(vec![Val::Option(
                    r.charged(reader)?.map(|c| Box::new(charged_val(&c))),
                )])
            }),
            other => Err(format!("user-orders#charged received {other:?}")),
        }),
    );
    ops
}

/// **The checkout's writes**, a command's: the tip a reader chose, and the
/// code applied or taken off. Each answers what the reader has chosen.
pub(crate) fn writes(with: With, written: Arc<Mutex<Written>>) -> Ops {
    let mut ops: Ops = BTreeMap::new();
    let (w, wrote) = (with.clone(), written.clone());
    ops.insert(
        "store:data/checkout#set-tip".to_string(),
        Arc::new(move |args: &[Val]| match args {
            [Val::String(reader), tip] => {
                let tip = tip_from(tip)?;
                w(&mut |r| {
                    let mut chosen = r.chosen(reader)?;
                    chosen.tip = tip.clone();
                    r.set_chosen(reader, &chosen)?;
                    wrote.lock().expect("written").wrote = true;
                    Ok(vec![chosen_val(&chosen)])
                })
            }
            other => Err(format!("checkout#set-tip received {other:?}")),
        }),
    );
    ops.insert(
        "store:data/checkout#set-promo".to_string(),
        Arc::new(move |args: &[Val]| match args {
            [Val::String(reader), Val::Option(code)] => {
                let code = match code.as_deref() {
                    None => None,
                    Some(Val::String(c)) => Some(c.trim().to_string()),
                    Some(other) => return Err(format!("checkout#set-promo received {other:?}")),
                };
                with(&mut |r| {
                    let mut chosen = r.chosen(reader)?;
                    chosen.promo = code.clone();
                    r.set_chosen(reader, &chosen)?;
                    written.lock().expect("written").wrote = true;
                    Ok(vec![chosen_val(&chosen)])
                })
            }
            other => Err(format!("checkout#set-promo received {other:?}")),
        }),
    );
    ops
}

/// **The host's own writes** (`store:host/…`, a test's or a deployment's
/// staff's, never a program's): a store's fees, and a promo code.
pub(crate) fn host_writes(with: With, written: Arc<Mutex<Written>>) -> Ops {
    let mut ops: Ops = BTreeMap::new();
    let (w, wrote) = (with.clone(), written.clone());
    ops.insert(
        format!("{}fees#set", crate::store::HOST_OPS),
        Arc::new(move |args: &[Val]| {
            let [Val::String(store), fees] = args else {
                return Err(format!("fees#set received {args:?}"));
            };
            let fees = fees_from(fees)?;
            w(&mut |r| {
                r.set_fees(store, &fees)?;
                wrote.lock().expect("written").wrote = true;
                Ok(ok_unit())
            })
        }),
    );
    ops.insert(
        format!("{}promos#set", crate::store::HOST_OPS),
        Arc::new(move |args: &[Val]| {
            let [promo] = args else {
                return Err(format!("promos#set received {args:?}"));
            };
            let promo = promo_from(promo)?;
            with(&mut |r| {
                r.set_promo(&promo)?;
                written.lock().expect("written").wrote = true;
                Ok(ok_unit())
            })
        }),
    );
    ops
}

fn ok_unit() -> Vec<Val> {
    vec![Val::Result(Ok(None))]
}

/// **An order placed with its charges**, by the reader's handle: the code's
/// use first, which the data refuses where the reader used it already, and
/// then the order, its charges, the cart emptied and the choices cleared.
/// `Ok(false)` where the code was used: nothing is written.
pub(crate) fn place_charged(
    r: &mut dyn Rows,
    owner: &str,
    lines: &Lines,
    charged: &Charged,
) -> Result<bool, String> {
    if let Some(code) = &charged.promo
        && !r.use_promo(owner, code)?
    {
        return Ok(false);
    }
    r.place(owner, lines)?;
    r.charge(owner, charged)?;
    r.set_chosen(owner, &Chosen::default())?;
    Ok(true)
}

/// `code` refused as used: `Promo(AlreadyUsed)`.
pub(crate) fn already_used() -> Vec<Val> {
    vec![Val::Result(Err(Some(Box::new(case(
        "promo",
        Some(case("already-used", None)),
    )))))]
}

/// The arguments `user-orders#place` takes after the reader: the order as
/// charged, and the code it used.
pub(crate) fn place_args(rest: &[Val]) -> Result<Charged, String> {
    let [charged, Val::Option(code)] = rest else {
        return Err(format!("user-orders#place received {rest:?}"));
    };
    let code = match code.as_deref() {
        None => None,
        Some(Val::String(c)) => Some(c.clone()),
        Some(other) => return Err(format!("user-orders#place received {other:?}")),
    };
    charged_from(charged, code)
}

// --- PostgreSQL ----------------------------------------------------------

fn pg(e: postgres::Error) -> String {
    crate::feed_pg::pg(e)
}

fn fees_row(r: &postgres::Row) -> (String, Fees) {
    (
        r.get(0),
        Fees {
            delivery_fee: r.get(1),
            service_rate: r.get(2),
            service_min: r.get(3),
            service_max: r.get(4),
            small_order_under: r.get(5),
            small_order_fee: r.get(6),
            tax_rate: r.get(7),
            tax_delivery: r.get(8),
            tax_service: r.get(9),
        },
    )
}

/// **The fees of a list of stores, in one statement**, by set (the
/// integrator's ruling): in the list's order, a store without a schedule
/// left out.
pub(crate) fn pg_fees(
    c: &mut Counted<'_>,
    stores: &[String],
) -> Result<Vec<(String, Fees)>, String> {
    let rows = c
        .query(
            "SELECT store, delivery_fee, service_rate, service_min, service_max, \
                    small_order_under, small_order_fee, tax_rate, tax_delivery, tax_service \
             FROM store_fees WHERE store = ANY($1)",
            &[&stores],
        )
        .map_err(pg)?;
    let found: BTreeMap<String, Fees> = rows.iter().map(fees_row).collect();
    Ok(stores
        .iter()
        .filter_map(|s| found.get(s).map(|f| (s.clone(), f.clone())))
        .collect())
}

pub(crate) fn pg_set_fees(c: &mut Counted<'_>, store: &str, f: &Fees) -> Result<(), String> {
    c.execute(
        "INSERT INTO store_fees (store, delivery_fee, service_rate, service_min, service_max, \
             small_order_under, small_order_fee, tax_rate, tax_delivery, tax_service) \
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10) \
         ON CONFLICT (store) DO UPDATE SET delivery_fee = EXCLUDED.delivery_fee, \
             service_rate = EXCLUDED.service_rate, service_min = EXCLUDED.service_min, \
             service_max = EXCLUDED.service_max, small_order_under = EXCLUDED.small_order_under, \
             small_order_fee = EXCLUDED.small_order_fee, tax_rate = EXCLUDED.tax_rate, \
             tax_delivery = EXCLUDED.tax_delivery, tax_service = EXCLUDED.tax_service",
        &[
            &store,
            &f.delivery_fee,
            &f.service_rate,
            &f.service_min,
            &f.service_max,
            &f.small_order_under,
            &f.small_order_fee,
            &f.tax_rate,
            &f.tax_delivery,
            &f.tax_service,
        ],
    )
    .map_err(pg)?;
    Ok(())
}

/// **A code, and whether `owner` used it**, in one statement.
pub(crate) fn pg_promo(
    c: &mut Counted<'_>,
    owner: &str,
    code: &str,
) -> Result<Option<(Promo, bool)>, String> {
    let row = c
        .query_opt(
            "SELECT code, store, percent_off, amount_off, min_subtotal, expires_at, funded_by, \
                    EXISTS (SELECT 1 FROM promo_uses u WHERE u.code = p.code AND u.owner = $2) \
             FROM promos p WHERE code = $1",
            &[&code, &owner],
        )
        .map_err(pg)?;
    Ok(row.map(|r| {
        let percent: Option<i64> = r.get(2);
        let amount: Option<i64> = r.get(3);
        let funded: String = r.get(6);
        (
            Promo {
                code: r.get(0),
                store: r.get(1),
                off: match percent {
                    Some(p) => Off::Percent(p),
                    None => Off::Amount(amount.unwrap_or(0)),
                },
                min_subtotal: r.get(4),
                expires: r.get(5),
                by_store: funded == "store",
            },
            r.get(7),
        )
    }))
}

pub(crate) fn pg_set_promo(c: &mut Counted<'_>, p: &Promo) -> Result<(), String> {
    let (percent, amount) = match p.off {
        Off::Percent(r) => (Some(r), None),
        Off::Amount(a) => (None, Some(a)),
    };
    let funded = if p.by_store { "store" } else { "platform" };
    c.execute(
        "INSERT INTO promos (code, store, percent_off, amount_off, min_subtotal, expires_at, \
             funded_by) VALUES ($1, $2, $3, $4, $5, $6, $7) \
         ON CONFLICT (code) DO UPDATE SET store = EXCLUDED.store, \
             percent_off = EXCLUDED.percent_off, amount_off = EXCLUDED.amount_off, \
             min_subtotal = EXCLUDED.min_subtotal, expires_at = EXCLUDED.expires_at, \
             funded_by = EXCLUDED.funded_by",
        &[
            &p.code,
            &p.store,
            &percent,
            &amount,
            &p.min_subtotal,
            &p.expires,
            &funded,
        ],
    )
    .map_err(pg)?;
    Ok(())
}

/// **`owner`'s use of `code`, recorded**, or `false` where the data already
/// holds it: the key on `(code, owner)` decides, so of two transactions at
/// once one records it and the other, waiting on it, finds the conflict.
pub(crate) fn pg_use_promo(c: &mut Counted<'_>, owner: &str, code: &str) -> Result<bool, String> {
    let n = c
        .execute(
            "INSERT INTO promo_uses (code, owner) VALUES ($1, $2) ON CONFLICT DO NOTHING",
            &[&code, &owner],
        )
        .map_err(pg)?;
    Ok(n == 1)
}

/// **`owner`'s use of `code` released** (NEXT 6c, step eight): an order
/// taken back before it was paid used none.
pub(crate) fn pg_release_promo(c: &mut Counted<'_>, owner: &str, code: &str) -> Result<(), String> {
    c.execute(
        "DELETE FROM promo_uses WHERE code = $1 AND owner = $2",
        &[&code, &owner],
    )
    .map_err(pg)?;
    Ok(())
}

fn tip_parts(t: &Tip) -> (&'static str, i64) {
    match t {
        Tip::None => ("none", 0),
        Tip::Fifteen => ("fifteen", 0),
        Tip::Eighteen => ("eighteen", 0),
        Tip::Twenty => ("twenty", 0),
        Tip::Amount(a) => ("amount", *a),
    }
}

pub(crate) fn pg_chosen(c: &mut Counted<'_>, owner: &str) -> Result<Chosen, String> {
    let row = c
        .query_opt(
            "SELECT tip_kind, tip_amount, promo FROM checkout_choices WHERE owner = $1",
            &[&owner],
        )
        .map_err(pg)?;
    Ok(match row {
        None => Chosen::default(),
        Some(r) => {
            let kind: String = r.get(0);
            Chosen {
                tip: match kind.as_str() {
                    "fifteen" => Tip::Fifteen,
                    "eighteen" => Tip::Eighteen,
                    "twenty" => Tip::Twenty,
                    "amount" => Tip::Amount(r.get(1)),
                    _ => Tip::None,
                },
                promo: r.get(2),
            }
        }
    })
}

pub(crate) fn pg_set_chosen(
    c: &mut Counted<'_>,
    owner: &str,
    chosen: &Chosen,
) -> Result<(), String> {
    let (kind, amount) = tip_parts(&chosen.tip);
    c.execute(
        "INSERT INTO checkout_choices (owner, tip_kind, tip_amount, promo) \
         VALUES ($1, $2, $3, $4) ON CONFLICT (owner) DO UPDATE SET \
             tip_kind = EXCLUDED.tip_kind, tip_amount = EXCLUDED.tip_amount, \
             promo = EXCLUDED.promo",
        &[&owner, &kind, &amount, &chosen.promo],
    )
    .map_err(pg)?;
    Ok(())
}

/// **`owner`'s latest order, charged**: its tip, total and code, and each
/// store's part.
pub(crate) fn pg_charge(c: &mut Counted<'_>, owner: &str, charged: &Charged) -> Result<(), String> {
    let id: i64 = c
        .query_one(
            "UPDATE orders SET tip = $2, total = $3, promo = $4 WHERE id = \
                 (SELECT id FROM orders WHERE owner = $1 ORDER BY id DESC LIMIT 1) RETURNING id",
            &[&owner, &charged.tip, &charged.total, &charged.promo],
        )
        .map_err(pg)?
        .get(0);
    for (position, p) in charged.parts.iter().enumerate() {
        c.execute(
            "INSERT INTO order_parts (order_id, position, store, subtotal, discount, funded_by, \
                 delivery, service, small_order, taxable, tax_rate, tax) \
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12)",
            &[
                &id,
                &(position as i32),
                &p.store,
                &p.subtotal,
                &p.discount,
                &p.funded_by,
                &p.delivery,
                &p.service,
                &p.small_order,
                &p.taxable,
                &p.tax_rate,
                &p.tax,
            ],
        )
        .map_err(pg)?;
    }
    Ok(())
}

pub(crate) fn pg_charged(c: &mut Counted<'_>, owner: &str) -> Result<Option<Charged>, String> {
    let Some(order) = c
        .query_opt(
            "SELECT id, tip, total, promo FROM orders WHERE owner = $1 ORDER BY id DESC LIMIT 1",
            &[&owner],
        )
        .map_err(pg)?
    else {
        return Ok(None);
    };
    let (id, tip, total): (i64, Option<i64>, Option<i64>) =
        (order.get(0), order.get(1), order.get(2));
    let (Some(tip), Some(total)) = (tip, total) else {
        return Ok(None);
    };
    let parts = c
        .query(
            "SELECT store, subtotal, discount, funded_by, delivery, service, small_order, \
                    taxable, tax_rate, tax FROM order_parts WHERE order_id = $1 ORDER BY position",
            &[&id],
        )
        .map_err(pg)?
        .iter()
        .map(|r| Part {
            store: r.get(0),
            subtotal: r.get(1),
            discount: r.get(2),
            funded_by: r.get(3),
            delivery: r.get(4),
            service: r.get(5),
            small_order: r.get(6),
            taxable: r.get(7),
            tax_rate: r.get(8),
            tax: r.get(9),
        })
        .collect();
    Ok(Some(Charged {
        parts,
        tip,
        total,
        promo: order.get(3),
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn database() -> Option<String> {
        match std::env::var("PW_STORE_DATABASE_URL") {
            Ok(url) if !url.is_empty() => Some(url),
            _ => {
                eprintln!("skipped: PW_STORE_DATABASE_URL is not set");
                None
            }
        }
    }

    /// **A cart's stores' fees are one statement on PostgreSQL, whatever the
    /// number of stores** (the integrator's ruling): through the layer's own
    /// operation, every statement it runs counted, for one store and for
    /// two.
    #[test]
    fn on_postgres_the_fees_of_a_carts_stores_are_one_statement() {
        let Some(url) = database() else { return };
        let layer = crate::store_pg::StorePg::for_test(&url).expect("the layer");
        let counted = |stores: &[&str]| {
            let ops = crate::data::DataLayer::reads(&layer, "s", None);
            let read = ops.get("store:data/fees#for-stores").expect("registered");
            let list = Val::List(stores.iter().map(|s| Val::String(s.to_string())).collect());
            crate::store_pg::STATEMENTS.with(|n| n.set(0));
            let answer = read(&[list]).expect("answered");
            let [Val::List(found)] = answer.as_slice() else {
                panic!("{answer:?}")
            };
            assert_eq!(found.len(), stores.len(), "{answer:?}");
            crate::store_pg::STATEMENTS.with(|n| n.get())
        };
        assert_eq!(counted(&["47"]), 1, "one store");
        assert_eq!(counted(&["47", "48"]), 1, "two stores");
    }

    /// **A code is used once per reader, by the data, even by two orders at
    /// once** (the integrator's ruling of 2026-10-13): of two transactions
    /// recording one reader's use, the second waits on the first and, once
    /// it commits, finds the conflict; the control, another reader's use of
    /// the same code is recorded.
    #[test]
    fn on_postgres_two_orders_at_once_use_a_code_once() {
        let Some(url) = database() else { return };
        let layer = crate::store_pg::StorePg::for_test(&url).expect("the layer");
        let mut first = layer.sql();
        let mut second = layer.sql();
        first.batch_execute("BEGIN").expect("begun");
        assert!(pg_use_promo(&mut Counted(&mut first), "ada", "WELCOME10").expect("used"));
        let waiting = std::thread::spawn(move || {
            second.batch_execute("BEGIN").expect("begun");
            let used = pg_use_promo(&mut Counted(&mut second), "ada", "WELCOME10").expect("ran");
            let other = pg_use_promo(&mut Counted(&mut second), "bea", "WELCOME10").expect("ran");
            second.batch_execute("COMMIT").expect("committed");
            (used, other)
        });
        std::thread::sleep(std::time::Duration::from_millis(300));
        assert!(!waiting.is_finished(), "the second waits on the first");
        first.batch_execute("COMMIT").expect("committed");
        let (used, other) = waiting.join().expect("joined");
        assert!(!used, "ada's second use conflicts");
        assert!(other, "bea's use is recorded");
    }
}

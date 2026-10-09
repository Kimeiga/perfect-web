//! **The store's data** (ADR-0218): what the food-delivery app's data layer
//! holds, which a deployment supplies to the host. Until ADR-0218 it was the
//! host's own state, and no other program could be served.
//!
//! Track `store-pg`: **one set of operations over two stores of rows.** The
//! operations a query reads through and a command writes through are built
//! once, here, over [`Rows`]: the in-memory layer's ([`StoreData`]) and
//! PostgreSQL's (`store_pg.rs`). Each layer supplies its rows and its
//! transaction; what an operation answers, and how a fault meets it, is
//! this file's, so the two layers cannot answer differently.
//!
//! **All of the store's state is the layer's**: the stores and their menus,
//! the stock, the carts, the orders, the notice, the kitchen's preparation
//! time, the curated recommendations and each session's estimate. A route
//! that changes any of it stages one of the host's own operations
//! (`store:host/…`) through [`DataLayer::begin`] and commits it with its
//! declared events, as a command does. No program is linked to those
//! operations: the host strips them from a component's operations, and a
//! program that names one is refused before it is served.
//!
//! **The fault hooks stay test controls** ([`Faults`]): delays, one-shot
//! failures, the recommender's gate and the slow category. Both layers apply
//! them, as a database that is slow or down would meet either.
//!
//! [`DataLayer::begin`]: crate::data::DataLayer::begin

use super::*;
use crate::data::Ops;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};

/// **The prefix of the host's own operations** (track `store-pg`): what a
/// route that changes the store's data stages, and no program may import.
pub(crate) const HOST_OPS: &str = "store:host/";

/// **What the store's data layer is, to the host** (track `store-pg`): a
/// data layer, and the test controls its operations meet.
pub(crate) trait StoreLayer: crate::data::DataLayer {
    /// The fault hooks a test sets, which every operation of the layer meets.
    fn faults(&self) -> &Faults;

    /// **A line's quantity written around the program** (ADR-0179), as a
    /// database's row can be: a test's, of what the store must refuse to
    /// show. A database that holds the invariant itself refuses it.
    #[cfg(test)]
    fn line_around(&self, session: &str, at: usize, quantity: i64) -> Result<(), String>;
}

/// **The layer the server's tests serve the store on** (track `store-pg`):
/// in memory, or on PostgreSQL where `PW_STORE_TEST_LAYER=postgres` and
/// `PW_STORE_DATABASE_URL` say, in a schema of its own that is dropped with
/// it. So the whole of the server's suite runs on either layer.
#[cfg(test)]
pub(crate) fn layer_for_tests() -> Arc<dyn StoreLayer> {
    if std::env::var("PW_STORE_TEST_LAYER").as_deref() == Ok("postgres") {
        let url = std::env::var("PW_STORE_DATABASE_URL")
            .expect("PW_STORE_TEST_LAYER=postgres needs PW_STORE_DATABASE_URL");
        return Arc::new(
            crate::store_pg::StorePg::for_test(&url).expect("the store on PostgreSQL"),
        );
    }
    Arc::new(StoreData::new())
}

/// **The test controls every operation of the store meets** (charter §15.5,
/// ADR-0174, ADR-0177, ADR-0148, ADR-0223, E14's T07 and T10). Not the
/// store's data: what a database or a source that is slow, held or down does
/// to it.
pub(crate) struct Faults {
    /// **A test's: the next command's data layer answers `cart-expired`**,
    /// the declared refusal, once (ADR-0174).
    pub(crate) fail_next: AtomicBool,
    /// **How long the store's data layer takes to answer for a store**
    /// (charter §15.5's store delay, ADR-0174), in milliseconds: one delay
    /// for every reader, as the store is one value for all of them. A test
    /// sets it through `/bench/store`.
    pub(crate) store_delay_ms: Arc<AtomicU64>,
    /// **Whether the store's next read fails at its origin** (ADR-0177),
    /// once, for every reader. A test sets it through `/bench/store?fail=next`.
    pub(crate) store_fails_next: Arc<AtomicBool>,
    /// **What each session's cart meets at the database** (charter §15.5,
    /// ADR-0174): how long a read of it takes, and whether its next write
    /// and its next read fail. A test sets them through `/bench/cart` and
    /// `/bench/fail`.
    pub(crate) cart_faults: Arc<Mutex<BTreeMap<String, CartFaults>>>,
    /// **How the recommender answers** (ADR-0148, ADR-0223): its delay, how
    /// it fails, and the gate a held read waits at. What it recommends is
    /// the layer's data.
    pub(crate) recommender: Mutex<Recommender>,
    /// **How each session's estimator answers** (E14, T10): its delay and
    /// how it fails; the default for a session no test has. What it
    /// estimates is the layer's data.
    pub(crate) estimators: Mutex<BTreeMap<String, Estimator>>,
    /// **The menu's slow category** (E14, T07): which, and how slow.
    pub(crate) categories: Mutex<Categories>,
    /// How many reads of a category saw they were stopped, and ended early
    /// (E14, T07).
    pub(crate) category_stopped: Arc<AtomicU64>,
}

impl Faults {
    pub(crate) fn new() -> Faults {
        Faults {
            fail_next: AtomicBool::new(false),
            store_delay_ms: Arc::new(AtomicU64::new(0)),
            store_fails_next: Arc::new(AtomicBool::new(false)),
            cart_faults: Arc::new(Mutex::new(BTreeMap::new())),
            recommender: Mutex::new(Recommender::default()),
            estimators: Mutex::new(BTreeMap::new()),
            categories: Mutex::new(Categories::default()),
            category_stopped: Arc::new(AtomicU64::new(0)),
        }
    }
}

/// **A store, as the data layer holds it** (ADR-0125, ADR-0192).
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct StoreRow {
    pub(crate) id: String,
    pub(crate) name: String,
    pub(crate) description: String,
    pub(crate) opens_minute: i64,
    pub(crate) closes_minute: i64,
}

/// **One item of a store's menu, as the data layer holds it** (ADR-0166,
/// ADR-0169, ADR-0178, ADR-0181, E14's T07).
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Item {
    pub(crate) id: String,
    pub(crate) name: String,
    pub(crate) description: String,
    /// In cents (ADR-0169).
    pub(crate) price: i64,
    /// The section it is listed under (ADR-0181), its id and name.
    pub(crate) category: String,
    pub(crate) category_name: String,
    /// How it is served, `hot` or `cold`: what T07's categories read.
    pub(crate) served: String,
    /// Whether it can be ordered now (ADR-0178).
    pub(crate) available: bool,
}

impl Item {
    /// **An item a store's menu gains** (E7-P), or one it was first seeded
    /// with: described, priced and listed as the store's catalogue says of
    /// its id, and available.
    pub(crate) fn new(store: &str, id: &str, name: &str) -> Item {
        let (category, category_name) = item_category(store, id);
        Item {
            id: id.to_string(),
            name: name.to_string(),
            description: item_description(id).to_string(),
            price: item_price(id),
            category: category.to_string(),
            category_name: category_name.to_string(),
            served: category_of(id).to_string(),
            available: true,
        }
    }
}

/// **A session's order** (ADR-0193): the cart's lines it was placed with,
/// and its status's case.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Order {
    pub(crate) status: String,
    pub(crate) lines: Lines,
}

/// **A session's estimate** (E14, T10; ADR-0180): the least minutes and,
/// where a test said, the most.
pub(crate) type Estimate = (i64, Option<i64>);

/// **The store's rows, as an operation reads and writes them** (track
/// `store-pg`): the one thing each layer supplies. A read is of what the
/// layer's transaction sees; a write is staged in it, and kept only if it
/// commits.
pub(crate) trait Rows {
    /// Every store, in the order the home page lists them (ADR-0192).
    fn stores(&mut self) -> Result<Vec<StoreRow>, String>;
    /// A store's menu, in its order; empty where the store has none.
    fn menu(&mut self, store: &str) -> Result<Vec<Item>, String>;
    /// A store's menu, replaced whole.
    fn set_menu(&mut self, store: &str, items: &[Item]) -> Result<(), String>;
    /// Whether every store's item `item` can be ordered.
    fn stock(&mut self, item: &str, available: bool) -> Result<(), String>;
    /// **What a new line records of `item`** (ADR-0172): its name and price
    /// in the last store that lists it, and whether any store sells it now.
    fn catalog_item(&mut self, item: &str) -> Result<Option<(String, i64, bool)>, String>;
    /// A session's cart.
    fn cart(&mut self, session: &str) -> Result<Lines, String>;
    fn set_cart(&mut self, session: &str, lines: &[Line]) -> Result<(), String>;
    /// A session's order, its latest.
    fn order(&mut self, session: &str) -> Result<Option<Order>, String>;
    /// A session's order, placed with `lines`.
    fn place(&mut self, session: &str, lines: &[Line]) -> Result<(), String>;
    /// A session's order moved along, the lines it was placed with kept;
    /// one set with none placed has none. `None` removes it.
    fn set_status(&mut self, session: &str, status: Option<&str>) -> Result<(), String>;
    fn notice(&mut self, store: &str) -> Result<Option<String>, String>;
    fn set_notice(&mut self, store: &str, text: &str) -> Result<(), String>;
    fn prep(&mut self, store: &str) -> Result<Option<i64>, String>;
    fn set_prep(&mut self, store: &str, minutes: i64) -> Result<(), String>;
    /// What the recommender suggests for every store, where it was curated.
    fn curated(&mut self) -> Result<Option<Vec<(String, String)>>, String>;
    fn curate(&mut self, items: Option<&[(String, String)]>) -> Result<(), String>;
    fn estimate(&mut self, session: &str) -> Result<Option<Estimate>, String>;
    fn set_estimate(&mut self, session: &str, estimate: Estimate) -> Result<(), String>;
}

/// **Where an operation's rows are**, each layer's: it runs `f` on them, in
/// a query's own read or in the command's transaction.
pub(crate) type With = Arc<
    dyn Fn(&mut dyn FnMut(&mut dyn Rows) -> Result<Vec<Val>, String>) -> Result<Vec<Val>, String>
        + Send
        + Sync,
>;

/// **What a command's writes left to commit** (track `store-pg`): whether
/// it wrote, and the session's cart's total where it wrote the cart, which
/// the host's materializer records as the in-memory path always has.
#[derive(Default)]
pub(crate) struct Written {
    pub(crate) wrote: bool,
    pub(crate) cart_total: Option<i64>,
}

impl Written {
    /// **What the commit writes in the materializer's transaction**, by key,
    /// or none where nothing was written: the session's cart's total.
    pub(crate) fn rows(&self, session: &str) -> Option<Vec<(String, String)>> {
        if !self.wrote {
            return None;
        }
        Some(match self.cart_total {
            Some(total) => vec![(format!("cart:{session}"), total.to_string())],
            None => Vec::new(),
        })
    }
}

fn ok(v: Val) -> Vec<Val> {
    vec![Val::Result(Ok(Some(Box::new(v))))]
}

fn declared(case: &str) -> Vec<Val> {
    vec![Val::Result(Err(Some(Box::new(Val::Variant(
        case.into(),
        None,
    )))))]
}

fn not_found() -> Vec<Val> {
    declared("not-found")
}

/// A host operation's answer: done, with a value where it has one.
fn done(v: Option<Val>) -> Vec<Val> {
    vec![Val::Result(Ok(v.map(Box::new)))]
}

/// A host operation refused, and why: nothing it staged commits.
fn refused(why: String) -> Vec<Val> {
    vec![Val::Result(Err(Some(Box::new(Val::String(why)))))]
}

/// A store as the program's `Store` is (ADR-0125, ADR-0166).
fn store_val(row: &StoreRow) -> Val {
    Val::Record(vec![
        ("id".into(), Val::String(row.id.clone())),
        ("name".into(), Val::String(row.name.clone())),
        // A program whose `Store` declares none is not given it (ADR-0166):
        // the benchmark's.
        ("description".into(), Val::String(row.description.clone())),
        (
            "hours".into(),
            Val::Record(vec![
                ("opens-minute".into(), Val::S64(row.opens_minute)),
                ("closes-minute".into(), Val::S64(row.closes_minute)),
            ]),
        ),
    ])
}

/// An item as the program's `MenuItem` is (ADR-0166): the benchmark's, which
/// declares fewer fields, is passed what it declares.
fn item_val(store: &str, item: &Item) -> Val {
    Val::Record(vec![
        ("id".into(), Val::String(item.id.clone())),
        // Its store, and its category (ADR-0181).
        ("store-id".into(), Val::String(store.to_string())),
        ("name".into(), Val::String(item.name.clone())),
        ("description".into(), Val::String(item.description.clone())),
        // Its price, in cents (ADR-0169).
        (
            "price".into(),
            Val::Record(vec![("minor-units".into(), Val::S64(item.price))]),
        ),
        // Whether it can be ordered now (ADR-0178): what
        // `menus#is-available` answers.
        ("available".into(), Val::Bool(item.available)),
        (
            "category".into(),
            Val::Record(vec![
                ("id".into(), Val::String(item.category.clone())),
                ("name".into(), Val::String(item.category_name.clone())),
            ]),
        ),
    ])
}

/// `{ id, name }`, as a recommendation and a category's item are.
fn named(id: &str, name: &str) -> Val {
    Val::Record(vec![
        ("id".into(), Val::String(id.to_string())),
        ("name".into(), Val::String(name.to_string())),
    ])
}

/// **The menu, grouped by category** (ADR-0181): each category in the
/// order its first item is listed, with its items in theirs.
fn sections(store: &str, menu: &[Item]) -> Val {
    let mut grouped: Vec<(String, Val, Vec<Val>)> = Vec::new();
    for item in menu {
        let row = item_val(store, item);
        match grouped.iter_mut().find(|(c, ..)| *c == item.category) {
            Some((_, _, items)) => items.push(row),
            None => grouped.push((
                item.category.clone(),
                named(&item.category, &item.category_name),
                vec![row],
            )),
        }
    }
    Val::List(
        grouped
            .into_iter()
            .map(|(_, category, items)| {
                Val::Record(vec![
                    ("category".into(), category),
                    ("items".into(), Val::List(items)),
                ])
            })
            .collect(),
    )
}

/// The session an operation was passed, which must be the one the host gave
/// the component, or the component is acting for someone else.
fn own_session<'v>(op: &str, session: &str, args: &'v [Val]) -> Result<&'v [Val], String> {
    let Some(Val::String(s)) = args.first() else {
        return Err(format!("{op} received {args:?}"));
    };
    if s != session {
        return Err(format!("{op} was passed another session"));
    }
    Ok(&args[1..])
}

/// **The session's cart operations, as its faults make them** (ADR-0174).
/// A read waits the session's delay. The next write, and the next read,
/// fail once each as a database that is down fails: the operation answers
/// no value, and the component that called it traps.
pub(crate) fn faulted(
    cart_faults: &Arc<Mutex<BTreeMap<String, CartFaults>>>,
    session: &str,
    mut host: Ops,
) -> Ops {
    for (name, writes) in [
        ("current", false),
        ("add", true),
        ("decrease", true),
        ("remove", true),
        ("clear", true),
    ] {
        let key = format!("store:data/carts#{name}");
        let Some(op) = host.remove(&key) else {
            continue;
        };
        let faults = cart_faults.clone();
        let session = session.to_string();
        host.insert(
            key,
            Arc::new(move |args: &[Val]| {
                let delay = {
                    let mut all = faults.lock().expect("cart faults");
                    let mut mine = all.get_mut(&session);
                    let failing = mine.as_deref_mut().is_some_and(|m| {
                        std::mem::take(if writes {
                            &mut m.fail_write
                        } else {
                            &mut m.fail_read
                        })
                    });
                    if failing {
                        return Err(format!("carts#{name}: the database is unavailable"));
                    }
                    mine.map(|m| if writes { 0 } else { m.delay_ms })
                        .unwrap_or_default()
                };
                if delay > 0 {
                    std::thread::sleep(std::time::Duration::from_millis(delay));
                }
                op(args)
            }),
        );
    }
    host
}

/// **The store's reads, for one session** (ADR-0218): the cart, with the
/// faults a test set; the session's order and its delivery estimate; and
/// the catalogue. Each through `with`, the layer's rows. Nothing a query does
/// commits.
///
/// Its slow sources poll `stopped`, to end early a read nobody is waiting
/// for (ADR-0152).
pub(crate) fn reads(with: With, session: &str, faults: &Faults, stopped: Option<Stopped>) -> Ops {
    let mut ops = faulted(
        &faults.cart_faults,
        session,
        unfaulted(with.clone(), session, faults),
    );
    ops.extend(catalog(with, faults, stopped));
    ops
}

/// [`reads`], before the cart's faults meet them.
fn unfaulted(with: With, session: &str, faults: &Faults) -> Ops {
    let mut ops: Ops = BTreeMap::new();
    let this = session.to_string();
    let w = with.clone();
    ops.insert(
        "store:data/carts#current".to_string(),
        Arc::new(move |args: &[Val]| {
            let rest = own_session("carts#current", &this, args)?;
            if !rest.is_empty() {
                return Err(format!("carts#current received {args:?}"));
            }
            w(&mut |r| Ok(ok(cart_value(&r.cart(&this)?))))
        }),
    );
    // The session's order, as its status's case (E14, T04). A case the
    // program's type does not have is refused by the component's types, as
    // any value the host gives is.
    let (this, w) = (session.to_string(), with.clone());
    ops.insert(
        "store:data/orders#current".to_string(),
        Arc::new(move |args: &[Val]| {
            own_session("orders#current", &this, args)?;
            w(&mut |r| {
                let status = r
                    .order(&this)?
                    .map(|o| Box::new(Val::Variant(o.status, None)));
                Ok(ok(Val::Option(status)))
            })
        }),
    );
    // **And the lines it was placed with** (ADR-0193), a cart's value, or
    // none before one is placed. No program reads it yet; the store's tests
    // read an order through it.
    let (this, w) = (session.to_string(), with.clone());
    ops.insert(
        "store:data/orders#placed".to_string(),
        Arc::new(move |args: &[Val]| {
            own_session("orders#placed", &this, args)?;
            w(&mut |r| {
                let cart = r.order(&this)?.map(|o| Box::new(cart_value(&o.lines)));
                Ok(vec![Val::Option(cart)])
            })
        }),
    );
    // The session's delivery estimate (E14, T10), after the estimator's
    // delay: a slow source, which a streamed region does not wait for.
    let estimator = faults
        .estimators
        .lock()
        .expect("estimators")
        .get(session)
        .cloned()
        .unwrap_or_default();
    let (this, w) = (session.to_string(), with.clone());
    ops.insert(
        "store:data/estimates#current".to_string(),
        Arc::new(move |args: &[Val]| {
            own_session("estimates#current", &this, args)?;
            std::thread::sleep(std::time::Duration::from_millis(estimator.delay_ms));
            match estimator.fail.as_deref() {
                Some("declared") => return Ok(declared("location-unavailable")),
                Some(_) => return Err("the estimator is down".to_string()),
                None => {}
            }
            // A range, and when it was made (ADR-0180). And `minutes`, which
            // the benchmark's own store still reads (ADR-0156): a program is
            // passed the fields its type declares (ADR-0166).
            w(&mut |r| {
                let (minutes, max) = r.estimate(&this)?.unwrap_or(DEFAULT_ESTIMATE);
                Ok(ok(Val::Record(vec![
                    ("minutes".into(), Val::S64(minutes)),
                    ("min-minutes".into(), Val::S64(minutes)),
                    ("max-minutes".into(), Val::S64(max.unwrap_or(minutes + 10))),
                    ("generated-at".into(), Val::S64(wall_millis())),
                ])))
            })
        }),
    );
    ops
}

/// What a session no test gave an estimate is estimated (ADR-0180).
pub(crate) const DEFAULT_ESTIMATE: Estimate = (25, None);

/// **The deployment's catalogue** (ADR-0125): `store:data/stores#get` and
/// `store:data/menus#sections`, what the `Store` and `Menu` queries read,
/// and `menus#for-store`, the benchmark's own store's menu (ADR-0156,
/// ADR-0181); the notice, the kitchen, the menu by category and the
/// recommender. Any store the layer does not hold is answered `not-found`.
fn catalog(with: With, faults: &Faults, stopped: Option<Stopped>) -> Ops {
    let mut ops: Ops = BTreeMap::new();
    // Charter §15.5's store delay (ADR-0174), and its origin failing once
    // (ADR-0177), as a test set them.
    let store_delay = faults.store_delay_ms.clone();
    let store_fails = faults.store_fails_next.clone();
    let w = with.clone();
    ops.insert(
        "store:data/stores#get".to_string(),
        Arc::new(move |args: &[Val]| match args {
            [Val::String(id)] => {
                let found = w(&mut |r| {
                    Ok(r.stores()?
                        .iter()
                        .find(|s| s.id == *id)
                        .map(store_val)
                        .into_iter()
                        .collect())
                })?;
                let Some(store) = found.into_iter().next() else {
                    return Ok(not_found());
                };
                if store_fails.swap(false, Ordering::SeqCst) {
                    return Err("stores#get: the store's origin is unavailable".to_string());
                }
                let delay = store_delay.load(Ordering::SeqCst);
                if delay > 0 {
                    std::thread::sleep(std::time::Duration::from_millis(delay));
                }
                Ok(ok(store))
            }
            other => Err(format!("stores#get received {other:?}")),
        }),
    );
    // Every store this server holds, in its order (ADR-0192): what the home
    // page lists.
    let w = with.clone();
    ops.insert(
        "store:data/stores#list".to_string(),
        Arc::new(move |args: &[Val]| match args {
            [] => w(&mut |r| Ok(vec![Val::List(r.stores()?.iter().map(store_val).collect())])),
            other => Err(format!("stores#list received {other:?}")),
        }),
    );
    // A store's menu, `None` where the layer holds no such store.
    fn menu_of(r: &mut dyn Rows, id: &str) -> Result<Option<Vec<Item>>, String> {
        if !r.stores()?.iter().any(|s| s.id == id) {
            return Ok(None);
        }
        r.menu(id).map(Some)
    }
    let w = with.clone();
    ops.insert(
        "store:data/menus#for-store".to_string(),
        Arc::new(move |args: &[Val]| match args {
            [Val::String(id)] => w(&mut |r| {
                Ok(match menu_of(r, id)? {
                    Some(menu) => ok(Val::List(menu.iter().map(|i| item_val(id, i)).collect())),
                    None => not_found(),
                })
            }),
            other => Err(format!("menus#for-store received {other:?}")),
        }),
    );
    let w = with.clone();
    ops.insert(
        "store:data/menus#sections".to_string(),
        Arc::new(move |args: &[Val]| match args {
            [Val::String(id)] => w(&mut |r| {
                Ok(match menu_of(r, id)? {
                    Some(menu) => ok(sections(id, &menu)),
                    None => not_found(),
                })
            }),
            other => Err(format!("menus#sections received {other:?}")),
        }),
    );
    // What the recommender answers (ADR-0148), after its delay: a slow
    // source, which a streamed region does not wait for. What it suggests is
    // what a test curated, or each store's own menu after its first item, two
    // of them (ADR-0165), so it changes when the menu does.
    let recommender = faults.recommender.lock().expect("recommender").clone();
    let w = with.clone();
    ops.insert(
        "store:data/recommendations#for-store".to_string(),
        Arc::new(move |args: &[Val]| {
            let [Val::String(store)] = args else {
                return Err(format!("recommendations#for-store received {args:?}"));
            };
            // Held until a test lets it go (ADR-0223), then slow.
            if let Some(gate) = &recommender.gate {
                gate.wait();
            }
            std::thread::sleep(std::time::Duration::from_millis(recommender.delay_ms));
            let items = w(&mut |r| {
                let items = match r.curated()? {
                    Some(items) => items,
                    None => match menu_of(r, store)? {
                        Some(menu) => recommended_from(
                            &menu
                                .iter()
                                .map(|i| (i.id.clone(), i.name.clone()))
                                .collect::<Vec<_>>(),
                        ),
                        None => Vec::new(),
                    },
                };
                Ok(items.iter().map(|(id, name)| named(id, name)).collect())
            })?;
            match recommender.fail.as_deref() {
                Some("host") => Err("the recommender is down".to_string()),
                Some(_) => Ok(declared("none-available")),
                None => Ok(ok(Val::List(items))),
            }
        }),
    );
    // What the store has posted (E14, T09). Every call is counted, as every
    // data-layer call is: what `/bench/calls` reports.
    let w = with.clone();
    ops.insert(
        "store:data/notices#current".to_string(),
        Arc::new(move |args: &[Val]| match args {
            [Val::String(id)] => w(&mut |r| {
                Ok(match r.notice(id)? {
                    Some(text) => ok(Val::String(text)),
                    None => not_found(),
                })
            }),
            other => Err(format!("notices#current received {other:?}")),
        }),
    );
    // How long the kitchen takes now (E14, T02), after [`KITCHEN_MS`]: a
    // slow source. The minutes are the ones it was asked with.
    let w = with.clone();
    ops.insert(
        "store:data/kitchen#prep-minutes".to_string(),
        Arc::new(move |args: &[Val]| match args {
            [Val::String(id)] => {
                let minutes = w(&mut |r| Ok(r.prep(id)?.map(Val::S64).into_iter().collect()))?;
                let Some(minutes) = minutes.into_iter().next() else {
                    return Ok(not_found());
                };
                std::thread::sleep(std::time::Duration::from_millis(KITCHEN_MS));
                Ok(ok(minutes))
            }
            other => Err(format!("kitchen#prep-minutes received {other:?}")),
        }),
    );
    // The menu's items served hot or cold (E14, T07), the slow category after
    // its delay. A read that is stopped meanwhile ends early, and says so:
    // what `/bench/calls` counts as stopped.
    let categories = faults.categories.lock().expect("categories").clone();
    let ended_early = faults.category_stopped.clone();
    ops.insert(
        "store:data/menus#in-category".to_string(),
        Arc::new(move |args: &[Val]| match args {
            [Val::String(id), Val::String(category)] => {
                let Some(menu) = with(&mut |r| {
                    Ok(menu_of(r, id)?
                        .map(|menu| {
                            Val::List(
                                menu.iter()
                                    .filter(|i| category == "all" || i.served == *category)
                                    .map(|i| named(&i.id, &i.name))
                                    .collect(),
                            )
                        })
                        .into_iter()
                        .collect())
                })?
                .into_iter()
                .next() else {
                    return Ok(not_found());
                };
                if categories.slow.as_deref() == Some(category.as_str()) {
                    let until = std::time::Instant::now()
                        + std::time::Duration::from_millis(categories.delay_ms);
                    while std::time::Instant::now() < until {
                        if stopped.as_ref().is_some_and(|s| s()) {
                            ended_early.fetch_add(1, Ordering::SeqCst);
                            return Err("stopped: nobody is waiting for it".to_string());
                        }
                        std::thread::sleep(std::time::Duration::from_millis(10));
                    }
                }
                Ok(ok(menu))
            }
            other => Err(format!("menus#in-category received {other:?}")),
        }),
    );
    ops
}

/// **A command's operations** (ADR-0218): every read, in the command's own
/// transaction; the cart's writes and the order a session places, which the
/// program's commands call; and the host's own writes (`store:host/…`),
/// which routes that change data call and no program is linked to. Each
/// write stages in `with`'s rows, and `written` says what is left to
/// commit. `fail` makes the cart's operations answer `cart-expired` (a
/// test's).
pub(crate) fn writes(
    with: With,
    session: &str,
    faults: &Faults,
    fail: bool,
    written: Arc<Mutex<Written>>,
) -> Ops {
    let mut ops = unfaulted(with.clone(), session, faults);
    ops.extend(catalog(with.clone(), faults, None));
    // **The deployment's data layer: `store:data/carts`, whole.** Every
    // operation the interface declares, whichever command is running; the
    // host links only those the component imports and was granted.
    type Change = fn(&mut dyn Rows, &mut Lines, &[Val]) -> Result<(), String>;
    let mut cart = |name: &'static str, change: Change| {
        let (this, w, written) = (session.to_string(), with.clone(), written.clone());
        ops.insert(
            format!("store:data/carts#{name}"),
            Arc::new(move |args: &[Val]| {
                let op = format!("carts#{name}");
                let rest = own_session(&op, &this, args)?;
                if fail {
                    return Ok(declared("cart-expired"));
                }
                w(&mut |r| {
                    let mut lines = r.cart(&this)?;
                    change(r, &mut lines, rest)?;
                    r.set_cart(&this, &lines)?;
                    let mut written = written.lock().expect("written");
                    written.wrote = true;
                    written.cart_total = Some(lines.iter().map(|l| l.quantity).sum());
                    Ok(ok(cart_value(&lines)))
                })
            }),
        );
    };
    cart("add", |r, lines, args| {
        let [Val::String(item), Val::S64(quantity)] = args else {
            return Err(format!("carts#add received {args:?}"));
        };
        match lines.iter_mut().find(|l| l.item == *item) {
            Some(line) => line.quantity += quantity,
            None => {
                // The line records its item as it is now (ADR-0172).
                let (name, price, _) = r
                    .catalog_item(item)?
                    .ok_or_else(|| format!("carts#add: no store has an item `{item}`"))?;
                lines.push(Line {
                    item: item.clone(),
                    quantity: *quantity,
                    name,
                    price,
                });
            }
        }
        Ok(())
    });
    // One fewer; a line that holds one goes (ADR-0172). A line that is not
    // there is not: another page took it away first.
    cart("decrease", |_, lines, args| {
        let [Val::String(item)] = args else {
            return Err(format!("carts#decrease received {args:?}"));
        };
        if let Some(at) = lines.iter().position(|l| l.item == *item) {
            if lines[at].quantity > 1 {
                lines[at].quantity -= 1;
            } else {
                lines.remove(at);
            }
        }
        Ok(())
    });
    // The line, gone (ADR-0172).
    cart("remove", |_, lines, args| {
        let [Val::String(item)] = args else {
            return Err(format!("carts#remove received {args:?}"));
        };
        lines.retain(|l| l.item != *item);
        Ok(())
    });
    cart("clear", |_, lines, args| {
        if !args.is_empty() {
            return Err(format!("carts#clear received {args:?}"));
        }
        lines.clear();
        Ok(())
    });
    // **The store's data layer for orders, as a command sees it**
    // (ADR-0193): `orders#place` places the session's cart as its order.
    // The order is the cart's lines and the status `placed`, and the cart
    // is emptied, in the command's own transaction. An empty cart places
    // nothing, and says so. Until 2026-10-08 the order kept its status
    // alone, and its lines went with the emptied cart.
    let (this, w, placed) = (session.to_string(), with.clone(), written.clone());
    ops.insert(
        "store:data/orders#place".to_string(),
        Arc::new(move |args: &[Val]| {
            let rest = own_session("orders#place", &this, args)?;
            if !rest.is_empty() {
                return Err(format!("orders#place received {args:?}"));
            }
            w(&mut |r| {
                let lines = r.cart(&this)?;
                if lines.is_empty() {
                    return Ok(declared("nothing-to-order"));
                }
                r.place(&this, &lines)?;
                r.set_cart(&this, &[])?;
                let mut written = placed.lock().expect("written");
                written.wrote = true;
                written.cart_total = Some(0);
                Ok(ok(Val::Variant("placed".into(), None)))
            })
        }),
    );
    // Whether an item can be ordered now (charter §15.4), read inside the
    // command that adds it. One no store's menu has is not (ADR-0172): a page
    // sends the item it showed, and a request can name any.
    let w = with.clone();
    ops.insert(
        "store:data/menus#is-available".to_string(),
        Arc::new(move |args: &[Val]| match args {
            [Val::String(item)] => w(&mut |r| {
                Ok(vec![Val::Bool(
                    r.catalog_item(item)?.is_some_and(|(_, _, can)| can),
                )])
            }),
            other => Err(format!("menus#is-available received {other:?}")),
        }),
    );
    ops = faulted(&faults.cart_faults, session, ops);
    ops.extend(host_writes(with, session, written));
    ops
}

/// **The host's own writes** (track `store-pg`): what a route that changes
/// the store's data stages, in the command's transaction, before the host
/// commits it with the route's declared events. Each answers `Ok`, with a
/// value where it has one, or `Err` with why, and then nothing it staged
/// commits.
fn host_writes(with: With, session: &str, written: Arc<Mutex<Written>>) -> Ops {
    type Write = fn(&mut dyn Rows, &str, &[Val]) -> Result<Result<Option<Val>, String>, String>;
    let mut ops: Ops = BTreeMap::new();
    let mut write = |name: &'static str, f: Write| {
        let (this, w, written) = (session.to_string(), with.clone(), written.clone());
        ops.insert(
            format!("{HOST_OPS}{name}"),
            Arc::new(move |args: &[Val]| {
                w(&mut |r| match f(r, &this, args)? {
                    Ok(answer) => {
                        // A write that changed nothing (`menus#replace` to
                        // the size it is) answers `false`, and commits none.
                        if answer != Some(Val::Bool(false)) {
                            written.lock().expect("written").wrote = true;
                        }
                        Ok(done(answer))
                    }
                    Err(why) => Ok(refused(why)),
                })
            }),
        );
    };
    // **A change to a store's menu** (E7-P): `MenuOp`'s, applied as it
    // always was, to the menu as the layer holds it. An item that stays
    // keeps what it was; one inserted is the catalogue's. A refusal changes
    // nothing.
    write("menus#change", |r, _, args| {
        let [Val::String(store), op @ ..] = args else {
            return Err(format!("menus#change received {args:?}"));
        };
        let op = MenuOp::from_vals(op)?;
        let menu = r.menu(store)?;
        let mut pairs: Vec<(String, String)> = menu
            .iter()
            .map(|i| (i.id.clone(), i.name.clone()))
            .collect();
        if let Err(why) = op.apply(&mut pairs) {
            return Ok(Err(why));
        }
        let items: Vec<Item> = pairs
            .iter()
            .map(|(id, name)| match menu.iter().find(|i| i.id == *id) {
                Some(item) => Item {
                    name: name.clone(),
                    ..item.clone()
                },
                None => Item::new(store, id, name),
            })
            .collect();
        r.set_menu(store, &items)?;
        // Whether it can be ordered (ADR-0178), in every store that sells it.
        if let MenuOp::Stock { id, available } = &op {
            r.stock(id, *available)?;
        }
        Ok(Ok(None))
    });
    // An item sold out at the source, or back, untold (charter §15.5's
    // forced stale item).
    write("menus#stock", |r, _, args| match args {
        [Val::String(item), Val::Bool(available)] => {
            r.stock(item, *available)?;
            Ok(Ok(None))
        }
        other => Err(format!("menus#stock received {other:?}")),
    });
    // **A store's menu, `n` items long** (E7 gate item 10's large menu):
    // `item-0` on. One that long already changes nothing.
    write("menus#replace", |r, _, args| match args {
        [Val::String(store), Val::S64(n)] => {
            if r.menu(store)?.len() as i64 == *n {
                return Ok(Ok(Some(Val::Bool(false))));
            }
            let items: Vec<Item> = (0..*n)
                .map(|i| Item::new(store, &format!("item-{i}"), &format!("Item {i}")))
                .collect();
            r.set_menu(store, &items)?;
            Ok(Ok(Some(Val::Bool(true))))
        }
        other => Err(format!("menus#replace received {other:?}")),
    });
    // **The kitchen moves the session's order along** (E14, T04; ADR-0193),
    // or removes it.
    write("orders#set-status", |r, this, args| {
        let rest = own_session("orders#set-status", this, args)?;
        match rest {
            [Val::Option(status)] => {
                let status = match status.as_deref() {
                    Some(Val::String(s)) => Some(s.as_str()),
                    None => None,
                    Some(other) => return Err(format!("orders#set-status received {other:?}")),
                };
                r.set_status(this, status)?;
                Ok(Ok(None))
            }
            other => Err(format!("orders#set-status received {other:?}")),
        }
    });
    // The store's staff post a notice (E14, T09).
    write("notices#post", |r, _, args| match args {
        [Val::String(store), Val::String(text)] => {
            r.set_notice(store, text)?;
            Ok(Ok(None))
        }
        other => Err(format!("notices#post received {other:?}")),
    });
    // The kitchen's prep time changes (E14, T02).
    write("kitchen#set-prep", |r, _, args| match args {
        [Val::String(store), Val::S64(minutes)] => {
            r.set_prep(store, *minutes)?;
            Ok(Ok(None))
        }
        other => Err(format!("kitchen#set-prep received {other:?}")),
    });
    // What the recommender suggests (ADR-0148), curated, or drawn from each
    // store's menu again.
    write("recommendations#curate", |r, _, args| match args {
        [Val::Option(items)] => {
            let items = match items.as_deref() {
                None => None,
                Some(Val::List(items)) => Some(
                    items
                        .iter()
                        .map(|i| match (val_at(i, &["id"]), val_at(i, &["name"])) {
                            (Some(Val::String(id)), Some(Val::String(name))) => {
                                Ok((id.clone(), name.clone()))
                            }
                            _ => Err(format!("recommendations#curate received {i:?}")),
                        })
                        .collect::<Result<Vec<_>, String>>()?,
                ),
                Some(other) => return Err(format!("recommendations#curate received {other:?}")),
            };
            r.curate(items.as_deref())?;
            Ok(Ok(None))
        }
        other => Err(format!("recommendations#curate received {other:?}")),
    });
    // What the session's estimator estimates (E14, T10; ADR-0180).
    write("estimates#set", |r, this, args| {
        match own_session("estimates#set", this, args)? {
            [Val::S64(minutes), Val::Option(max)] => {
                let max = match max.as_deref() {
                    Some(Val::S64(m)) => Some(*m),
                    None => None,
                    Some(other) => return Err(format!("estimates#set received {other:?}")),
                };
                r.set_estimate(this, (*minutes, max))?;
                Ok(Ok(None))
            }
            other => Err(format!("estimates#set received {other:?}")),
        }
    });
    ops
}

impl MenuOp {
    /// **The change as a host operation is passed it** (track `store-pg`).
    pub(crate) fn to_vals(&self) -> Vec<Val> {
        let s = |v: &str| Val::String(v.to_string());
        let opt = |v: &Option<String>| Val::Option(v.as_deref().map(|v| Box::new(s(v))));
        match self {
            MenuOp::Insert {
                id,
                name,
                at,
                before,
            } => vec![s("insert"), s(id), s(name), opt(at), Val::Bool(*before)],
            MenuOp::Remove { id } => vec![s("remove"), s(id)],
            MenuOp::Move { id, after } => vec![s("move"), s(id), opt(after)],
            MenuOp::Rename { id, name } => vec![s("rename"), s(id), s(name)],
            MenuOp::Stock { id, available } => vec![s("stock"), s(id), Val::Bool(*available)],
        }
    }

    /// [`MenuOp::to_vals`], read back.
    fn from_vals(vals: &[Val]) -> Result<MenuOp, String> {
        let opt = |v: &Val| match v {
            Val::Option(None) => Ok(None),
            Val::Option(Some(v)) => match v.as_ref() {
                Val::String(s) => Ok(Some(s.clone())),
                other => Err(format!("{other:?}")),
            },
            other => Err(format!("{other:?}")),
        };
        let bad = || format!("menus#change received {vals:?}");
        Ok(match vals {
            [
                Val::String(k),
                Val::String(id),
                Val::String(name),
                at,
                Val::Bool(before),
            ] if k == "insert" => MenuOp::Insert {
                id: id.clone(),
                name: name.clone(),
                at: opt(at).map_err(|_| bad())?,
                before: *before,
            },
            [Val::String(k), Val::String(id)] if k == "remove" => MenuOp::Remove { id: id.clone() },
            [Val::String(k), Val::String(id), after] if k == "move" => MenuOp::Move {
                id: id.clone(),
                after: opt(after).map_err(|_| bad())?,
            },
            [Val::String(k), Val::String(id), Val::String(name)] if k == "rename" => {
                MenuOp::Rename {
                    id: id.clone(),
                    name: name.clone(),
                }
            }
            [Val::String(k), Val::String(id), Val::Bool(available)] if k == "stock" => {
                MenuOp::Stock {
                    id: id.clone(),
                    available: *available,
                }
            }
            _ => return Err(bad()),
        })
    }
}

/// **What a node grants the store's data layer** (ADR-0218), beyond the
/// platform's session and outbox. The host's own writes (`store:host/…`)
/// are granted to no program.
pub(crate) fn grants() -> &'static [&'static str] {
    &[
        "database.read<Carts>",
        "database.read<Menus>",
        // A session's order (E14, T04).
        "database.read<Orders>",
        // And the order a session places (ADR-0193).
        "database.write<Orders>",
        "database.read<Stores>",
        "database.write<Carts>",
        // The recommender (ADR-0148), a source reached over the network.
        "network.fetch",
        // A session's delivery estimate (E14, T10).
        "database.read<Estimates>",
        // The store's notice board (E14, T09).
        "database.read<Notices>",
        // The kitchen's prep time (E14, T02).
        "database.read<Kitchen>",
        // The menu by category (E14, T07).
        "database.read<Categories>",
    ]
}

/// **The store's data, as it is first** (ADR-0125, ADR-0162, ADR-0192): two
/// stores, 47, whose menu E7-P changes, and 48; the notice the store has
/// posted; how long its kitchen takes; nothing curated, no cart, no order
/// and no estimate. `migrations/store/0002_seed.sql` is the same rows, and
/// a test holds the two layers to answer alike.
#[derive(Debug, Clone)]
pub(crate) struct State {
    stores: Vec<StoreRow>,
    menus: BTreeMap<String, Vec<Item>>,
    orders: BTreeMap<String, Order>,
    notices: BTreeMap<String, String>,
    prep: BTreeMap<String, i64>,
    curated: Option<Vec<(String, String)>>,
    estimates: BTreeMap<String, Estimate>,
}

impl State {
    fn seed() -> State {
        let store = |id: &str| StoreRow {
            id: id.to_string(),
            name: store_named(id).unwrap_or_default().to_string(),
            description: store_description(id).to_string(),
            opens_minute: 7 * 60,
            closes_minute: 19 * 60,
        };
        let menu = |id: &str, items: Vec<(String, String)>| {
            (
                id.to_string(),
                items
                    .iter()
                    .map(|(item, name)| Item::new(id, item, name))
                    .collect(),
            )
        };
        State {
            stores: vec![store(STORE_ID), store(SECOND_STORE.0)],
            menus: BTreeMap::from([
                menu(STORE_ID, default_menu()),
                menu(SECOND_STORE.0, second_menu()),
            ]),
            orders: BTreeMap::new(),
            notices: BTreeMap::from([(STORE_ID.to_string(), "Open until 7 pm".to_string())]),
            prep: BTreeMap::from([(STORE_ID.to_string(), 12)]),
            curated: None,
            estimates: BTreeMap::new(),
        }
    }
}

/// **The in-memory layer's rows, for one session** (track `store-pg`): its
/// cart, and the rest of the store, as a read saw them or a command stages
/// them. A command's are committed by [`Staging`]'s `publish`.
struct MemRows {
    session: String,
    lines: Lines,
    state: Arc<State>,
}

impl MemRows {
    fn state(&mut self) -> &mut State {
        Arc::make_mut(&mut self.state)
    }
}

impl Rows for MemRows {
    fn stores(&mut self) -> Result<Vec<StoreRow>, String> {
        Ok(self.state.stores.clone())
    }
    fn menu(&mut self, store: &str) -> Result<Vec<Item>, String> {
        Ok(self.state.menus.get(store).cloned().unwrap_or_default())
    }
    fn set_menu(&mut self, store: &str, items: &[Item]) -> Result<(), String> {
        self.state().menus.insert(store.to_string(), items.to_vec());
        Ok(())
    }
    fn stock(&mut self, item: &str, available: bool) -> Result<(), String> {
        for menu in self.state().menus.values_mut() {
            for i in menu.iter_mut().filter(|i| i.id == item) {
                i.available = available;
            }
        }
        Ok(())
    }
    fn catalog_item(&mut self, item: &str) -> Result<Option<(String, i64, bool)>, String> {
        let mut found = None;
        let mut can = false;
        for store in &self.state.stores {
            for i in self.state.menus.get(&store.id).into_iter().flatten() {
                if i.id == item {
                    found = Some((i.name.clone(), i.price));
                    can |= i.available;
                }
            }
        }
        Ok(found.map(|(name, price)| (name, price, can)))
    }
    fn cart(&mut self, session: &str) -> Result<Lines, String> {
        if session != self.session {
            return Err("a cart is read for its own session".to_string());
        }
        Ok(self.lines.clone())
    }
    fn set_cart(&mut self, session: &str, lines: &[Line]) -> Result<(), String> {
        if session != self.session {
            return Err("a cart is written for its own session".to_string());
        }
        self.lines = lines.to_vec();
        Ok(())
    }
    fn order(&mut self, session: &str) -> Result<Option<Order>, String> {
        Ok(self.state.orders.get(session).cloned())
    }
    fn place(&mut self, session: &str, lines: &[Line]) -> Result<(), String> {
        let order = Order {
            status: "placed".to_string(),
            lines: lines.to_vec(),
        };
        self.state().orders.insert(session.to_string(), order);
        Ok(())
    }
    fn set_status(&mut self, session: &str, status: Option<&str>) -> Result<(), String> {
        let orders = &mut self.state().orders;
        match status {
            Some(status) => {
                orders
                    .entry(session.to_string())
                    .or_insert_with(|| Order {
                        status: String::new(),
                        lines: Lines::new(),
                    })
                    .status = status.to_string()
            }
            None => {
                orders.remove(session);
            }
        }
        Ok(())
    }
    fn notice(&mut self, store: &str) -> Result<Option<String>, String> {
        Ok(self.state.notices.get(store).cloned())
    }
    fn set_notice(&mut self, store: &str, text: &str) -> Result<(), String> {
        self.state()
            .notices
            .insert(store.to_string(), text.to_string());
        Ok(())
    }
    fn prep(&mut self, store: &str) -> Result<Option<i64>, String> {
        Ok(self.state.prep.get(store).copied())
    }
    fn set_prep(&mut self, store: &str, minutes: i64) -> Result<(), String> {
        self.state().prep.insert(store.to_string(), minutes);
        Ok(())
    }
    fn curated(&mut self) -> Result<Option<Vec<(String, String)>>, String> {
        Ok(self.state.curated.clone())
    }
    fn curate(&mut self, items: Option<&[(String, String)]>) -> Result<(), String> {
        self.state().curated = items.map(<[_]>::to_vec);
        Ok(())
    }
    fn estimate(&mut self, session: &str) -> Result<Option<Estimate>, String> {
        Ok(self.state.estimates.get(session).copied())
    }
    fn set_estimate(&mut self, session: &str, estimate: Estimate) -> Result<(), String> {
        self.state().estimates.insert(session.to_string(), estimate);
        Ok(())
    }
}

/// `with` over in-memory rows.
fn with_mem(rows: Arc<Mutex<MemRows>>) -> With {
    Arc::new(move |f| f(&mut *rows.lock().expect("rows")))
}

/// **The store's data, in memory** (ADR-0218): the default layer, and the
/// one the browser suite runs on.
pub(crate) struct StoreData {
    faults: Faults,
    /// Per-session cart lines — `(item, quantity)` — behind the
    /// materializer's command boundary. This is the deployment's DATA LAYER:
    /// `store:data/carts#add` is its operation, and the compiled command
    /// calls it through the host. Its lock is held from a command's first
    /// operation to its commit: one command at a time.
    carts: Mutex<BTreeMap<String, Lines>>,
    /// **The rest of the store** (track `store-pg`): its stores and menus,
    /// the orders, the notice, the kitchen, the recommender's curation and
    /// the estimates. Written by a command's commit alone.
    state: Mutex<Arc<State>>,
}

impl StoreData {
    /// **The page a document with none recorded is** (ADR-0218): the store's
    /// own, which its first documents and tests were served as.
    pub(crate) fn default_page() -> &'static str {
        "store.page.StorePage"
    }

    pub(crate) fn new() -> StoreData {
        StoreData {
            faults: Faults::new(),
            carts: Mutex::new(BTreeMap::new()),
            state: Mutex::new(Arc::new(State::seed())),
        }
    }

    /// **A command's writes, staged until its transaction commits**
    /// (ADR-0218): the session's cart and the rest of the store, read and
    /// written under the carts' lock, which the staging holds until it is
    /// dropped. `fail` makes the data layer answer `cart-expired` (a test's).
    pub(crate) fn begin(&self, session: &str, fail: bool) -> Staging<'_> {
        let carts = self.carts.lock().expect("carts");
        let rows = Arc::new(Mutex::new(MemRows {
            session: session.to_string(),
            lines: carts.get(session).cloned().unwrap_or_default(),
            state: self.state.lock().expect("state").clone(),
        }));
        let written: Arc<Mutex<Written>> = Arc::default();
        let ops = writes(
            with_mem(rows.clone()),
            session,
            &self.faults,
            fail,
            written.clone(),
        );
        Staging {
            store: self,
            session: session.to_string(),
            carts,
            rows,
            written,
            ops,
        }
    }
}

/// **One command's writes, staged until its transaction commits** (ADR-0218).
pub(crate) struct Staging<'a> {
    store: &'a StoreData,
    session: String,
    /// Held from the call until the commit is published.
    carts: std::sync::MutexGuard<'a, BTreeMap<String, Lines>>,
    rows: Arc<Mutex<MemRows>>,
    written: Arc<Mutex<Written>>,
    ops: Ops,
}

impl crate::data::Staged for Staging<'_> {
    fn ops(&self) -> Ops {
        self.ops.clone()
    }

    fn rows(&self) -> Option<Vec<(String, String)>> {
        self.written.lock().expect("written").rows(&self.session)
    }

    /// **After the commit, what was staged is the store's**: the cart, and
    /// the rest of the store it wrote, before what reads them is dropped and
    /// read again.
    fn publish(&mut self) {
        let written = self.written.lock().expect("written");
        if !written.wrote {
            return;
        }
        let rows = self.rows.lock().expect("rows");
        if written.cart_total.is_some() {
            self.carts.insert(self.session.clone(), rows.lines.clone());
        }
        *self.store.state.lock().expect("state") = rows.state.clone();
    }
}

impl crate::data::DataLayer for StoreData {
    fn reads(&self, session: &str, stopped: Option<Stopped>) -> Ops {
        // What a query reads is the store as it is now: the session's cart,
        // and the rest, as their last commit left them.
        let rows = MemRows {
            session: session.to_string(),
            lines: self
                .carts
                .lock()
                .expect("carts")
                .get(session)
                .cloned()
                .unwrap_or_default(),
            state: self.state.lock().expect("state").clone(),
        };
        reads(
            with_mem(Arc::new(Mutex::new(rows))),
            session,
            &self.faults,
            stopped,
        )
    }

    fn begin<'a>(&'a self, session: &str) -> Box<dyn crate::data::Staged + 'a> {
        let fail = self.faults.fail_next.swap(false, Ordering::SeqCst);
        Box::new(StoreData::begin(self, session, fail))
    }

    fn grants(&self) -> Vec<&'static str> {
        grants().to_vec()
    }

    fn default_page(&self) -> Option<&'static str> {
        Some(StoreData::default_page())
    }

    fn session_entry(&self) -> bool {
        true
    }

    fn style(&self) -> &'static str {
        crate::STYLE
    }
}

impl StoreLayer for StoreData {
    fn faults(&self) -> &Faults {
        &self.faults
    }

    #[cfg(test)]
    fn line_around(&self, session: &str, at: usize, quantity: i64) -> Result<(), String> {
        let mut carts = self.carts.lock().expect("carts");
        let line = carts
            .get_mut(session)
            .and_then(|lines| lines.get_mut(at))
            .ok_or("no such line")?;
        line.quantity = quantity;
        Ok(())
    }
}

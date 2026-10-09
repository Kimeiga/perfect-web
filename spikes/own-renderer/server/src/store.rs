//! **The store's data** (ADR-0218): what the food-delivery app's data layer
//! holds, which a deployment supplies to the host. Until ADR-0218 it was the
//! host's own state, and no other program could be served.

use super::*;

pub(crate) struct StoreData {
    /// **A test's: the next command's data layer answers `cart-expired`**,
    /// the declared refusal, once (ADR-0174).
    pub(crate) fail_next: std::sync::atomic::AtomicBool,
    /// The keyed collection E7-P's structural patches operate on.
    ///
    /// Mutable, because a list that never changes cannot demonstrate that a
    /// change preserves identity. Each item is `(key, name)` and the key is
    /// what `{#each menu as item key item.id}` declares.
    pub(crate) menu: Mutex<Vec<(String, String)>>,
    /// Per-session cart lines — `(item, quantity)` — behind the
    /// materializer's command boundary. This is the deployment's DATA LAYER:
    /// `store:data/carts#add` is its operation, and the compiled command calls
    /// it through the host.
    pub(crate) carts: Mutex<BTreeMap<String, Lines>>,
    /// **Each session's order** (E14, T04; ADR-0193): the cart's lines it
    /// was placed with, and its status's case, what
    /// `store:data/orders#current` answers. The kitchen sets the status
    /// through `/bench/order`; no order is `None`.
    pub(crate) orders: Mutex<BTreeMap<String, Order>>,
    /// **The recommender** (ADR-0148): what a streamed region's query reads,
    /// with the delay and failure a test sets.
    pub(crate) recommender: Mutex<Recommender>,
    /// **Each session's estimator** (E14, T10), as a test set it; the default
    /// for a session no test has.
    pub(crate) estimators: Mutex<BTreeMap<String, Estimator>>,
    /// **What the store has posted on its notice board** (E14, T09): what
    /// `store:data/notices#current` answers. A test posts a notice through
    /// `/bench/notice`, and nothing is told it changed.
    pub(crate) notice: Mutex<String>,
    /// **How long the kitchen takes now, in minutes** (E14, T02): what
    /// `store:data/kitchen#prep-minutes` answers, after [`KITCHEN_MS`]. A
    /// test changes it through `/bench/prep`, and nothing is told.
    pub(crate) prep_minutes: Mutex<i64>,
    /// **The items sold out** (charter §15.4, §15.5's forced stale item): what
    /// `store:data/menus#is-available` answers no for, inside the command
    /// that adds one. A test sells one out through `/bench/stock`, and the
    /// page that shows it is not told.
    pub(crate) sold_out: Mutex<std::collections::BTreeSet<String>>,
    /// **How long the store's data layer takes to answer for a store**
    /// (charter §15.5's store delay, ADR-0174), in milliseconds: one delay
    /// for every reader, as the store is one value for all of them. A test
    /// sets it through `/bench/store`.
    pub(crate) store_delay_ms: Arc<std::sync::atomic::AtomicU64>,
    /// **Whether the store's next read fails at its origin** (ADR-0177),
    /// once, for every reader. A test sets it through `/bench/store?fail=next`.
    pub(crate) store_fails_next: Arc<std::sync::atomic::AtomicBool>,
    /// **What each session's cart meets at the database** (charter §15.5,
    /// ADR-0174): how long a read of it takes, and whether its next write
    /// and its next read fail. A test sets them through `/bench/cart` and
    /// `/bench/fail`.
    pub(crate) cart_faults: Arc<Mutex<BTreeMap<String, CartFaults>>>,
    /// **The menu's categories** (E14, T07): which is slow, and how slow.
    pub(crate) categories: Mutex<Categories>,
    /// How many reads of a category saw they were stopped, and ended early
    /// (E14, T07).
    pub(crate) category_stopped: Arc<std::sync::atomic::AtomicU64>,
}

impl StoreData {
    /// **The page a document with none recorded is** (ADR-0218): the store's
    /// own, which its first documents and tests were served as.
    pub(crate) fn default_page() -> &'static str {
        "store.page.StorePage"
    }

    /// **What a node grants the store's data layer** (ADR-0218), beyond the
    /// platform's session and outbox.
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

    pub(crate) fn new() -> StoreData {
        StoreData {
            fail_next: std::sync::atomic::AtomicBool::new(false),
            menu: Mutex::new(default_menu()),
            carts: Mutex::new(BTreeMap::new()),
            orders: Mutex::new(BTreeMap::new()),
            recommender: Mutex::new(Recommender::default()),
            estimators: Mutex::new(BTreeMap::new()),
            notice: Mutex::new("Open until 7 pm".to_string()),
            prep_minutes: Mutex::new(12),
            sold_out: Mutex::new(std::collections::BTreeSet::new()),
            store_delay_ms: Arc::new(std::sync::atomic::AtomicU64::new(0)),
            store_fails_next: Arc::new(std::sync::atomic::AtomicBool::new(false)),
            cart_faults: Arc::new(Mutex::new(BTreeMap::new())),
            categories: Mutex::new(Categories::default()),
            category_stopped: Arc::new(std::sync::atomic::AtomicU64::new(0)),
        }
    }

    /// **Each item a store holds now, with its name and price** (ADR-0172):
    /// what a new line records. Store 47's menu as E7-P has changed it, and
    /// store 48's.
    pub(crate) fn catalog(&self) -> Arc<Catalog> {
        let first = self.menu.lock().expect("menu").clone();
        Arc::new(
            first
                .into_iter()
                .chain(second_menu())
                .map(|(id, name)| {
                    let price = item_price(&id);
                    (id, (name, price))
                })
                .collect(),
        )
    }

    /// **The session's cart operations, as its faults make them**
    /// (ADR-0174). A read waits the session's delay. The next write, and the
    /// next read, fail once each as a database that is down fails: the
    /// operation answers no value, and the component that called it traps.
    pub(crate) fn faulted(
        &self,
        session: &str,
        mut host: BTreeMap<String, HostFn>,
    ) -> BTreeMap<String, HostFn> {
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
            let faults = self.cart_faults.clone();
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

    /// **The deployment's data layer: `store:data/carts`, whole.**
    ///
    /// Every operation the interface declares, whichever command is running;
    /// the host links only those the component imports and was granted. Each
    /// write stages the session's new lines in `staged`, and nothing here
    /// commits. A command's `Ok` does that, in [`Server::command`]. The
    /// session an operation is passed must be the one the host gave the
    /// component, or the component is acting for someone else.
    pub(crate) fn data_layer(
        session: &str,
        current: Lines,
        staged: Arc<Mutex<Option<Lines>>>,
        fail: bool,
        catalog: Arc<Catalog>,
    ) -> BTreeMap<String, HostFn> {
        let expired = || {
            vec![Val::Result(Err(Some(Box::new(Val::Variant(
                "cart-expired".into(),
                None,
            )))))]
        };
        let op = |name: &'static str,
                  f: fn(&mut Lines, &[Val], &Catalog) -> Result<(), String>|
         -> (String, HostFn) {
            let this_session = session.to_string();
            let current = current.clone();
            let staged = staged.clone();
            let catalog = catalog.clone();
            let run: HostFn = Arc::new(move |args: &[Val]| {
                let Some(Val::String(s)) = args.first() else {
                    return Err(format!("carts#{name} received {args:?}"));
                };
                if *s != this_session {
                    return Err(format!("carts#{name} was passed another session"));
                }
                if fail {
                    return Ok(expired());
                }
                let mut staged = staged.lock().expect("staged");
                let mut lines = staged.clone().unwrap_or_else(|| current.clone());
                f(&mut lines, &args[1..], &catalog)?;
                let cart = cart_value(&lines);
                if name != "current" {
                    *staged = Some(lines);
                }
                Ok(vec![Val::Result(Ok(Some(Box::new(cart))))])
            });
            (format!("store:data/carts#{name}"), run)
        };
        BTreeMap::from([
            op("add", |lines, args, catalog| {
                let [Val::String(item), Val::S64(quantity)] = args else {
                    return Err(format!("carts#add received {args:?}"));
                };
                match lines.iter_mut().find(|l| l.item == *item) {
                    Some(line) => line.quantity += quantity,
                    None => {
                        // The line records its item as it is now (ADR-0172).
                        let (name, price) = catalog
                            .get(item)
                            .cloned()
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
            }),
            // One fewer; a line that holds one goes (ADR-0172). A line that
            // is not there is not: another page took it away first.
            op("decrease", |lines, args, _| {
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
            }),
            // The line, gone (ADR-0172).
            op("remove", |lines, args, _| {
                let [Val::String(item)] = args else {
                    return Err(format!("carts#remove received {args:?}"));
                };
                lines.retain(|l| l.item != *item);
                Ok(())
            }),
            op("clear", |lines, args, _| {
                if !args.is_empty() {
                    return Err(format!("carts#clear received {args:?}"));
                }
                lines.clear();
                Ok(())
            }),
            op("current", |_, args, _| {
                if !args.is_empty() {
                    return Err(format!("carts#current received {args:?}"));
                }
                Ok(())
            }),
        ])
    }

    /// **The store's data layer for orders, as a command sees it**
    /// (ADR-0193): `orders#place` places the session's cart as its order.
    /// The order is the cart's lines and the status `placed`, staged as
    /// `placed`, and the cart is staged empty, so the two commit together,
    /// in the command's commit. An empty cart places nothing, and says so.
    /// Until 2026-10-08 the order kept its status alone, and its lines went
    /// with the emptied cart.
    pub(crate) fn order_layer(
        session: &str,
        current: Lines,
        staged: Arc<Mutex<Option<Lines>>>,
        placed: Arc<Mutex<Option<Order>>>,
    ) -> BTreeMap<String, HostFn> {
        let this_session = session.to_string();
        let place: HostFn = Arc::new(move |args: &[Val]| {
            let [Val::String(s)] = args else {
                return Err(format!("orders#place received {args:?}"));
            };
            if *s != this_session {
                return Err("orders#place was passed another session".to_string());
            }
            let mut staged = staged.lock().expect("staged");
            let lines = staged.clone().unwrap_or_else(|| current.clone());
            if lines.is_empty() {
                return Ok(vec![Val::Result(Err(Some(Box::new(Val::Variant(
                    "nothing-to-order".into(),
                    None,
                )))))]);
            }
            *staged = Some(Lines::new());
            *placed.lock().expect("placed") = Some(Order {
                status: "placed".to_string(),
                lines,
            });
            Ok(vec![Val::Result(Ok(Some(Box::new(Val::Variant(
                "placed".into(),
                None,
            )))))])
        });
        BTreeMap::from([("store:data/orders#place".to_string(), place)])
    }

    /// **The deployment's catalogue** (ADR-0125): `store:data/stores#get` and
    /// `store:data/menus#sections`, what the `Store` and `Menu` queries read,
    /// and `menus#for-store`, the benchmark's own store's menu (ADR-0156,
    /// ADR-0181).
    /// This server holds two stores (ADR-0162): 47, whose menu is the keyed
    /// list E7-P mutates, and 48. Any other is answered `not-found`.
    ///
    /// Its slow sources poll `stopped`, to end early a read nobody is
    /// waiting for (ADR-0152).
    pub(crate) fn catalog_within(&self, stopped: Option<Stopped>) -> BTreeMap<String, HostFn> {
        let menu = self.menu.lock().expect("menu").clone();
        let not_found = || {
            vec![Val::Result(Err(Some(Box::new(Val::Variant(
                "not-found".into(),
                None,
            )))))]
        };
        // Charter §15.5's store delay (ADR-0174), and its origin failing
        // once (ADR-0177), as a test set them.
        let store_delay = self.store_delay_ms.clone();
        let store_fails = self.store_fails_next.clone();
        let get: HostFn = Arc::new(move |args: &[Val]| match args {
            [Val::String(id)] if store_named(id).is_some() => {
                if store_fails.swap(false, std::sync::atomic::Ordering::SeqCst) {
                    return Err("stores#get: the store's origin is unavailable".to_string());
                }
                let delay = store_delay.load(std::sync::atomic::Ordering::SeqCst);
                if delay > 0 {
                    std::thread::sleep(std::time::Duration::from_millis(delay));
                }
                Ok(vec![Val::Result(Ok(Some(Box::new(store_record(id)))))])
            }
            [Val::String(_)] => Ok(not_found()),
            other => Err(format!("stores#get received {other:?}")),
        });
        // Every store this server holds, in its order (ADR-0192): what the
        // home page lists.
        let list: HostFn = Arc::new(move |args: &[Val]| match args {
            [] => Ok(vec![Val::List(
                [STORE_ID, SECOND_STORE.0]
                    .into_iter()
                    .map(store_record)
                    .collect(),
            )]),
            other => Err(format!("stores#list received {other:?}")),
        });
        // What can be ordered now (ADR-0178), as the menu's rows say.
        let sold_out = self.sold_out.lock().expect("sold out").clone();
        // A store's items, each as the data layer holds it: read through the
        // program's `MenuItem` (ADR-0166), so the benchmark's, which declares
        // fewer fields, is passed what it declares.
        let rows_of = move |store: &str| -> Vec<(String, Val)> {
            let items = match store {
                STORE_ID => menu.clone(),
                _ => second_menu(),
            };
            items
                .iter()
                .map(|(id, name)| {
                    let (category, category_name) = item_category(store, id);
                    let row = Val::Record(vec![
                        ("id".into(), Val::String(id.clone())),
                        // Its store, and its category (ADR-0181).
                        ("store-id".into(), Val::String(store.to_string())),
                        ("name".into(), Val::String(name.clone())),
                        (
                            "description".into(),
                            Val::String(item_description(id).into()),
                        ),
                        // Its price, in cents (ADR-0169).
                        (
                            "price".into(),
                            Val::Record(vec![("minor-units".into(), Val::S64(item_price(id)))]),
                        ),
                        // Whether it can be ordered now (ADR-0178): what
                        // `menus#is-available` answers.
                        ("available".into(), Val::Bool(!sold_out.contains(id))),
                        (
                            "category".into(),
                            Val::Record(vec![
                                ("id".into(), Val::String(category.into())),
                                ("name".into(), Val::String(category_name.into())),
                            ]),
                        ),
                    ]);
                    (category.to_string(), row)
                })
                .collect()
        };
        let rows_of = Arc::new(rows_of);
        let rows = rows_of.clone();
        let for_store: HostFn = Arc::new(move |args: &[Val]| match args {
            [Val::String(id)] if store_named(id).is_some() => {
                Ok(vec![Val::Result(Ok(Some(Box::new(Val::List(
                    rows(id).into_iter().map(|(_, row)| row).collect(),
                )))))])
            }
            [Val::String(_)] => Ok(not_found()),
            other => Err(format!("menus#for-store received {other:?}")),
        });
        // **The menu, grouped by category** (ADR-0181): each category in the
        // order its first item is listed, with its items in theirs.
        let sections: HostFn = Arc::new(move |args: &[Val]| match args {
            [Val::String(id)] if store_named(id).is_some() => {
                let mut grouped: Vec<(String, Val, Vec<Val>)> = Vec::new();
                for (category, row) in rows_of(id) {
                    let Val::Record(fields) = &row else { continue };
                    let named = fields
                        .iter()
                        .find(|(n, _)| n == "category")
                        .map(|(_, v)| v.clone())
                        .unwrap_or(Val::Record(Vec::new()));
                    match grouped.iter_mut().find(|(c, ..)| *c == category) {
                        Some((_, _, items)) => items.push(row),
                        None => grouped.push((category, named, vec![row])),
                    }
                }
                Ok(vec![Val::Result(Ok(Some(Box::new(Val::List(
                    grouped
                        .into_iter()
                        .map(|(_, category, items)| {
                            Val::Record(vec![
                                ("category".into(), category),
                                ("items".into(), Val::List(items)),
                            ])
                        })
                        .collect(),
                )))))])
            }
            [Val::String(_)] => Ok(not_found()),
            other => Err(format!("menus#sections received {other:?}")),
        });
        // What the recommender answers (ADR-0148), after its delay: a slow
        // source, which a streamed region does not wait for.
        let recommender = self.recommender.lock().expect("recommender").clone();
        // Store 47's menu as it is now, which its recommendations are drawn
        // from (ADR-0165).
        let suggested_from = self.menu.lock().expect("menu").clone();
        let recommend: HostFn = Arc::new(move |args: &[Val]| {
            let [Val::String(store)] = args else {
                return Err(format!("recommendations#for-store received {args:?}"));
            };
            // Held until a test lets it go (ADR-0223), then slow.
            if let Some(gate) = &recommender.gate {
                gate.wait();
            }
            std::thread::sleep(std::time::Duration::from_millis(recommender.delay_ms));
            let items = match (&recommender.items, store.as_str()) {
                (Some(items), _) => items.clone(),
                (None, STORE_ID) => recommended_from(&suggested_from),
                (None, id) if store_named(id).is_some() => recommended_from(&second_menu()),
                (None, _) => Vec::new(),
            };
            match recommender.fail.as_deref() {
                Some("host") => Err("the recommender is down".to_string()),
                Some(_) => Ok(vec![Val::Result(Err(Some(Box::new(Val::Variant(
                    "none-available".into(),
                    None,
                )))))]),
                None => Ok(vec![Val::Result(Ok(Some(Box::new(Val::List(
                    items
                        .iter()
                        .map(|(id, name)| {
                            Val::Record(vec![
                                ("id".into(), Val::String(id.clone())),
                                ("name".into(), Val::String(name.clone())),
                            ])
                        })
                        .collect(),
                )))))]),
            }
        });
        // What the store has posted (E14, T09). Every call is counted, as
        // every data-layer call is: what `/bench/calls` reports.
        let posted = self.notice.lock().expect("notice").clone();
        let notice: HostFn = Arc::new(move |args: &[Val]| match args {
            [Val::String(id)] if id == STORE_ID => Ok(vec![Val::Result(Ok(Some(Box::new(
                Val::String(posted.clone()),
            ))))]),
            [Val::String(_)] => Ok(vec![Val::Result(Err(Some(Box::new(Val::Variant(
                "not-found".into(),
                None,
            )))))]),
            other => Err(format!("notices#current received {other:?}")),
        });
        // How long the kitchen takes now (E14, T02), after [`KITCHEN_MS`]:
        // a slow source. The minutes are the ones it was asked with.
        let minutes = *self.prep_minutes.lock().expect("prep minutes");
        let prep: HostFn = Arc::new(move |args: &[Val]| match args {
            [Val::String(id)] if id == STORE_ID => {
                std::thread::sleep(std::time::Duration::from_millis(KITCHEN_MS));
                Ok(vec![Val::Result(Ok(Some(Box::new(Val::S64(minutes)))))])
            }
            [Val::String(_)] => Ok(not_found()),
            other => Err(format!("kitchen#prep-minutes received {other:?}")),
        });
        // The menu's items in a category (E14, T07), the slow category after
        // its delay. A read that is stopped meanwhile ends early, and says so:
        // what `/bench/calls` counts as stopped.
        let categories = self.categories.lock().expect("categories").clone();
        let in_menu = self.menu.lock().expect("menu").clone();
        let ended_early = self.category_stopped.clone();
        let in_category: HostFn = Arc::new(move |args: &[Val]| match args {
            [Val::String(id), Val::String(category)] if id == STORE_ID => {
                if categories.slow.as_deref() == Some(category.as_str()) {
                    let until = std::time::Instant::now()
                        + std::time::Duration::from_millis(categories.delay_ms);
                    while std::time::Instant::now() < until {
                        if stopped.as_ref().is_some_and(|s| s()) {
                            ended_early.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                            return Err("stopped: nobody is waiting for it".to_string());
                        }
                        std::thread::sleep(std::time::Duration::from_millis(10));
                    }
                }
                Ok(vec![Val::Result(Ok(Some(Box::new(Val::List(
                    in_menu
                        .iter()
                        .filter(|(item, _)| {
                            category.as_str() == "all" || category_of(item) == category.as_str()
                        })
                        .map(|(id, name)| {
                            Val::Record(vec![
                                ("id".into(), Val::String(id.clone())),
                                ("name".into(), Val::String(name.clone())),
                            ])
                        })
                        .collect(),
                )))))])
            }
            [Val::String(_), Val::String(_)] => Ok(not_found()),
            other => Err(format!("menus#in-category received {other:?}")),
        });
        BTreeMap::from([
            ("store:data/stores#get".to_string(), get),
            ("store:data/stores#list".to_string(), list),
            ("store:data/menus#for-store".to_string(), for_store),
            ("store:data/menus#sections".to_string(), sections),
            ("store:data/notices#current".to_string(), notice),
            ("store:data/kitchen#prep-minutes".to_string(), prep),
            ("store:data/menus#in-category".to_string(), in_category),
            (
                "store:data/recommendations#for-store".to_string(),
                recommend,
            ),
        ])
    }

    /// **The store's reads, for one session** (ADR-0218): its data layer as a
    /// query sees it, with the faults a test set, the session's order and its
    /// delivery estimate, and the catalogue. Nothing a query does commits.
    pub(crate) fn reads(
        &self,
        session: &str,
        stopped: Option<Stopped>,
    ) -> BTreeMap<String, HostFn> {
        let current = self
            .carts
            .lock()
            .expect("carts")
            .get(session)
            .cloned()
            .unwrap_or_default();
        let mut host = self.faulted(
            session,
            Self::data_layer(session, current, Arc::default(), false, self.catalog()),
        );
        // The session's order, as its status's case (E14, T04). A case the
        // program's type does not have is refused by the component's types,
        // as any value the host gives is.
        let order = self.orders.lock().expect("orders").get(session).cloned();
        // And the lines it was placed with (ADR-0193), a cart's value, or
        // none before one is placed.
        host.insert(
            "store:data/orders#placed".to_string(),
            placed_op(session, order.clone().map(|o| o.lines)),
        );
        let order = order.map(|o| o.status);
        let this_session = session.to_string();
        host.insert(
            "store:data/orders#current".to_string(),
            Arc::new(move |args: &[Val]| {
                let Some(Val::String(s)) = args.first() else {
                    return Err(format!("orders#current received {args:?}"));
                };
                if *s != this_session {
                    return Err("orders#current was passed another session".to_string());
                }
                let status = order.clone().map(|case| Box::new(Val::Variant(case, None)));
                Ok(vec![Val::Result(Ok(Some(Box::new(Val::Option(status)))))])
            }),
        );
        // The session's delivery estimate (E14, T10), after the estimator's
        // delay: a slow source, which a streamed region does not wait for.
        let estimator = self
            .estimators
            .lock()
            .expect("estimators")
            .get(session)
            .cloned()
            .unwrap_or_default();
        let this_session = session.to_string();
        host.insert(
            "store:data/estimates#current".to_string(),
            Arc::new(move |args: &[Val]| {
                let Some(Val::String(s)) = args.first() else {
                    return Err(format!("estimates#current received {args:?}"));
                };
                if *s != this_session {
                    return Err("estimates#current was passed another session".to_string());
                }
                std::thread::sleep(std::time::Duration::from_millis(estimator.delay_ms));
                match estimator.fail.as_deref() {
                    Some("declared") => Ok(vec![Val::Result(Err(Some(Box::new(Val::Variant(
                        "location-unavailable".into(),
                        None,
                    )))))]),
                    Some(_) => Err("the estimator is down".to_string()),
                    // A range, and when it was made (ADR-0180). And `minutes`,
                    // which the benchmark's own store still reads (ADR-0156):
                    // a program is passed the fields its type declares
                    // (ADR-0166).
                    None => Ok(vec![Val::Result(Ok(Some(Box::new(Val::Record(vec![
                        ("minutes".into(), Val::S64(estimator.minutes)),
                        ("min-minutes".into(), Val::S64(estimator.minutes)),
                        (
                            "max-minutes".into(),
                            Val::S64(estimator.max_minutes.unwrap_or(estimator.minutes + 10)),
                        ),
                        ("generated-at".into(), Val::S64(wall_millis())),
                    ])))))]),
                }
            }),
        );
        host.extend(self.catalog_within(stopped));
        host
    }

    /// **A command's writes, staged until its transaction commits**
    /// (ADR-0218): the session's cart and the order it places, read and
    /// written under one lock, which the staging holds until it is dropped.
    /// `fail` makes the data layer answer `cart-expired` (a test's).
    pub(crate) fn begin(&self, session: &str, fail: bool) -> Staging<'_> {
        let carts = self.carts.lock().expect("carts");
        let current = carts.get(session).cloned().unwrap_or_default();
        let staged: Arc<Mutex<Option<Lines>>> = Arc::default();
        // The order a command places (ADR-0193), staged with the cart it
        // empties, and committed with it.
        let placed: Arc<Mutex<Option<Order>>> = Arc::default();
        let order = Self::order_layer(session, current.clone(), staged.clone(), placed.clone());
        let mut host = self.faulted(
            session,
            Self::data_layer(session, current, staged.clone(), fail, self.catalog()),
        );
        host.extend(order);
        // Whether an item can be ordered now (charter §15.4), read inside
        // the command that adds it. One no store's menu has is not
        // (ADR-0172): a page sends the item it showed, and a request can
        // name any.
        let sold_out = self.sold_out.lock().expect("sold out").clone();
        let catalog = self.catalog();
        host.insert(
            "store:data/menus#is-available".to_string(),
            Arc::new(move |args: &[Val]| match args {
                [Val::String(item)] => Ok(vec![Val::Bool(
                    catalog.contains_key(item) && !sold_out.contains(item),
                )]),
                other => Err(format!("menus#is-available received {other:?}")),
            }),
        );
        Staging {
            store: self,
            session: session.to_string(),
            carts,
            staged,
            placed,
            ops: host,
        }
    }
}

/// **One command's writes, staged until its transaction commits** (ADR-0218).
pub(crate) struct Staging<'a> {
    store: &'a StoreData,
    session: String,
    /// Held from the call until the commit is published.
    carts: std::sync::MutexGuard<'a, BTreeMap<String, Lines>>,
    staged: Arc<Mutex<Option<Lines>>>,
    /// The order a command places (ADR-0193), staged with the cart it
    /// empties, and committed with it.
    placed: Arc<Mutex<Option<Order>>>,
    ops: BTreeMap<String, HostFn>,
}

impl Staging<'_> {
    /// The data layer's operations for the command: its reads, and its writes,
    /// which stage.
    pub(crate) fn ops(&self) -> BTreeMap<String, HostFn> {
        self.ops.clone()
    }

    /// **What the commit writes in the transaction**, by key, or none where
    /// nothing was written: the session's cart's total.
    pub(crate) fn rows(&self) -> Option<Vec<(String, String)>> {
        let staged = self.staged.lock().expect("staged");
        let lines = staged.as_ref()?;
        let total: i64 = lines.iter().map(|l| l.quantity).sum();
        Some(vec![(format!("cart:{}", self.session), total.to_string())])
    }

    /// **After the commit, what was staged is the store's**: the cart, and the
    /// order it placed, before what reads them is dropped and read again.
    pub(crate) fn publish(&mut self) {
        if let Some(lines) = self.staged.lock().expect("staged").take() {
            self.carts.insert(self.session.clone(), lines);
        }
        if let Some(order) = self.placed.lock().expect("placed").take() {
            self.store
                .orders
                .lock()
                .expect("orders")
                .insert(self.session.clone(), order);
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

/// **`store:data/orders#placed`** (ADR-0193): the lines the session's order
/// was placed with, as a cart's value, or none before one is placed. No
/// program reads it yet; the store's tests read an order through it.
pub(crate) fn placed_op(session: &str, lines: Option<Lines>) -> HostFn {
    let this_session = session.to_string();
    Arc::new(move |args: &[Val]| {
        let [Val::String(s)] = args else {
            return Err(format!("orders#placed received {args:?}"));
        };
        if *s != this_session {
            return Err("orders#placed was passed another session".to_string());
        }
        let cart = lines.as_deref().map(|l| Box::new(cart_value(l)));
        Ok(vec![Val::Option(cart)])
    })
}

impl crate::data::Staged for Staging<'_> {
    fn ops(&self) -> crate::data::Ops {
        Staging::ops(self)
    }

    fn rows(&self) -> Option<Vec<(String, String)>> {
        Staging::rows(self)
    }

    fn publish(&mut self) {
        Staging::publish(self)
    }
}

impl crate::data::DataLayer for StoreData {
    fn reads(&self, session: &str, stopped: Option<Stopped>) -> crate::data::Ops {
        StoreData::reads(self, session, stopped)
    }

    fn begin<'a>(&'a self, session: &str) -> Box<dyn crate::data::Staged + 'a> {
        let fail = self
            .fail_next
            .swap(false, std::sync::atomic::Ordering::SeqCst);
        Box::new(StoreData::begin(self, session, fail))
    }

    fn grants(&self) -> Vec<&'static str> {
        StoreData::grants().to_vec()
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

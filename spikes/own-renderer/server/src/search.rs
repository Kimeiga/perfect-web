//! **The store's search** (track `store-search`, docs/PARALLEL.md, W9): the
//! stores and their dishes that match a reader's text, matched in the
//! store's data layer, in memory and on PostgreSQL, to one meaning.
//!
//! - **The terms** are the text's whitespace-separated words, each lowered
//!   by [`lower`], here, once, whichever layer matches them. Text with no
//!   term is [`Asked::Blank`]; text past [`MOST_CHARACTERS`] is
//!   [`Asked::TooLong`]. Both are the program's declared errors.
//! - **A record matches** where every term is part of one of its fields:
//!   a store's name, cuisine or description; a dish's name, category or
//!   description. Each field is lowered as the terms are: in memory by
//!   [`lower`], on PostgreSQL by `lower(.. COLLATE pg_c_utf8)`, Unicode's
//!   simple case mapping both. No accent and no stem is folded.
//! - **Its tier** says where its terms were found: 0, every term in its name;
//!   1, each in its name or its second field (a store's cuisine, a dish's
//!   category); 2, each in any of its three.
//! - **Each layer answers what matched, with its tier** ([`Matched`]): in
//!   memory, [`scan`] over the layer's rows; on PostgreSQL, `store_pg.rs`'s
//!   statements, each term a bound `LIKE` pattern with `%`, `_` and `\`
//!   escaped ([`pattern`]). Everything after is this file's, so the two
//!   layers cannot rank differently: [`found`] orders and cuts the stores
//!   and their dishes, and writes the program's value.
//!
//! **A store's cuisine** is search's own: [`cuisine_of`] in memory, the table
//! `store_cuisines` on PostgreSQL (`migrations/store/0010_search.sql`), the
//! same rows. The store's own rows are left as they are.

use super::*;
use crate::data::Ops;
use crate::store::{Item, Rows, With};

/// **The longest text a search takes**, in characters (Unicode scalar
/// values), as it is given, before it is trimmed.
pub(crate) const MOST_CHARACTERS: usize = 100;

/// **The most stores a search lists**; the rest are counted.
pub(crate) const MOST_STORES: usize = 20;

/// **The most dishes a store lists under it**; the rest are counted.
pub(crate) const MOST_DISHES: usize = 5;

/// **What a search's text asks for.**
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Asked {
    /// No term: nothing to search for.
    Blank,
    /// Longer than [`MOST_CHARACTERS`].
    TooLong,
    /// Its terms, each lowered, in the order written.
    Terms(Vec<String>),
}

/// **The terms of `text`**: its words, split at Unicode's white space and
/// each lowered.
pub(crate) fn asked(text: &str) -> Asked {
    if text.chars().count() > MOST_CHARACTERS {
        return Asked::TooLong;
    }
    let terms: Vec<String> = text.split_whitespace().map(lower).collect();
    if terms.is_empty() {
        return Asked::Blank;
    }
    Asked::Terms(terms)
}

/// **Unicode 16.0's simple lowercase mapping**, a character at a time: what
/// PostgreSQL 18's `lower` does under `pg_c_utf8`. A character whose full
/// mapping is longer (`İ`, U+0130, to `i` and U+0307) is given its simple
/// mapping, the full one's first character, so a field never grows. Rust's
/// tables are Unicode 17.0's: a character 17.0 gave a case that 16.0 did not
/// ([`NEWER_THAN_POSTGRES`]) is kept as it is, as PostgreSQL keeps it.
pub(crate) fn lower(text: &str) -> String {
    text.chars()
        .map(|c| {
            if NEWER_THAN_POSTGRES
                .iter()
                .any(|r| r.contains(&u32::from(c)))
            {
                return c;
            }
            c.to_lowercase().next().unwrap_or(c)
        })
        .collect()
}

/// **The capitals Unicode 17.0 added that 16.0 did not have**, and so no
/// lowercase in PostgreSQL 18's tables: three Latin letters and Beria Erfe's
/// capitals. Measured, character by character, by the test
/// `on_postgres_every_character_is_lowered_alike`, which fails when either
/// side's tables move.
const NEWER_THAN_POSTGRES: [std::ops::RangeInclusive<u32>; 4] = [
    0xA7CE..=0xA7CE,
    0xA7D2..=0xA7D2,
    0xA7D4..=0xA7D4,
    0x16EA0..=0x16EB8,
];

/// **A term as PostgreSQL's `LIKE` matches it anywhere in a field**, with
/// `ESCAPE '\'`: each `\`, `%` and `_` it holds escaped, so it is matched as
/// written, and `%` either side.
pub(crate) fn pattern(term: &str) -> String {
    let mut p = String::with_capacity(term.len() + 2);
    p.push('%');
    for c in term.chars() {
        if matches!(c, '\\' | '%' | '_') {
            p.push('\\');
        }
        p.push(c);
    }
    p.push('%');
    p
}

/// **Where a record's terms were found**, from its three fields lowered,
/// its name first: 0, every term in its name; 1, each in its name or its
/// second; 2, each in any; `None`, some term in none.
pub(crate) fn tier(terms: &[String], fields: [&str; 3]) -> Option<u8> {
    let in_first = |t: &String, n: usize| fields[..n].iter().any(|f| f.contains(t.as_str()));
    (1..=3)
        .find(|&n| terms.iter().all(|t| in_first(t, n)))
        .map(|n| n as u8 - 1)
}

/// **A dish that matched**, and its tier.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Dish {
    pub(crate) id: String,
    pub(crate) name: String,
    pub(crate) description: String,
    pub(crate) category: String,
    /// In cents (ADR-0169).
    pub(crate) price: i64,
    pub(crate) tier: u8,
}

/// **A store a search lists**: its own tier where it matched, and its dishes
/// that matched. A layer answers a store only where one of the two is so.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Matched {
    pub(crate) id: String,
    pub(crate) name: String,
    pub(crate) description: String,
    pub(crate) cuisine: String,
    pub(crate) tier: Option<u8>,
    pub(crate) dishes: Vec<Dish>,
}

/// **Each store's cuisine, as the store is first**: the in-memory layer's,
/// as `migrations/store/0010_search.sql` is PostgreSQL's.
pub(crate) fn cuisine_of(store: &str) -> &'static str {
    match store {
        "47" => "Coffee",
        "48" => "Café, Bakery",
        _ => "",
    }
}

/// **A store's own text**, as the search reads it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct StoreText {
    pub(crate) id: String,
    pub(crate) name: String,
    pub(crate) description: String,
    pub(crate) cuisine: String,
}

/// **The in-memory layer's search**: every store the layer holds and every
/// dish of its menu, held to `terms` by [`matched`].
pub(crate) fn scan<R: Rows + ?Sized>(r: &mut R, terms: &[String]) -> Result<Vec<Matched>, String> {
    let mut stores = Vec::new();
    for s in r.stores()? {
        let menu = r.menu(&s.id)?;
        let cuisine = cuisine_of(&s.id).to_string();
        stores.push((
            StoreText {
                id: s.id,
                name: s.name,
                description: s.description,
                cuisine,
            },
            menu,
        ));
    }
    Ok(matched(terms, stores))
}

/// **What `terms` match among `stores`**, each with its menu: each store
/// that matches or holds a dish that does, its tier and those dishes'.
pub(crate) fn matched(terms: &[String], stores: Vec<(StoreText, Vec<Item>)>) -> Vec<Matched> {
    let mut out = Vec::new();
    for (store, menu) in stores {
        let tier_of_store = tier(
            terms,
            [
                &lower(&store.name),
                &lower(&store.cuisine),
                &lower(&store.description),
            ],
        );
        let dishes: Vec<Dish> = menu
            .into_iter()
            .filter_map(|i| {
                let t = tier(
                    terms,
                    [
                        &lower(&i.name),
                        &lower(&i.category_name),
                        &lower(&i.description),
                    ],
                )?;
                Some(Dish {
                    id: i.id,
                    name: i.name,
                    description: i.description,
                    category: i.category_name,
                    price: i.price,
                    tier: t,
                })
            })
            .collect();
        if tier_of_store.is_some() || !dishes.is_empty() {
            out.push(Matched {
                id: store.id,
                name: store.name,
                description: store.description,
                cuisine: store.cuisine,
                tier: tier_of_store,
                dishes,
            });
        }
    }
    out
}

/// **A store's rank**: its own tier where it matched; past every such
/// store, 3 and its best dish's tier.
fn rank(m: &Matched) -> u8 {
    match m.tier {
        Some(t) => t,
        None => 3 + m.dishes.iter().map(|d| d.tier).min().unwrap_or(2),
    }
}

/// **The stores in their order, each with its dishes in theirs**: by rank,
/// then name, then id, each name compared code point by code point.
pub(crate) fn ordered(mut matched: Vec<Matched>) -> Vec<Matched> {
    for m in &mut matched {
        m.dishes
            .sort_by(|a, b| (a.tier, &a.name, &a.id).cmp(&(b.tier, &b.name, &b.id)));
    }
    matched.sort_by(|a, b| (rank(a), &a.name, &a.id).cmp(&(rank(b), &b.name, &b.id)));
    matched
}

/// **What a search found, as the program's `SearchFound` is**: the stores in
/// their order, the first [`MOST_STORES`], each with its first
/// [`MOST_DISHES`] dishes, and how many of each are left.
pub(crate) fn found(matched: Vec<Matched>) -> Val {
    let ordered = ordered(matched);
    let more_stores = ordered.len().saturating_sub(MOST_STORES);
    let stores = ordered
        .into_iter()
        .take(MOST_STORES)
        .map(|m| {
            let more_dishes = m.dishes.len().saturating_sub(MOST_DISHES);
            let dishes = m
                .dishes
                .into_iter()
                .take(MOST_DISHES)
                .map(|d| {
                    Val::Record(vec![
                        ("id".into(), Val::String(d.id)),
                        ("name".into(), Val::String(d.name)),
                        ("description".into(), Val::String(d.description)),
                        ("category".into(), Val::String(d.category)),
                        (
                            "price".into(),
                            Val::Record(vec![("minor-units".into(), Val::S64(d.price))]),
                        ),
                    ])
                })
                .collect();
            Val::Record(vec![
                ("id".into(), Val::String(m.id)),
                ("name".into(), Val::String(m.name)),
                ("description".into(), Val::String(m.description)),
                ("cuisine".into(), Val::String(m.cuisine)),
                ("dishes".into(), Val::List(dishes)),
                ("more-dishes".into(), Val::S64(more_dishes as i64)),
            ])
        })
        .collect();
    Val::Record(vec![
        ("stores".into(), Val::List(stores)),
        ("more-stores".into(), Val::S64(more_stores as i64)),
    ])
}

/// **The search's operation** (`store:data/search#find`), over `with`, the
/// layer's rows: the text's terms matched by the layer's
/// [`Rows::search`], or the declared error the text is.
pub(crate) fn ops(with: With) -> Ops {
    let mut ops: Ops = BTreeMap::new();
    ops.insert(
        "store:data/search#find".to_string(),
        Arc::new(move |args: &[Val]| {
            let [Val::String(text)] = args else {
                return Err(format!("search#find received {args:?}"));
            };
            let case = |c: &str| {
                vec![Val::Result(Err(Some(Box::new(Val::Variant(
                    c.into(),
                    None,
                )))))]
            };
            match asked(text) {
                Asked::Blank => Ok(case("blank")),
                Asked::TooLong => Ok(case("too-long")),
                Asked::Terms(terms) => with(&mut |r| {
                    let matched = r.search(&terms)?;
                    Ok(vec![Val::Result(Ok(Some(Box::new(found(matched)))))])
                }),
            }
        }),
    );
    ops
}

/// **PostgreSQL's search** (`store_pg.rs`'s `Rows::search`): two statements
/// on `c`, in whatever transaction it is in. The dishes that match, then the
/// stores that match or hold one. Every term is a bound `LIKE` pattern
/// ([`pattern`]), never written into the statement.
pub(crate) fn on_postgres(
    c: &mut postgres::Client,
    terms: &[String],
) -> Result<Vec<Matched>, String> {
    let patterns: Vec<String> = terms.iter().map(|t| pattern(t)).collect();
    let pg = crate::feed_pg::pg;
    let dishes = c
        .query(DISHES_SQL, &[&patterns])
        .map_err(pg)?
        .iter()
        .map(|r| {
            let store: String = r.get(0);
            let tier: i32 = r.get(6);
            (
                store,
                Dish {
                    id: r.get(1),
                    name: r.get(2),
                    description: r.get(3),
                    category: r.get(4),
                    price: r.get(5),
                    tier: tier as u8,
                },
            )
        })
        .collect::<Vec<_>>();
    let holding: Vec<String> = dishes.iter().map(|(s, _)| s.clone()).collect();
    let stores = c.query(STORES_SQL, &[&patterns, &holding]).map_err(pg)?;
    Ok(stores
        .iter()
        .map(|r| {
            let id: String = r.get(0);
            let tier: Option<i32> = r.get(4);
            Matched {
                dishes: dishes
                    .iter()
                    .filter(|(s, _)| *s == id)
                    .map(|(_, d)| d.clone())
                    .collect(),
                id,
                name: r.get(1),
                description: r.get(2),
                cuisine: r.get(3),
                tier: tier.map(|t| t as u8),
            }
        })
        .collect())
}

/// Every dish that matches `$1`, the terms' patterns, and its tier.
const DISHES_SQL: &str = "\
    SELECT store, id, name, description, category, price, tier FROM (\
      SELECT i.store, i.id, i.name, i.description, c.name AS category, i.price, \
        CASE \
          WHEN (SELECT bool_and(lower(i.name COLLATE pg_c_utf8) LIKE p ESCAPE '\\') \
                FROM unnest($1::text[]) AS p) THEN 0 \
          WHEN (SELECT bool_and(lower(i.name COLLATE pg_c_utf8) LIKE p ESCAPE '\\' \
                             OR lower(c.name COLLATE pg_c_utf8) LIKE p ESCAPE '\\') \
                FROM unnest($1::text[]) AS p) THEN 1 \
          WHEN (SELECT bool_and(lower(i.name COLLATE pg_c_utf8) LIKE p ESCAPE '\\' \
                             OR lower(c.name COLLATE pg_c_utf8) LIKE p ESCAPE '\\' \
                             OR lower(i.description COLLATE pg_c_utf8) LIKE p ESCAPE '\\') \
                FROM unnest($1::text[]) AS p) THEN 2 \
        END AS tier \
      FROM menu_items i JOIN categories c ON c.store = i.store AND c.id = i.category\
    ) AS d WHERE tier IS NOT NULL";

/// Every store that matches `$1`, or is among `$2`, the stores holding a
/// dish that does, with its cuisine and its tier where it matched itself.
const STORES_SQL: &str = "\
    SELECT id, name, description, cuisine, tier FROM (\
      SELECT s.id, s.name, s.description, coalesce(k.cuisine, '') AS cuisine, \
        CASE \
          WHEN (SELECT bool_and(lower(s.name COLLATE pg_c_utf8) LIKE p ESCAPE '\\') \
                FROM unnest($1::text[]) AS p) THEN 0 \
          WHEN (SELECT bool_and(lower(s.name COLLATE pg_c_utf8) LIKE p ESCAPE '\\' \
                             OR lower(coalesce(k.cuisine, '') COLLATE pg_c_utf8) LIKE p ESCAPE '\\') \
                FROM unnest($1::text[]) AS p) THEN 1 \
          WHEN (SELECT bool_and(lower(s.name COLLATE pg_c_utf8) LIKE p ESCAPE '\\' \
                             OR lower(coalesce(k.cuisine, '') COLLATE pg_c_utf8) LIKE p ESCAPE '\\' \
                             OR lower(s.description COLLATE pg_c_utf8) LIKE p ESCAPE '\\') \
                FROM unnest($1::text[]) AS p) THEN 2 \
        END AS tier \
      FROM stores s LEFT JOIN store_cuisines k ON k.store = s.id\
    ) AS m WHERE tier IS NOT NULL OR id = ANY($2::text[])";

#[cfg(test)]
mod tests {
    //! **One meaning on both layers** (docs/PARALLEL.md, W9's rule 2): the
    //! same fixture, the same table of searches, answered in memory and on
    //! PostgreSQL where `PW_STORE_DATABASE_URL` names a database.

    use super::*;

    /// A fixture's item: its id, name, category and description.
    fn item(id: &str, name: &str, category: &str, description: &str) -> Item {
        Item {
            id: id.to_string(),
            name: name.to_string(),
            description: description.to_string(),
            price: 400,
            category: lower(category).replace(' ', "-"),
            category_name: category.to_string(),
            served: "hot".to_string(),
            available: true,
        }
    }

    /// **The fixture**: names with accents, Greek, a sharp s, a dotted
    /// capital I, `%`, `_`, `\` and quotes, two stores of one name, and a
    /// store with more dishes than a search lists.
    fn fixture() -> Vec<(StoreText, Vec<Item>)> {
        let store = |id: &str, name: &str, cuisine: &str, description: &str| StoreText {
            id: id.to_string(),
            name: name.to_string(),
            description: description.to_string(),
            cuisine: cuisine.to_string(),
        };
        vec![
            (
                store(
                    "f1",
                    "Crêperie Été",
                    "Française",
                    "Galettes de sarrasin et crêpes sucrées.",
                ),
                vec![
                    item(
                        "suzette",
                        "Crêpe Suzette",
                        "Desserts",
                        "Orange and Grand Marnier.",
                    ),
                    item(
                        "complete",
                        "Galette Complète",
                        "Galettes",
                        "Ham, egg and Emmental.",
                    ),
                ],
            ),
            (
                store(
                    "f2",
                    "100% Juice",
                    "Juice_Bar",
                    "Cold-pressed fruit, no \\ added sugar.",
                ),
                vec![
                    item("green", "Green_Machine", "Juices", "Kale, apple and lemon."),
                    item(
                        "shot",
                        "O'Reilly's Shot",
                        "Shots",
                        "Ginger, served \"hot\".",
                    ),
                ],
            ),
            (
                store("f3", "ΚΑΦΕΝΕΙΟ", "Greek", "Ελληνικός καφές."),
                vec![item("frape", "Φραπέ", "Coffee", "Iced instant coffee.")],
            ),
            (
                store("f4", "Straße Döner", "Turkish", "Open late."),
                vec![
                    item("doner", "Döner Kebab", "Mains", "In bread."),
                    item("iskender", "İskender", "Mains", "Sliced döner with yogurt."),
                ],
            ),
            (
                store("f5", "Zest", "Juice", "Fresh to go."),
                vec![item("apple", "Apple Juice", "Pressed", "Just apples.")],
            ),
            (
                store("f6", "Zest", "Smoothies", "Blended to order."),
                (1..=7)
                    .rev()
                    .map(|n| {
                        item(
                            &format!("s{n}"),
                            &format!("Smoothie {n}"),
                            "Blends",
                            "Thick.",
                        )
                    })
                    .collect(),
            ),
        ]
    }

    /// What a layer answered, in the order shown: each store, its own tier,
    /// and its dishes' ids.
    type Shown = Vec<(String, Option<u8>, Vec<String>)>;

    fn shown(matched: Vec<Matched>) -> Shown {
        ordered(matched)
            .into_iter()
            .map(|m| (m.id, m.tier, m.dishes.into_iter().map(|d| d.id).collect()))
            .collect()
    }

    fn row(store: &str, tier: Option<u8>, dishes: &[&str]) -> (String, Option<u8>, Vec<String>) {
        (
            store.to_string(),
            tier,
            dishes.iter().map(|d| d.to_string()).collect(),
        )
    }

    /// **The table**: each search, and what both layers show for it.
    fn table() -> Vec<(&'static str, Shown)> {
        vec![
            // A `%` is a percent sign, not "anything": one store has one.
            ("%", vec![row("f2", Some(0), &[])]),
            // And an `_` an underscore, not "any character".
            ("_", vec![row("f2", Some(1), &["green"])]),
            // A backslash, as written, in a description.
            ("\\", vec![row("f2", Some(2), &[])]),
            // Quotes, single and double.
            ("'", vec![row("f2", None, &["shot"])]),
            ("\"hot\"", vec![row("f2", None, &["shot"])]),
            // Case ignored, accents kept: "CRÊPE" is "crêpe".
            ("CRÊPE", vec![row("f1", Some(0), &["suzette"])]),
            ("crepe", vec![]),
            // Greek, lowered; its accent is not folded.
            ("καφές", vec![row("f3", Some(2), &[])]),
            ("καφενειο", vec![row("f3", Some(0), &[])]),
            ("καφες", vec![]),
            // A capital sharp s is a sharp s; a sharp s is not "ss".
            ("STRAẞE", vec![row("f4", Some(0), &[])]),
            ("strasse", vec![]),
            // A dotted capital I is an i (its simple mapping).
            ("İSKENDER", vec![row("f4", None, &["iskender"])]),
            ("iskender", vec![row("f4", None, &["iskender"])]),
            // Several terms: each must match one of the record's fields.
            ("döner mains", vec![row("f4", None, &["doner", "iskender"])]),
            ("galette ham", vec![row("f1", None, &["complete"])]),
            // A term in the store and another in a dish: neither matches.
            ("zest apple", vec![]),
            // Ranked: a store's name, then its cuisine, then its
            // description; a store matched only by its dishes after.
            (
                "juice",
                vec![
                    row("f2", Some(0), &["green"]),
                    row("f5", Some(1), &["apple"]),
                ],
            ),
            ("coffee", vec![row("f3", None, &["frape"])]),
            // Two stores of one name, by id; then one only its dish matched.
            (
                "z",
                vec![
                    row("f5", Some(0), &[]),
                    row("f6", Some(0), &[]),
                    row("f1", None, &["suzette"]),
                ],
            ),
            // Dishes by tier, then name, then id, whatever the menu's order.
            (
                "smoothie",
                vec![row(
                    "f6",
                    Some(1),
                    &["s1", "s2", "s3", "s4", "s5", "s6", "s7"],
                )],
            ),
            // Dishes of one tier by name, not by id.
            ("ette", vec![row("f1", Some(2), &["suzette", "complete"])]),
            // None matching.
            ("nothing like it", vec![]),
        ]
    }

    fn terms_of(text: &str) -> Vec<String> {
        match asked(text) {
            Asked::Terms(t) => t,
            other => panic!("{text:?} asked {other:?}"),
        }
    }

    #[test]
    fn in_memory_the_table_is_answered() {
        for (text, want) in table() {
            let got = shown(matched(&terms_of(text), fixture()));
            assert_eq!(got, want, "{text:?}");
        }
    }

    /// **A dish's tier, and a store's**: the three tiers each.
    #[test]
    fn a_tier_says_where_the_terms_were_found() {
        let t = |text: &str| terms_of(text);
        let fields = ["blue bottle", "coffee", "small-batch beans"];
        assert_eq!(tier(&t("blue"), fields), Some(0));
        assert_eq!(tier(&t("blue coffee"), fields), Some(1));
        assert_eq!(tier(&t("coffee"), fields), Some(1));
        assert_eq!(tier(&t("blue beans"), fields), Some(2));
        assert_eq!(tier(&t("blue tea"), fields), None);
        let found = matched(&t("smoothie 3"), fixture());
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].tier, None, "the store's fields hold no 3");
        assert_eq!(found[0].dishes.len(), 1);
        assert_eq!(found[0].dishes[0].tier, 0);
        // A dish's fields: its name, then its category, then its
        // description.
        for (text, dish, want) in DISH_TIERS {
            let found = matched(&t(text), fixture());
            let got = found
                .iter()
                .flat_map(|m| &m.dishes)
                .find(|d| d.id == *dish)
                .map(|d| d.tier);
            assert_eq!(got, Some(*want), "{text:?}: {dish}");
        }
    }

    /// **Searches, a dish each finds, and its tier.**
    const DISH_TIERS: &[(&str, &str, u8)] = &[
        ("kebab", "doner", 0),
        ("mains", "doner", 1),
        ("döner mains", "doner", 1),
        ("döner mains", "iskender", 2),
        ("bread", "doner", 2),
        ("juices", "green", 1),
        ("coffee", "frape", 1),
    ];

    /// **Stores by rank, then name, then id; dishes by tier, then name,
    /// then id**, whatever order they came in.
    #[test]
    fn stores_and_dishes_are_ordered_by_rank_then_name_then_id() {
        let dish = |id: &str, name: &str, tier: u8| Dish {
            id: id.to_string(),
            name: name.to_string(),
            description: String::new(),
            category: String::new(),
            price: 0,
            tier,
        };
        let store = |id: &str, name: &str, tier: Option<u8>, dishes: Vec<Dish>| Matched {
            id: id.to_string(),
            name: name.to_string(),
            description: String::new(),
            cuisine: String::new(),
            tier,
            dishes,
        };
        let got = shown(vec![
            store("1", "B", Some(1), vec![]),
            store("2", "A", Some(1), vec![]),
            store("3", "A", None, vec![dish("x", "Y", 0)]),
            store("4", "Z", Some(2), vec![]),
            store("5", "A", Some(1), vec![]),
            store(
                "6",
                "Q",
                None,
                vec![
                    dish("b", "N", 1),
                    dish("c", "M", 1),
                    dish("a", "M", 1),
                    dish("d", "Z", 0),
                ],
            ),
        ]);
        assert_eq!(
            got,
            vec![
                row("2", Some(1), &[]),
                row("5", Some(1), &[]),
                row("1", Some(1), &[]),
                row("4", Some(2), &[]),
                row("3", None, &["x"]),
                row("6", None, &["d", "a", "c", "b"]),
            ]
        );
    }

    /// **Blank, too long, and the bound's edge**: in characters, not bytes.
    #[test]
    fn a_blank_or_long_text_is_a_declared_error() {
        assert_eq!(asked(""), Asked::Blank);
        assert_eq!(
            asked(" \t\u{3000}\n"),
            Asked::Blank,
            "Unicode's white space"
        );
        assert_eq!(asked(&"x".repeat(MOST_CHARACTERS + 1)), Asked::TooLong);
        assert_eq!(
            asked(&" ".repeat(MOST_CHARACTERS + 1)),
            Asked::TooLong,
            "bounded as given, before it is trimmed"
        );
        let edge = "é".repeat(MOST_CHARACTERS);
        assert_eq!(
            asked(&edge),
            Asked::Terms(vec![edge.clone()]),
            "200 bytes, 100 characters"
        );
        assert_eq!(
            asked("  Two\tWORDS "),
            Asked::Terms(vec!["two".to_string(), "words".to_string()])
        );
    }

    #[test]
    fn a_term_is_lowered_a_character_at_a_time() {
        assert_eq!(lower("İSTANBUL"), "istanbul", "U+0130's simple mapping");
        assert_eq!(lower("ΟΔΟΣ"), "οδοσ", "no final sigma: the simple mapping");
        assert_eq!(lower("ẞ"), "ß");
        assert_eq!(lower("Ǆ"), "ǆ");
    }

    #[test]
    fn a_pattern_escapes_what_like_reads() {
        assert_eq!(pattern("a%b_c\\d"), "%a\\%b\\_c\\\\d%");
        assert_eq!(
            pattern("o'r\"x"),
            "%o'r\"x%",
            "quotes are bound, not escaped"
        );
    }

    /// **The in-memory layer's operation**: registered among its reads, and
    /// answering the program's declared cases.
    #[test]
    fn in_memory_the_layer_answers_the_programs_cases() {
        let ops = crate::data::DataLayer::reads(&crate::store::StoreData::new(), "s", None);
        let find = ops.get("store:data/search#find").expect("registered");
        let case = |c: &str| {
            vec![Val::Result(Err(Some(Box::new(Val::Variant(
                c.into(),
                None,
            )))))]
        };
        assert_eq!(find(&[Val::String("  ".into())]).unwrap(), case("blank"));
        assert_eq!(
            find(&[Val::String("y".repeat(MOST_CHARACTERS + 1))]).unwrap(),
            case("too-long")
        );
        assert!(find(&[]).is_err(), "it takes its text");
    }

    /// The stores the in-memory layer's operation lists for `text`, by id.
    fn listed_in_memory(text: &str) -> Vec<String> {
        let ops = crate::data::DataLayer::reads(&crate::store::StoreData::new(), "s", None);
        let find = ops.get("store:data/search#find").expect("registered");
        let answer = find(&[Val::String(text.into())]).expect("answered");
        let [Val::Result(Ok(Some(found)))] = answer.as_slice() else {
            panic!("{answer:?}")
        };
        let Val::Record(fields) = found.as_ref() else {
            panic!("{found:?}")
        };
        let Some((_, Val::List(stores))) = fields.iter().find(|(n, _)| n == "stores") else {
            panic!("{fields:?}")
        };
        stores
            .iter()
            .map(|s| match s {
                Val::Record(f) => match f.iter().find(|(n, _)| n == "id") {
                    Some((_, Val::String(id))) => id.clone(),
                    other => panic!("{other:?}"),
                },
                other => panic!("{other:?}"),
            })
            .collect()
    }

    /// **The seed, searched in memory**: each store's cuisine among what is
    /// matched, its name first.
    #[test]
    fn in_memory_the_seed_is_searched() {
        assert_eq!(listed_in_memory("CAFÉ"), ["48"], "Harbor Coffee's cuisine");
        assert_eq!(listed_in_memory("SCONE"), ["48"], "its dish");
        assert_eq!(
            listed_in_memory("coffee"),
            ["48", "47"],
            "its name, then a cuisine"
        );
        assert_eq!(
            listed_in_memory("tea"),
            ["48"],
            "steamed milk: a part of a word"
        );
        assert!(listed_in_memory("tofu").is_empty());
    }

    /// **What is listed, and counted**: the first twenty stores, each with
    /// its first five dishes; the rest counted.
    #[test]
    fn found_lists_the_first_and_counts_the_rest() {
        let store = |n: usize, dishes: usize| Matched {
            id: format!("s{n:02}"),
            name: "Same".to_string(),
            description: String::new(),
            cuisine: String::new(),
            tier: Some(0),
            dishes: (0..dishes)
                .map(|d| Dish {
                    id: format!("d{d}"),
                    name: format!("Dish {d}"),
                    description: String::new(),
                    category: String::new(),
                    price: 100,
                    tier: 0,
                })
                .collect(),
        };
        let field = |fields: &[(String, Val)], name: &str| {
            fields
                .iter()
                .find(|(n, _)| n == name)
                .map(|(_, v)| v.clone())
        };
        let all: Vec<Matched> = (0..MOST_STORES + 2).rev().map(|n| store(n, 7)).collect();
        let Val::Record(fields) = found(all) else {
            panic!("a record")
        };
        assert_eq!(field(&fields, "more-stores"), Some(Val::S64(2)));
        let Some(Val::List(stores)) = field(&fields, "stores") else {
            panic!("stores")
        };
        assert_eq!(stores.len(), MOST_STORES);
        let Val::Record(first) = &stores[0] else {
            panic!("a store")
        };
        assert_eq!(field(first, "id"), Some(Val::String("s00".into())));
        assert_eq!(field(first, "more-dishes"), Some(Val::S64(2)));
        let Some(Val::List(dishes)) = field(first, "dishes") else {
            panic!("dishes")
        };
        assert_eq!(dishes.len(), MOST_DISHES);
        // At the bounds, nothing is counted.
        let all: Vec<Matched> = (0..MOST_STORES).map(|n| store(n, MOST_DISHES)).collect();
        let Val::Record(fields) = found(all) else {
            panic!("a record")
        };
        assert_eq!(field(&fields, "more-stores"), Some(Val::S64(0)));
        let Some(Val::List(stores)) = field(&fields, "stores") else {
            panic!("stores")
        };
        let Val::Record(first) = &stores[0] else {
            panic!("a store")
        };
        assert_eq!(field(first, "more-dishes"), Some(Val::S64(0)));
    }

    // ----- on PostgreSQL ---------------------------------------------------

    /// The database the tests run against, where the environment names one.
    fn database() -> Option<String> {
        match std::env::var("PW_STORE_DATABASE_URL") {
            Ok(url) if !url.is_empty() => Some(url),
            _ => {
                eprintln!("skipped: PW_STORE_DATABASE_URL is not set");
                None
            }
        }
    }

    /// **The fixture, as PostgreSQL holds it**: the seed's stores gone, the
    /// fixture's in their place, in a test's own schema.
    fn on_postgres_fixture(layer: &crate::store_pg::StorePg) -> postgres::Client {
        let mut c = layer.sql();
        c.batch_execute("TRUNCATE stores CASCADE").expect("emptied");
        for (position, (s, menu)) in fixture().into_iter().enumerate() {
            c.execute(
                "INSERT INTO stores (id, position, name, description, opens_minute, closes_minute) \
                 VALUES ($1, $2, $3, $4, 420, 1140)",
                &[&s.id, &(position as i32), &s.name, &s.description],
            )
            .expect("a store");
            c.execute(
                "INSERT INTO store_cuisines (store, cuisine) VALUES ($1, $2)",
                &[&s.id, &s.cuisine],
            )
            .expect("its cuisine");
            for (at, i) in menu.iter().enumerate() {
                c.execute(
                    "INSERT INTO categories (store, id, name) VALUES ($1, $2, $3) \
                     ON CONFLICT DO NOTHING",
                    &[&s.id, &i.category, &i.category_name],
                )
                .expect("a category");
                c.execute(
                    "INSERT INTO menu_items (store, id, position, name, description, price, \
                         category, served, available) \
                     VALUES ($1, $2, $3, $4, $5, $6, $7, 'hot', true)",
                    &[
                        &s.id,
                        &i.id,
                        &(at as i32),
                        &i.name,
                        &i.description,
                        &i.price,
                        &i.category,
                    ],
                )
                .expect("an item");
            }
        }
        c
    }

    #[test]
    fn on_postgres_the_table_is_answered_alike() {
        let Some(url) = database() else { return };
        let layer = crate::store_pg::StorePg::for_test(&url).expect("the layer");
        let mut c = on_postgres_fixture(&layer);
        for (text, want) in table() {
            let got = shown(on_postgres(&mut c, &terms_of(text)).expect("searched"));
            assert_eq!(got, want, "{text:?}");
        }
        // And each record found is the fixture's own text, as memory's is.
        let tiered = DISH_TIERS.iter().map(|(text, ..)| *text);
        for text in ["juice", "z", "%", "ette"].into_iter().chain(tiered) {
            let terms = terms_of(text);
            let pg = ordered(on_postgres(&mut c, &terms).expect("searched"));
            let mem = ordered(matched(&terms, fixture()));
            assert_eq!(pg, mem, "{text:?}");
        }
    }

    /// **The layer's own operation, on PostgreSQL**: the seed searched
    /// through `reads`, its cuisine read from the migration's table.
    #[test]
    fn on_postgres_the_seed_is_searched_through_the_layer() {
        let Some(url) = database() else { return };
        let layer = crate::store_pg::StorePg::for_test(&url).expect("the layer");
        let ops = crate::data::DataLayer::reads(&layer, "s", None);
        let find = ops.get("store:data/search#find").expect("registered");
        let answer = find(&[Val::String("CAFÉ".into())]).expect("answered");
        let text = format!("{answer:?}");
        assert!(
            text.contains("Harbor Coffee") && text.contains("Café, Bakery"),
            "{text}"
        );
        assert!(!text.contains("Blue Bottle"), "{text}");
        // What the database holds is what is matched: a cuisine changed
        // there is found, as no table in this file could find it.
        layer
            .sql()
            .execute(
                "UPDATE store_cuisines SET cuisine = 'Ramen' WHERE store = '47'",
                &[],
            )
            .expect("changed");
        let ops = crate::data::DataLayer::reads(&layer, "s", None);
        let find = ops.get("store:data/search#find").expect("registered");
        let text = format!(
            "{:?}",
            find(&[Val::String("RAMEN".into())]).expect("answered")
        );
        assert!(
            text.contains("Blue Bottle") && text.contains("Ramen"),
            "{text}"
        );
    }

    /// **The seed's cuisines are the in-memory layer's**: the migration's
    /// rows, store by store.
    #[test]
    fn on_postgres_each_stores_cuisine_is_memorys() {
        let Some(url) = database() else { return };
        let layer = crate::store_pg::StorePg::for_test(&url).expect("the layer");
        let rows = layer
            .sql()
            .query(
                "SELECT store, cuisine FROM store_cuisines ORDER BY store",
                &[],
            )
            .expect("read");
        let pg: Vec<(String, String)> = rows.iter().map(|r| (r.get(0), r.get(1))).collect();
        let mem: Vec<(String, String)> = ["47", "48"]
            .iter()
            .map(|s| (s.to_string(), cuisine_of(s).to_string()))
            .collect();
        assert_eq!(pg, mem);
        assert!(
            mem.iter().all(|(_, c)| !c.is_empty()),
            "every seeded store has one"
        );
    }

    /// **Every character is lowered alike by both layers**: each scalar
    /// value but NUL, which `text` cannot hold, lowered by `lower(..
    /// COLLATE pg_c_utf8)` and by [`lower`], and compared. Without
    /// [`NEWER_THAN_POSTGRES`] 28 differ, PostgreSQL 18.6's Unicode 16.0
    /// against Rust 1.97's 17.0. It fails when either side's Unicode version
    /// moves, a character then lowered by one and not the other: that is how
    /// the limitation (KNOWN_LIMITATIONS) is looked at again, and the hold
    /// changed or dropped.
    #[test]
    fn on_postgres_every_character_is_lowered_alike() {
        let Some(url) = database() else { return };
        let layer = crate::store_pg::StorePg::for_test(&url).expect("the layer");
        let mut c = layer.sql();
        let all: Vec<char> = (1..=0x10FFFF_u32).filter_map(char::from_u32).collect();
        let mut differ: Vec<(u32, u32, u32)> = Vec::new();
        for chunk in all.chunks(1 << 16) {
            let text: String = chunk.iter().collect();
            let pg: String = c
                .query_one("SELECT lower($1::text COLLATE pg_c_utf8)", &[&text])
                .expect("lowered")
                .get(0);
            let pg: Vec<char> = pg.chars().collect();
            assert_eq!(pg.len(), chunk.len(), "a simple mapping keeps the length");
            let ours: Vec<char> = lower(&text).chars().collect();
            for ((c, p), o) in chunk.iter().zip(&pg).zip(&ours) {
                if p != o {
                    differ.push((u32::from(*c), u32::from(*p), u32::from(*o)));
                }
            }
        }
        let unicode: String = c
            .query_one("SELECT unicode_version()", &[])
            .expect("its version")
            .get(0);
        let rust = char::UNICODE_VERSION;
        eprintln!(
            "PostgreSQL's Unicode {unicode}, Rust's {}.{}: {} differ",
            rust.0,
            rust.1,
            differ.len()
        );
        assert_eq!(
            differ,
            Vec::<(u32, u32, u32)>::new(),
            "(character, PostgreSQL's, ours): Unicode {unicode} against {rust:?}"
        );
    }

    /// **What is held back is what Rust's tables would lower**, each of it:
    /// a character Rust's tables leave alone needs no holding, and a hold
    /// left after both sides agree would keep a case PostgreSQL lowers.
    #[test]
    fn each_character_held_back_is_one_rusts_tables_lower() {
        for range in NEWER_THAN_POSTGRES {
            for c in range.clone().filter_map(char::from_u32) {
                let rusts = c.to_lowercase().next().unwrap_or(c);
                assert_ne!(
                    rusts,
                    c,
                    "{:X} has no lowercase in Rust's tables",
                    u32::from(c)
                );
                assert_eq!(lower(&c.to_string()), c.to_string());
            }
        }
        assert_eq!(
            char::UNICODE_VERSION,
            (17, 0, 0),
            "the hold is 17.0's against 16.0"
        );
    }
}

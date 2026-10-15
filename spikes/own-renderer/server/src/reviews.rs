//! **A store's ratings and reviews** (track `reviews`, ADR-XXXX): a
//! customer's rating of each store of their latest order, once it is
//! delivered, once; 1 to 5 stars and a comment, empty where none. The
//! store's data, held by both layers to one meaning; what a page says of it
//! is the program's (`examples/store/reviews.pw`).
//!
//! - In memory: `State`'s `ratings`, oldest first, each order with its id.
//! - On PostgreSQL: the table `order_ratings`
//!   (`migrations/store/0017_ratings.sql`), keyed by its order and store.
//!
//! A rating holds no user: its order names its owner, and the reviews a
//! store's page shows are its stars and words alone (ADR-XXXX, decision 5).
//!
//! The operations (`store:data/ratings`), registered beside the catalogue's
//! (`store.rs`'s seam): `#for-stores`, each listed store's count and stars,
//! one statement for any number of stores (NEXT 4b); `#reviews`, a store's
//! newest with a comment; `#for-order`, the reader's latest order as rating
//! reads it, one statement; and `#rate`, a command's, the order read again
//! and the rating written. The deployment's predicate `MayRate(store)` reads
//! `#for-order` (`identity.rs`'s seam).

use super::*;
use crate::data::Ops;
use crate::store::{With, Written};

/// **The most characters a comment holds** (code points, as the program's
/// `String.length` and PostgreSQL's `char_length` count them).
pub(crate) const MOST_COMMENT: usize = 500;

/// **How many reviews a store's page shows**: the newest with a comment.
pub(crate) const REVIEWS_SHOWN: usize = 5;

/// The status an order may be rated in.
const DELIVERED: &str = "delivered";

/// What the predicate and the page read the reader's order through.
const FOR_ORDER: &str = "store:data/ratings#for-order";

/// **A rating, as it is written**: the order it was given through, the
/// store rated, its stars (1 to 5) and its comment, empty where none.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Rating {
    pub(crate) order: i64,
    pub(crate) store: String,
    pub(crate) stars: i64,
    pub(crate) comment: String,
}

/// **A store of an order, as rating reads it**: its id, its name, and the
/// stars and comment given of it through the order, where given.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct OfOrder {
    pub(crate) store: String,
    pub(crate) name: String,
    pub(crate) given: Option<(i64, String)>,
}

/// **An owner's latest order, as rating reads it**: its id and status, and
/// each of its stores, in the order it was charged them (its parts, ADR-0329).
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ToRate {
    pub(crate) order: i64,
    pub(crate) status: String,
    pub(crate) stores: Vec<OfOrder>,
}

// --- In memory ---------------------------------------------------------------

/// **Each of `stores`' ratings counted, and their stars summed**, in
/// `stores`' order, a store with none left out: memory's way, a scan.
pub(crate) fn mem_ratings_of(ratings: &[Rating], stores: &[String]) -> Vec<(String, i64, i64)> {
    stores
        .iter()
        .filter_map(|s| {
            let of: Vec<&Rating> = ratings.iter().filter(|r| r.store == *s).collect();
            (!of.is_empty()).then(|| (s.clone(), of.len() as i64, of.iter().map(|r| r.stars).sum()))
        })
        .collect()
}

/// **A store's newest `most` ratings with a comment**, newest first, each by
/// its id: its place among the ratings, from 1, as PostgreSQL's serial.
pub(crate) fn mem_reviews(ratings: &[Rating], store: &str, most: usize) -> Vec<(i64, i64, String)> {
    ratings
        .iter()
        .enumerate()
        .rev()
        .filter(|(_, r)| r.store == store && !r.comment.is_empty())
        .take(most)
        .map(|(at, r)| (at as i64 + 1, r.stars, r.comment.clone()))
        .collect()
}

/// **An owner's latest order, as rating reads it**: its stores its charged
/// parts', each named as the store is, and rated where a rating names the
/// order and the store.
pub(crate) fn mem_to_rate(
    order: Option<&crate::store::Order>,
    charged: Option<&crate::checkout::Charged>,
    stores: &[crate::store::StoreRow],
    ratings: &[Rating],
) -> Option<ToRate> {
    let order = order?;
    let parts = charged.map(|c| c.parts.as_slice()).unwrap_or_default();
    Some(ToRate {
        order: order.id,
        status: order.status.clone(),
        stores: parts
            .iter()
            .map(|p| OfOrder {
                store: p.store.clone(),
                name: stores
                    .iter()
                    .find(|s| s.id == p.store)
                    .map(|s| s.name.clone())
                    .unwrap_or_default(),
                given: ratings
                    .iter()
                    .find(|r| r.order == order.id && r.store == p.store)
                    .map(|r| (r.stars, r.comment.clone())),
            })
            .collect(),
    })
}

/// **A rating written in memory**, or `false` where its order already has
/// one of its store: the key PostgreSQL's table has.
pub(crate) fn mem_rate(ratings: &mut Vec<Rating>, rating: &Rating) -> bool {
    if ratings
        .iter()
        .any(|r| r.order == rating.order && r.store == rating.store)
    {
        return false;
    }
    ratings.push(rating.clone());
    true
}

// --- The operations ----------------------------------------------------------

fn s(v: &str) -> Val {
    Val::String(v.to_string())
}

/// A refusal the program declares, `RatingError`'s case.
fn refused(case: &str) -> Vec<Val> {
    vec![Val::Result(Err(Some(Box::new(Val::Variant(
        case.into(),
        None,
    )))))]
}

/// **The reader an operation by a user's handle was passed**: the user's
/// id, a handle on the wire, which only the host makes (ADR-0270).
fn reader_of<'v>(op: &str, args: &'v [Val]) -> Result<(&'v str, &'v [Val]), String> {
    match args.first() {
        Some(Val::String(reader)) if !reader.is_empty() => Ok((reader, &args[1..])),
        _ => Err(format!("{op} received {args:?}")),
    }
}

/// `OrderToRate`, as the program's is: none delivered and no store where the
/// reader has no order.
fn to_rate_val(order: Option<&ToRate>) -> Val {
    let delivered = order.is_some_and(|o| o.status == DELIVERED);
    let stores = order.map(|o| o.stores.as_slice()).unwrap_or_default();
    Val::Record(vec![
        ("delivered".into(), Val::Bool(delivered)),
        (
            "stores".into(),
            Val::List(
                stores
                    .iter()
                    .map(|of| {
                        Val::Record(vec![
                            ("store".into(), s(&of.store)),
                            ("name".into(), s(&of.name)),
                            (
                                "given".into(),
                                Val::Option(of.given.as_ref().map(|(stars, comment)| {
                                    Box::new(Val::Record(vec![
                                        ("stars".into(), Val::S64(*stars)),
                                        ("comment".into(), s(comment)),
                                    ]))
                                })),
                            ),
                        ])
                    })
                    .collect(),
            ),
        ),
    ])
}

/// **The ratings' reads**, over `with`, the layer's rows: each listed
/// store's count and stars, a store's reviews, and the reader's latest order
/// as rating reads it.
pub(crate) fn ops(with: With) -> Ops {
    let mut ops: Ops = BTreeMap::new();
    let w = with.clone();
    ops.insert(
        "store:data/ratings#for-stores".to_string(),
        Arc::new(move |args: &[Val]| match args {
            [Val::List(stores)] => {
                let stores = stores
                    .iter()
                    .map(|s| match s {
                        Val::String(s) => Ok(s.clone()),
                        other => Err(format!("ratings#for-stores received {other:?}")),
                    })
                    .collect::<Result<Vec<_>, String>>()?;
                // By set (NEXT 4b): one read of the stores' ratings.
                w(&mut |r| {
                    Ok(vec![Val::List(
                        r.ratings_of(&stores)?
                            .into_iter()
                            .map(|(store, count, stars)| {
                                Val::Record(vec![
                                    ("store".into(), Val::String(store)),
                                    ("count".into(), Val::S64(count)),
                                    ("stars".into(), Val::S64(stars)),
                                ])
                            })
                            .collect(),
                    )])
                })
            }
            other => Err(format!("ratings#for-stores received {other:?}")),
        }),
    );
    let w = with.clone();
    ops.insert(
        "store:data/ratings#reviews".to_string(),
        Arc::new(move |args: &[Val]| match args {
            // Its stars and its words: no user (ADR-XXXX, decision 5).
            [Val::String(store)] => w(&mut |r| {
                Ok(vec![Val::List(
                    r.reviews(store, REVIEWS_SHOWN)?
                        .into_iter()
                        .map(|(id, stars, comment)| {
                            Val::Record(vec![
                                ("id".into(), Val::S64(id)),
                                ("stars".into(), Val::S64(stars)),
                                ("comment".into(), Val::String(comment)),
                            ])
                        })
                        .collect(),
                )])
            }),
            other => Err(format!("ratings#reviews received {other:?}")),
        }),
    );
    ops.insert(
        FOR_ORDER.to_string(),
        Arc::new(move |args: &[Val]| {
            let (reader, rest) = reader_of("ratings#for-order", args)?;
            if !rest.is_empty() {
                return Err(format!("ratings#for-order received {args:?}"));
            }
            with(&mut |r| Ok(vec![to_rate_val(r.to_rate(reader)?.as_ref())]))
        }),
    );
    ops
}

/// **A rating given, a command's** (`store:data/ratings#rate`): the reader's
/// latest order read again in the command's transaction, delivered and
/// holding the store, and the rating written once, by the data's own key.
/// Stars past 1 to 5 or a comment past [`MOST_COMMENT`] are no program's,
/// whose types refuse them: the operation traps.
pub(crate) fn writes(with: With, written: Arc<Mutex<Written>>) -> Ops {
    let mut ops: Ops = BTreeMap::new();
    ops.insert(
        "store:data/ratings#rate".to_string(),
        Arc::new(move |args: &[Val]| {
            let (reader, rest) = reader_of("ratings#rate", args)?;
            let [Val::String(store), Val::S64(stars), Val::String(comment)] = rest else {
                return Err(format!("ratings#rate received {args:?}"));
            };
            if !(1..=5).contains(stars) {
                return Err(format!("ratings#rate: {stars} stars, past 1 to 5"));
            }
            if comment.chars().count() > MOST_COMMENT {
                return Err(format!(
                    "ratings#rate: a comment past {MOST_COMMENT} characters"
                ));
            }
            with(&mut |r| {
                let Some(order) = r.to_rate(reader)? else {
                    return Ok(refused("not-delivered"));
                };
                if order.status != DELIVERED {
                    return Ok(refused("not-delivered"));
                }
                let Some(of) = order.stores.iter().find(|of| of.store == *store) else {
                    return Ok(refused("not-in-order"));
                };
                if of.given.is_some() {
                    return Ok(refused("already-rated"));
                }
                let rating = Rating {
                    order: order.order,
                    store: store.clone(),
                    stars: *stars,
                    comment: comment.clone(),
                };
                if !r.rate(&rating)? {
                    return Ok(refused("already-rated"));
                }
                written.lock().expect("written").wrote = true;
                Ok(vec![Val::Result(Ok(Some(Box::new(Val::S64(*stars)))))])
            })
        }),
    );
    ops
}

/// **Whether `MayRate(store)` holds for a session's principal** (ADR-XXXX,
/// decision 3): their latest order is delivered, holds `store`, and has no
/// rating of it, read through the command's own operations, so within its
/// transaction, as `OwnsPost(post)` reads a post's author. Only the order's
/// owner: the order is read by the principal's own user.
pub(crate) fn may_rate(
    principal: Option<crate::identity::Principal>,
    bound: &[&Val],
    host: &BTreeMap<String, HostFn>,
) -> Result<bool, String> {
    let Some(principal) = principal else {
        return Ok(false);
    };
    let [Val::String(store)] = bound else {
        return Err(format!(
            "MayRate takes one store id, and was given {bound:?}"
        ));
    };
    let read = host
        .get(FOR_ORDER)
        .ok_or("MayRate reads the reader's order through store:data/ratings#for-order")?;
    match read(&[Val::String(principal.user)])?.as_slice() {
        [order] => Ok(rateable(order, store)),
        other => Err(format!("ratings#for-order answered {other:?}")),
    }
}

/// Whether an order, as `#for-order` answers it, is delivered and holds
/// `store` with no rating given.
fn rateable(order: &Val, store: &str) -> bool {
    let delivered = matches!(val_at(order, &["delivered"]), Some(Val::Bool(true)));
    let Some(Val::List(stores)) = val_at(order, &["stores"]) else {
        return false;
    };
    delivered
        && stores.iter().any(|of| {
            matches!(val_at(of, &["store"]), Some(Val::String(id)) if id == store)
                && matches!(val_at(of, &["given"]), Some(Val::Option(None)))
        })
}

// --- PostgreSQL --------------------------------------------------------------

fn pg(e: postgres::Error) -> String {
    crate::feed_pg::pg(e)
}

/// **Each of `stores`' ratings counted, and their stars summed, on
/// PostgreSQL**, in one statement whatever their number (NEXT 4b), in
/// `stores`' order as memory's scan gives them.
pub(crate) fn pg_ratings_of(
    c: &mut crate::store_pg::Counted<'_>,
    stores: &[String],
) -> Result<Vec<(String, i64, i64)>, String> {
    let counted: BTreeMap<String, (i64, i64)> = c
        .query(
            "SELECT store, count(*), sum(stars)::bigint FROM order_ratings \
             WHERE store = ANY($1) GROUP BY store",
            &[&stores],
        )
        .map_err(pg)?
        .iter()
        .map(|r| (r.get(0), (r.get(1), r.get(2))))
        .collect();
    Ok(stores
        .iter()
        .filter_map(|s| counted.get(s).map(|(n, sum)| (s.clone(), *n, *sum)))
        .collect())
}

/// **A store's newest `most` ratings with a comment, on PostgreSQL**, newest
/// first.
pub(crate) fn pg_reviews(
    c: &mut crate::store_pg::Counted<'_>,
    store: &str,
    most: usize,
) -> Result<Vec<(i64, i64, String)>, String> {
    Ok(c.query(
        "SELECT id, stars, comment FROM order_ratings \
             WHERE store = $1 AND comment <> '' ORDER BY id DESC LIMIT $2",
        &[&store, &(most as i64)],
    )
    .map_err(pg)?
    .iter()
    .map(|r| (r.get(0), r.get(1), r.get(2)))
    .collect())
}

/// **An owner's latest order, as rating reads it, on PostgreSQL**: in one
/// statement, its id and status, and each of its parts' stores, named, with
/// the rating given of it, in the order it was charged them.
pub(crate) fn pg_to_rate(
    c: &mut crate::store_pg::Counted<'_>,
    owner: &str,
) -> Result<Option<ToRate>, String> {
    let rows = c
        .query(
            "SELECT o.id, o.status, p.store, s.name, r.stars, r.comment \
             FROM (SELECT id, status FROM orders WHERE owner = $1 ORDER BY id DESC LIMIT 1) o \
             LEFT JOIN order_parts p ON p.order_id = o.id \
             LEFT JOIN stores s ON s.id = p.store \
             LEFT JOIN order_ratings r ON r.order_id = o.id AND r.store = p.store \
             ORDER BY p.position",
            &[&owner],
        )
        .map_err(pg)?;
    let Some(first) = rows.first() else {
        return Ok(None);
    };
    let mut out = ToRate {
        order: first.get(0),
        status: first.get(1),
        stores: Vec::new(),
    };
    for row in &rows {
        let Some(store) = row.get::<_, Option<String>>(2) else {
            continue;
        };
        let stars: Option<i64> = row.get(4);
        out.stores.push(OfOrder {
            store,
            name: row.get::<_, Option<String>>(3).unwrap_or_default(),
            given: stars.map(|stars| (stars, row.get::<_, Option<String>>(5).unwrap_or_default())),
        });
    }
    Ok(Some(out))
}

/// **A rating written on PostgreSQL**, or `false` where its order already
/// has one of its store, by the table's key.
pub(crate) fn pg_rate(
    c: &mut crate::store_pg::Counted<'_>,
    rating: &Rating,
) -> Result<bool, String> {
    let written = c
        .execute(
            "INSERT INTO order_ratings (order_id, store, stars, comment) \
             VALUES ($1, $2, $3, $4) ON CONFLICT (order_id, store) DO NOTHING",
            &[&rating.order, &rating.store, &rating.stars, &rating.comment],
        )
        .map_err(pg)?;
    Ok(written == 1)
}

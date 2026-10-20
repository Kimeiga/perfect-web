//! **A store's opening hours** (track `store-hours`, docs/PARALLEL.md): its
//! weekly periods, in its IANA time zone, a period whose closing is not
//! after its opening running past midnight; its special dates, closed or
//! open other hours; and its last-order cut-off. The store's data, held by
//! both layers to one meaning; whether a store is open is the program's,
//! read by comparison with the platform's clock (ADR-0304), so a page is
//! told when the clock passes an opening or a closing.
//!
//! - In memory: `State`'s `hours`, first [`seed`].
//! - On PostgreSQL: `stores.time_zone` and `stores.last_order_minutes`, and
//!   the tables `store_hours` and `store_special_hours`
//!   (`migrations/store/0012_store_hours.sql`).
//!
//! The operations, registered beside the catalogue's (`store.rs`'s seam):
//! `store:data/hours#for-store`, `#for-stores` and `#for-cart`, read; and the
//! host's own `store:host/hours#set`, written by a test or a deployment's
//! staff, never by a program.

use super::*;
use crate::data::Ops;
use crate::store::{With, Written};

/// **One weekly period**: its ISO weekday (1, Monday, to 7, Sunday), and its
/// opening and closing minutes from midnight, 0 to 1440. A closing not after
/// the opening is the next day's (`22:00` to `02:00`); the same minute is
/// open the whole day round.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Period {
    pub(crate) weekday: i64,
    pub(crate) opens: i64,
    pub(crate) closes: i64,
}

/// **A special date**, in days since 1970-01-01 in the store's zone: closed,
/// or open from `opens` to `closes` in place of its weekday's periods.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Special {
    pub(crate) date: i64,
    pub(crate) closed: bool,
    pub(crate) opens: i64,
    pub(crate) closes: i64,
}

/// **A store's hours**: its zone, its cut-off in minutes before a closing,
/// its weekly periods and its special dates.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Hours {
    pub(crate) zone: String,
    pub(crate) last_order_minutes: i64,
    pub(crate) weekly: Vec<Period>,
    pub(crate) special: Vec<Special>,
}

/// **Each store's hours, as the store is first**: open every day, the whole
/// day round, in San Francisco's zone, the last order 20 minutes before a
/// closing (DoorDash's). Always open, so nothing a test of another track
/// orders depends on the hour it runs; the hours' own tests set theirs.
/// `migrations/store/0012_store_hours.sql` is the same rows.
pub(crate) fn seed() -> Hours {
    Hours {
        zone: "America/Los_Angeles".to_string(),
        last_order_minutes: 20,
        weekly: (1..=7)
            .map(|weekday| Period {
                weekday,
                opens: 0,
                closes: 0,
            })
            .collect(),
        special: Vec::new(),
    }
}

/// **What a set of hours is refused for**: a weekday past 1 to 7, a minute
/// past 0 to 1440, a cut-off below 0 or a day long, or a zone the host's
/// database does not have.
pub(crate) fn refused(h: &Hours) -> Option<String> {
    if pw_time::Database::system().zone(&h.zone).is_err() {
        return Some(format!("no time zone `{}`", h.zone));
    }
    if !(0..1440).contains(&h.last_order_minutes) {
        return Some(format!("a cut-off of {} minutes", h.last_order_minutes));
    }
    let minute = |m: i64| (0..=1440).contains(&m);
    for p in &h.weekly {
        if !(1..=7).contains(&p.weekday) || !minute(p.opens) || !minute(p.closes) {
            return Some(format!("a period {p:?}"));
        }
    }
    for s in &h.special {
        if !s.closed && (!minute(s.opens) || !minute(s.closes)) {
            return Some(format!("a special date {s:?}"));
        }
    }
    None
}

/// **A set of hours as a layer holds it**: its weekly periods by weekday
/// and opening, its special dates by date, the last of each that a set
/// lists twice kept, as PostgreSQL's upsert keeps it (`pg_set_hours`), so a
/// set reads back as one value on both layers (the integrator's ruling of
/// 2026-10-12).
pub(crate) fn held(h: &Hours) -> Hours {
    let weekly: BTreeMap<(i64, i64), &Period> =
        h.weekly.iter().map(|p| ((p.weekday, p.opens), p)).collect();
    let special: BTreeMap<i64, &Special> = h.special.iter().map(|s| (s.date, s)).collect();
    Hours {
        zone: h.zone.clone(),
        last_order_minutes: h.last_order_minutes,
        weekly: weekly.into_values().cloned().collect(),
        special: special.into_values().cloned().collect(),
    }
}

/// **The hours, as the program's `OpeningHours` is.**
pub(crate) fn hours_val(h: &Hours) -> Val {
    let int = |v: i64| Val::S64(v);
    Val::Record(vec![
        ("zone".into(), Val::String(h.zone.clone())),
        ("last-order-minutes".into(), int(h.last_order_minutes)),
        (
            "weekly".into(),
            Val::List(
                h.weekly
                    .iter()
                    .map(|p| {
                        Val::Record(vec![
                            ("weekday".into(), int(p.weekday)),
                            ("opens".into(), int(p.opens)),
                            ("closes".into(), int(p.closes)),
                        ])
                    })
                    .collect(),
            ),
        ),
        (
            "special".into(),
            Val::List(
                h.special
                    .iter()
                    .map(|s| {
                        Val::Record(vec![
                            ("date".into(), int(s.date)),
                            ("closed".into(), Val::Bool(s.closed)),
                            ("opens".into(), int(s.opens)),
                            ("closes".into(), int(s.closes)),
                        ])
                    })
                    .collect(),
            ),
        ),
    ])
}

/// [`hours_val`], read back: what `hours#set` is passed.
pub(crate) fn hours_from(v: &Val) -> Result<Hours, String> {
    let bad = || format!("hours received {v:?}");
    let int = |r: &Val, k: &str| match val_at(r, &[k]) {
        Some(Val::S64(n)) => Ok(*n),
        _ => Err(bad()),
    };
    let list = |k: &str| match val_at(v, &[k]) {
        Some(Val::List(items)) => Ok(items.clone()),
        _ => Err(bad()),
    };
    let zone = match val_at(v, &["zone"]) {
        Some(Val::String(z)) => z.clone(),
        _ => return Err(bad()),
    };
    let weekly = list("weekly")?
        .iter()
        .map(|p| {
            Ok(Period {
                weekday: int(p, "weekday")?,
                opens: int(p, "opens")?,
                closes: int(p, "closes")?,
            })
        })
        .collect::<Result<Vec<_>, String>>()?;
    let special = list("special")?
        .iter()
        .map(|s| {
            Ok(Special {
                date: int(s, "date")?,
                closed: matches!(val_at(s, &["closed"]), Some(Val::Bool(true))),
                opens: int(s, "opens")?,
                closes: int(s, "closes")?,
            })
        })
        .collect::<Result<Vec<_>, String>>()?;
    Ok(Hours {
        zone,
        last_order_minutes: int(v, "last-order-minutes")?,
        weekly,
        special,
    })
}

/// `{ store, hours }`, each store's, as `OpeningHoursOf` is.
fn of(store: &str, h: &Hours) -> Val {
    Val::Record(vec![
        ("store".into(), Val::String(store.to_string())),
        ("name".into(), Val::String(String::new())),
        ("hours".into(), hours_val(h)),
    ])
}

/// **The hours' reads**, over `with`, the layer's rows: a store's
/// (`not-found` where the layer holds no such store), each of a list's, and
/// each store's whose item is in a reader's cart, with its name.
pub(crate) fn ops(with: With) -> Ops {
    let mut ops: Ops = BTreeMap::new();
    let w = with.clone();
    ops.insert(
        "store:data/hours#for-store".to_string(),
        Arc::new(move |args: &[Val]| match args {
            [Val::String(store)] => w(&mut |r| {
                Ok(match r.hours(store)? {
                    Some(h) => vec![Val::Result(Ok(Some(Box::new(hours_val(&h)))))],
                    None => vec![Val::Result(Err(Some(Box::new(Val::Variant(
                        "not-found".into(),
                        None,
                    )))))],
                })
            }),
            other => Err(format!("hours#for-store received {other:?}")),
        }),
    );
    let w = with.clone();
    ops.insert(
        "store:data/hours#for-stores".to_string(),
        Arc::new(move |args: &[Val]| match args {
            [Val::List(stores)] => {
                let stores = stores
                    .iter()
                    .map(|s| match s {
                        Val::String(s) => Ok(s.clone()),
                        other => Err(format!("hours#for-stores received {other:?}")),
                    })
                    .collect::<Result<Vec<_>, String>>()?;
                // By set (NEXT 4b): one read of the stores' hours.
                w(&mut |r| {
                    let found = r.hours_of(&stores)?;
                    Ok(vec![Val::List(
                        found.iter().map(|(s, h)| of(s, h)).collect(),
                    )])
                })
            }
            other => Err(format!("hours#for-stores received {other:?}")),
        }),
    );
    ops.insert(
        "store:data/hours#for-cart".to_string(),
        Arc::new(move |args: &[Val]| match args {
            [Val::String(reader)] => with(&mut |r| {
                // By set (NEXT 4b): the cart's items' stores, their names and
                // their hours, a read each whatever the cart holds.
                let lines = r.cart(reader)?;
                let items: Vec<String> = lines.iter().map(|l| l.item.clone()).collect();
                let of_item = r.stores_of_items(&items)?;
                let mut seen: Vec<String> = Vec::new();
                for line in &lines {
                    if let Some(store) = of_item.get(&line.item)
                        && !seen.contains(store)
                    {
                        seen.push(store.clone());
                    }
                }
                let names: BTreeMap<String, String> = r
                    .stores_by_ids(&seen)?
                    .into_iter()
                    .map(|s| (s.id, s.name))
                    .collect();
                let mut out = Vec::new();
                for (store, h) in r.hours_of(&seen)? {
                    let Val::Record(mut fields) = of(&store, &h) else {
                        unreachable!("a record")
                    };
                    fields[1].1 = Val::String(names.get(&store).cloned().unwrap_or_default());
                    out.push(Val::Record(fields));
                }
                Ok(vec![Val::List(out)])
            }),
            other => Err(format!("hours#for-cart received {other:?}")),
        }),
    );
    ops
}

/// **The host's own write of a store's hours** (`store:host/hours#set`),
/// staged in a command's transaction as the host's other writes are; a
/// refused set changes nothing.
pub(crate) fn host_writes(with: With, written: Arc<Mutex<Written>>) -> Ops {
    let mut ops: Ops = BTreeMap::new();
    ops.insert(
        format!("{}hours#set", crate::store::HOST_OPS),
        Arc::new(move |args: &[Val]| {
            let [Val::String(store), hours] = args else {
                return Err(format!("hours#set received {args:?}"));
            };
            let hours = hours_from(hours)?;
            if let Some(why) = refused(&hours) {
                return Ok(vec![Val::Result(Err(Some(Box::new(Val::String(why)))))]);
            }
            with(&mut |r| {
                if r.hours(store)?.is_none() {
                    return Ok(vec![Val::Result(Err(Some(Box::new(Val::String(
                        format!("no store `{store}`"),
                    )))))]);
                }
                r.set_hours(store, &hours)?;
                written.lock().expect("written").wrote = true;
                Ok(vec![Val::Result(Ok(None))])
            })
        }),
    );
    ops
}

/// **The hours of a list of stores on PostgreSQL**, in three statements
/// whatever their number (NEXT 4b): the stores' zones and cut-offs, their
/// weekly periods and their special dates, each by set (`= ANY($1)`), in
/// the orders a store's own read gives them (`pg_hours`).
pub(crate) fn pg_hours_of(
    c: &mut crate::store_pg::Counted<'_>,
    stores: &[String],
) -> Result<Vec<(String, Hours)>, String> {
    let pg = crate::feed_pg::pg;
    let held: BTreeMap<String, (String, i64)> = c
        .query(
            "SELECT id, time_zone, last_order_minutes FROM stores WHERE id = ANY($1)",
            &[&stores],
        )
        .map_err(pg)?
        .iter()
        .map(|r| (r.get(0), (r.get(1), r.get(2))))
        .collect();
    let mut weekly: BTreeMap<String, Vec<Period>> = BTreeMap::new();
    for r in c
        .query(
            "SELECT store, weekday, opens_minute, closes_minute FROM store_hours \
             WHERE store = ANY($1) ORDER BY store, weekday, opens_minute",
            &[&stores],
        )
        .map_err(pg)?
    {
        weekly.entry(r.get(0)).or_default().push(Period {
            weekday: r.get(1),
            opens: r.get(2),
            closes: r.get(3),
        });
    }
    let mut special: BTreeMap<String, Vec<Special>> = BTreeMap::new();
    for r in c
        .query(
            "SELECT store, (day - DATE '1970-01-01')::bigint, opens_minute IS NULL, \
                    coalesce(opens_minute, 0), coalesce(closes_minute, 0) \
             FROM store_special_hours WHERE store = ANY($1) ORDER BY store, day",
            &[&stores],
        )
        .map_err(pg)?
    {
        special.entry(r.get(0)).or_default().push(Special {
            date: r.get(1),
            closed: r.get(2),
            opens: r.get(3),
            closes: r.get(4),
        });
    }
    Ok(stores
        .iter()
        .filter_map(|s| {
            let (zone, last_order_minutes) = held.get(s)?.clone();
            Some((
                s.clone(),
                Hours {
                    zone,
                    last_order_minutes,
                    weekly: weekly.get(s).cloned().unwrap_or_default(),
                    special: special.get(s).cloned().unwrap_or_default(),
                },
            ))
        })
        .collect())
}

/// **A store's hours on PostgreSQL**, on `c` in whatever transaction it is
/// in: none where the store is not held.
pub(crate) fn pg_hours(
    c: &mut crate::store_pg::Counted<'_>,
    store: &str,
) -> Result<Option<Hours>, String> {
    let pg = crate::feed_pg::pg;
    let Some(row) = c
        .query_opt(
            "SELECT time_zone, last_order_minutes FROM stores WHERE id = $1",
            &[&store],
        )
        .map_err(pg)?
    else {
        return Ok(None);
    };
    let weekly = c
        .query(
            "SELECT weekday, opens_minute, closes_minute FROM store_hours \
             WHERE store = $1 ORDER BY weekday, opens_minute",
            &[&store],
        )
        .map_err(pg)?
        .iter()
        .map(|r| Period {
            weekday: r.get(0),
            opens: r.get(1),
            closes: r.get(2),
        })
        .collect();
    let special = c
        .query(
            "SELECT (day - DATE '1970-01-01')::bigint, opens_minute IS NULL, \
                    coalesce(opens_minute, 0), coalesce(closes_minute, 0) \
             FROM store_special_hours WHERE store = $1 ORDER BY day",
            &[&store],
        )
        .map_err(pg)?
        .iter()
        .map(|r| Special {
            date: r.get(0),
            closed: r.get(1),
            opens: r.get(2),
            closes: r.get(3),
        })
        .collect();
    Ok(Some(Hours {
        zone: row.get(0),
        last_order_minutes: row.get(1),
        weekly,
        special,
    }))
}

/// **A store's hours replaced on PostgreSQL**, on `c`.
pub(crate) fn pg_set_hours(
    c: &mut crate::store_pg::Counted<'_>,
    store: &str,
    h: &Hours,
) -> Result<(), String> {
    let pg = crate::feed_pg::pg;
    c.execute(
        "UPDATE stores SET time_zone = $2, last_order_minutes = $3 WHERE id = $1",
        &[&store, &h.zone, &h.last_order_minutes],
    )
    .map_err(pg)?;
    c.execute("DELETE FROM store_hours WHERE store = $1", &[&store])
        .map_err(pg)?;
    c.execute(
        "DELETE FROM store_special_hours WHERE store = $1",
        &[&store],
    )
    .map_err(pg)?;
    for p in &h.weekly {
        c.execute(
            "INSERT INTO store_hours (store, weekday, opens_minute, closes_minute) \
             VALUES ($1, $2, $3, $4) ON CONFLICT (store, weekday, opens_minute) \
             DO UPDATE SET closes_minute = EXCLUDED.closes_minute",
            &[&store, &p.weekday, &p.opens, &p.closes],
        )
        .map_err(pg)?;
    }
    for s in &h.special {
        let (opens, closes) = if s.closed {
            (None, None)
        } else {
            (Some(s.opens), Some(s.closes))
        };
        c.execute(
            "INSERT INTO store_special_hours (store, day, opens_minute, closes_minute) \
             VALUES ($1, DATE '1970-01-01' + $2::integer, $3, $4) \
             ON CONFLICT (store, day) DO UPDATE SET opens_minute = EXCLUDED.opens_minute, \
                 closes_minute = EXCLUDED.closes_minute",
            &[&store, &(s.date as i32), &opens, &closes],
        )
        .map_err(pg)?;
    }
    Ok(())
}

/// **`POST /bench/hours?store=..&opens=..&closes=..`** (a test's): the store
/// open every day from `opens` to `closes`, in minutes, its cut-off 20
/// minutes, written with its `StoreChanged`, and every session whose open
/// page read what that dropped told (ADR-0219).
pub(crate) fn bench(server: &Server, query: &str) -> Result<(), String> {
    let read = |k: &str| {
        query
            .split('&')
            .find_map(|p| p.strip_prefix(k)?.strip_prefix('='))
            .map(str::to_string)
    };
    let store = read("store").ok_or("`store` names the store")?;
    let minute = |k: &str| {
        read(k)
            .and_then(|v| v.parse::<i64>().ok())
            .ok_or(format!("`{k}`, a minute of the day"))
    };
    let (opens, closes) = (minute("opens")?, minute("closes")?);
    let hours = Hours {
        weekly: (1..=7)
            .map(|weekday| Period {
                weekday,
                opens,
                closes,
            })
            .collect(),
        ..seed()
    };
    let event = (
        "Events.StoreChanged".to_string(),
        vec![Val::String(store.clone())],
    );
    server.store_write(
        "",
        &format!("{}hours#set", crate::store::HOST_OPS),
        &[Val::String(store), hours_val(&hours)],
        std::slice::from_ref(&event),
        |delivered| {
            let reached = server.invalidate_queries("", &[], delivered);
            server
                .telling
                .lock()
                .expect("telling")
                .push((String::new(), reached));
        },
    )?;
    Ok(())
}

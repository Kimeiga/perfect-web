//! **An order placed for a time its reader chooses** (track
//! `scheduled-orders`, NEXT's item 4, its last step; the integrator's design
//! of 2026-10-14): the store's data for it. Which times are offered, and
//! when an order is sent to its stores, is the program's
//! (`examples/store/scheduled.pw`); this holds what it chose and decided.
//!
//! - **The time chosen at checkout**, none for as soon as possible, kept with
//!   the reader's other choices (`checkout.rs`' `Chosen`): `State`'s
//!   `choices` in memory, `checkout_choices.scheduled_for` on PostgreSQL.
//! - **An order's time** ([`OrderTime`]): the reference of the order it is
//!   for, the start of its window, when it is sent to its stores, and its
//!   day and time as its page says them. One for an owner's latest order, as
//!   the payment it awaits is: written with the order where it awaits its
//!   payment (`user-orders#await-payment`), read where it is settled, which
//!   makes it `scheduled` rather than `placed`, and gone once it is sent.
//!   `State`'s `times` in memory, `order_times` on PostgreSQL
//!   (`migrations/store/0016_scheduled_orders.sql`).
//!
//! The operations, registered beside the checkout's (`store.rs`' seams): the
//! reader's `store:data/checkout#set-time`, and `store:data/user-orders#send`,
//! which the clock's run of `send_to_store` calls.

use super::*;
use crate::data::Ops;
use crate::store::{With, Written};
use crate::store_pg::Counted;

/// An order's status once it is paid for a time: placed when it is sent.
pub(crate) const SCHEDULED: &str = "scheduled";

/// The days of the week as the program's `Weekday` names its cases.
const WEEKDAYS: [&str; 7] = [
    "monday",
    "tuesday",
    "wednesday",
    "thursday",
    "friday",
    "saturday",
    "sunday",
];

/// **An order's time**: the reference of the order it is for; the start of
/// its window and when it is sent to its stores, in milliseconds since 1970;
/// and the window's weekday (as `Weekday` names it) and minute of the day in
/// its first store's zone, which its page words.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct OrderTime {
    pub(crate) reference: String,
    pub(crate) at: i64,
    pub(crate) sends: i64,
    pub(crate) weekday: String,
    pub(crate) minute: i64,
}

/// **An order's time as the program's `ScheduledFor` is.**
pub(crate) fn scheduled_val(t: &OrderTime) -> Val {
    Val::Record(vec![
        ("at".into(), Val::S64(t.at)),
        ("sends".into(), Val::S64(t.sends)),
        ("day".into(), Val::Variant(t.weekday.clone(), None)),
        ("time".into(), Val::S64(t.minute)),
    ])
}

/// [`scheduled_val`], read back, for the order `reference`: what
/// `user-orders#await-payment` is passed, none for as soon as possible. A
/// window sent after it starts, a weekday the program has no case for, or a
/// minute past the day is refused.
pub(crate) fn time_from(v: &Val, reference: &str) -> Result<Option<OrderTime>, String> {
    let time = match v {
        Val::Option(None) => return Ok(None),
        Val::Option(Some(time)) => time.as_ref(),
        other => return Err(format!("an order's time is {other:?}")),
    };
    let int = |k: &str| match val_at(time, &[k]) {
        Some(Val::S64(n)) => Ok(*n),
        other => Err(format!("an order's time's `{k}` is {other:?}")),
    };
    let weekday = match val_at(time, &["day"]) {
        Some(Val::Variant(day, None)) if WEEKDAYS.contains(&day.as_str()) => day.clone(),
        other => return Err(format!("an order's time's `day` is {other:?}")),
    };
    let (at, sends, minute) = (int("at")?, int("sends")?, int("time")?);
    if sends > at || !(0..1440).contains(&minute) {
        return Err(format!("an order's time is {time:?}"));
    }
    Ok(Some(OrderTime {
        reference: reference.to_string(),
        at,
        sends,
        weekday,
        minute,
    }))
}

/// **The reader's time, set as their other choices are**: none for as soon
/// as possible. The program holds a time to the times offered before it
/// sets one (`choose_time`).
pub(crate) fn writes(with: With, written: Arc<Mutex<Written>>) -> Ops {
    let mut ops: Ops = BTreeMap::new();
    let (w, wrote) = (with.clone(), written.clone());
    ops.insert(
        "store:data/checkout#set-time".to_string(),
        Arc::new(move |args: &[Val]| match args {
            [Val::String(reader), Val::Option(when)] => {
                let when = match when.as_deref() {
                    None => None,
                    Some(Val::S64(at)) => Some(*at),
                    Some(other) => return Err(format!("checkout#set-time received {other:?}")),
                };
                w(&mut |r| {
                    let mut chosen = r.chosen(reader)?;
                    chosen.when = when;
                    r.set_chosen(reader, &chosen)?;
                    wrote.lock().expect("written").wrote = true;
                    Ok(vec![crate::checkout::chosen_val(&chosen)])
                })
            }
            other => Err(format!("checkout#set-time received {other:?}")),
        }),
    );
    // **The reader's scheduled order, sent to its stores**: placed, where
    // their order is the one `reference` names and is scheduled still, its
    // time gone with it; as it is otherwise, nothing written.
    ops.insert(
        "store:data/user-orders#send".to_string(),
        Arc::new(move |args: &[Val]| {
            let [Val::String(reader), Val::String(reference)] = args else {
                return Err(format!("user-orders#send received {args:?}"));
            };
            with(&mut |r| {
                let Some(order) = r.order(reader)? else {
                    return Ok(crate::store::declared("nothing-to-order"));
                };
                let ours = r
                    .order_time(reader)?
                    .is_some_and(|t| t.reference == *reference);
                if order.status == SCHEDULED && ours {
                    r.set_status(reader, Some("placed"))?;
                    r.set_order_time(reader, None)?;
                    written.lock().expect("written").wrote = true;
                    return Ok(crate::store::ok(Val::Variant("placed".into(), None)));
                }
                Ok(crate::store::ok(crate::store::order_status(
                    r,
                    reader,
                    order.status,
                )?))
            })
        }),
    );
    ops
}

// --- PostgreSQL ----------------------------------------------------------

fn pg(e: postgres::Error) -> String {
    crate::feed_pg::pg(e)
}

/// **An owner's order's time on PostgreSQL**, in one statement.
pub(crate) fn pg_order_time(c: &mut Counted<'_>, owner: &str) -> Result<Option<OrderTime>, String> {
    let row = c
        .query_opt(
            "SELECT reference, at_ms, sends_ms, weekday, minute FROM order_times WHERE owner = $1",
            &[&owner],
        )
        .map_err(pg)?;
    Ok(row.map(|r| OrderTime {
        reference: r.get(0),
        at: r.get(1),
        sends: r.get(2),
        weekday: r.get(3),
        minute: r.get(4),
    }))
}

/// **An owner's order's time written, or none**, in one statement.
pub(crate) fn pg_set_order_time(
    c: &mut Counted<'_>,
    owner: &str,
    time: Option<&OrderTime>,
) -> Result<(), String> {
    match time {
        Some(t) => c.execute(
            "INSERT INTO order_times (owner, reference, at_ms, sends_ms, weekday, minute) \
             VALUES ($1, $2, $3, $4, $5, $6) \
             ON CONFLICT (owner) DO UPDATE SET reference = $2, at_ms = $3, sends_ms = $4, \
                 weekday = $5, minute = $6",
            &[&owner, &t.reference, &t.at, &t.sends, &t.weekday, &t.minute],
        ),
        None => c.execute("DELETE FROM order_times WHERE owner = $1", &[&owner]),
    }
    .map(|_| ())
    .map_err(pg)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn time(at: i64, sends: i64, day: &str, minute: i64) -> Val {
        Val::Option(Some(Box::new(Val::Record(vec![
            ("at".into(), Val::S64(at)),
            ("sends".into(), Val::S64(sends)),
            ("day".into(), Val::Variant(day.into(), None)),
            ("time".into(), Val::S64(minute)),
        ]))))
    }

    /// **An order's time is read as the program gives it, and refused where
    /// no order could have it**: sent after its window starts, a day the
    /// program has no case for, a minute past the day. None is as soon as
    /// possible.
    #[test]
    fn an_orders_time_is_read_as_given_and_refused_where_it_cannot_be() {
        let read = time_from(&time(5_000_000, 2_600_000, "thursday", 1110), "o-1")
            .expect("read")
            .expect("a time");
        assert_eq!(
            read,
            OrderTime {
                reference: "o-1".into(),
                at: 5_000_000,
                sends: 2_600_000,
                weekday: "thursday".into(),
                minute: 1110,
            }
        );
        assert_eq!(
            scheduled_val(&read),
            match time(5_000_000, 2_600_000, "thursday", 1110) {
                Val::Option(Some(v)) => *v,
                _ => unreachable!(),
            }
        );
        assert_eq!(time_from(&Val::Option(None), "o-1"), Ok(None));
        for bad in [
            time(5_000_000, 5_000_001, "thursday", 1110),
            time(5_000_000, 2_600_000, "someday", 1110),
            time(5_000_000, 2_600_000, "thursday", 1440),
        ] {
            assert!(time_from(&bad, "o-1").is_err(), "{bad:?}");
        }
    }
}

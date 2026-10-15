//! **An order placed for a time its reader chooses** (track
//! `scheduled-orders`, the integrator's design of 2026-10-14): the times
//! offered by the cart's stores' hours, compared with the platform's clock
//! (ADR-0304), in each store's zone (ADR-0328); a time held to them when it is
//! chosen and again when the order is placed; and an order for one paid now
//! and sent to its stores when its preparation is due (ADR-0334). On the
//! layer the server's tests run on: memory, or PostgreSQL under
//! `PW_STORE_TEST_LAYER=postgres`. Each rule's test has its control. The
//! stores' zone is San Francisco's; 2027-03-01 is a Monday, before the clocks
//! go forward on the 14th.

use super::store_hours::{at, clock_to, daily, page, set_hours};
use super::store_payments::{pay, paying, paying_with};
use super::*;

const ADD: &str = "store.page.add_to_cart";
const PLACE: &str = "store.page.place_order";
const CHOOSE: &str = "store.page.choose_time";
const SEND: &str = "store.page.send_to_store";
const SWEEP: &str = "store.page.expire_order";

const MINUTE: i64 = 60_000;

fn served() -> Served {
    served_from_patches_in("examples", |app| app.to_string(), &[])
}

/// **A time chosen** by session `a`'s reader, as the page of times presses
/// it: `None` for as soon as possible.
fn choose(s: &Server, when: Option<i64>, interaction: &str) -> Answered {
    let when = Val::Option(when.map(|at| Box::new(Val::S64(at))));
    s.command_answered(CHOOSE, "a", &[when], Some(interaction))
        .expect("answered")
}

/// **The reader's cart placed**, as the cart's page presses it.
fn place(s: &Server, interaction: &str) -> Answered {
    s.command_answered(PLACE, "a", &[s.shown_total("a")], Some(interaction))
        .expect("answered")
}

/// **Every window offered to session `a`'s reader now**, earliest first, as
/// the page of times shows them day by day: each window's start and its
/// words with its day.
fn offered(s: &Server) -> Vec<(i64, String)> {
    let owner = s.owner("a");
    let times = |day: Option<&str>| {
        let day = Val::Option(day.map(|d| Box::new(Val::String(d.to_string()))));
        s.query(
            "store.page.TimesOffered",
            "a",
            &[Val::String(owner.clone()), day],
        )
        .expect("the times offered")
    };
    let first = times(None);
    let Some(Val::List(days)) = val_at(&first, &["days"]) else {
        panic!("no days: {first:?}")
    };
    let mut out = Vec::new();
    for day in days {
        let Some(Val::String(key)) = val_at(day, &["key"]) else {
            panic!("a day without its key: {day:?}")
        };
        let shown = times(Some(key));
        let Some(Val::List(windows)) = val_at(&shown, &["windows"]) else {
            panic!("no windows: {shown:?}")
        };
        for w in windows {
            match (val_at(w, &["start"]), val_at(w, &["whole"])) {
                (Some(Val::S64(start)), Some(Val::String(whole))) => {
                    out.push((*start, whole.clone()))
                }
                other => panic!("a window is {other:?}"),
            }
        }
    }
    out
}

/// What the checkout says of when, for session `a`'s reader.
fn when_said(s: &Server) -> String {
    let checkout = s
        .query("store.page.Checkout", "a", &[Val::String(s.owner("a"))])
        .expect("the checkout");
    match val_at(&checkout, &["when-said"]) {
        Some(Val::String(said)) => said.clone(),
        other => panic!("the checkout's when is {other:?}"),
    }
}

/// **The reader's order as the layer reads it**: its case, and a scheduled
/// one's time and when it is sent.
fn order(s: &Server) -> Option<(String, Option<(i64, i64)>)> {
    let owner = s.owner("a");
    match s.layer_read("a", "store:data/user-orders#current", &owner) {
        Val::Result(Ok(Some(v))) => match *v {
            Val::Option(None) => None,
            Val::Option(Some(case)) => match *case {
                Val::Variant(case, Some(time)) if case == "scheduled" => {
                    match (val_at(&time, &["at"]), val_at(&time, &["sends"])) {
                        (Some(Val::S64(at)), Some(Val::S64(sends))) => {
                            Some((case, Some((*at, *sends))))
                        }
                        other => panic!("a scheduled order's time is {other:?}"),
                    }
                }
                Val::Variant(case, _) => Some((case, None)),
                other => panic!("user-orders#current answered {other:?}"),
            },
            other => panic!("user-orders#current answered {other:?}"),
        },
        other => panic!("user-orders#current answered {other:?}"),
    }
}

/// **The host's clock, moved `ms` on**, as `/bench/clock` moves it: a run
/// due a duration after its command's commit is due that long after now.
fn later(s: &Server, ms: i64) {
    s.query_clock.0.advance(u64::try_from(ms).expect("forward"));
}

/// Every run the host keeps, earliest first.
fn runs(s: &Server) -> Vec<pw_materialize::ScheduledRun> {
    s.runs().all_runs().expect("the runs read")
}

/// The answer a refusal is, as the page reads it.
fn refused(case: serde_json::Value) -> Option<serde_json::Value> {
    Some(serde_json::json!({ "$case": "err", "value": case }))
}

/// **Open the day round, as the stores are first**: every quarter hour from
/// the first more than an hour ahead to the last a week ahead, said in the
/// store's zone. The control: a quarter of an hour on, the first is gone,
/// within its hour, and one more is a week ahead.
#[test]
fn the_seeds_hours_offer_every_quarter_hour_from_an_hour_to_a_week_ahead() {
    let s = served();
    clock_to(&s, at(2027, 3, 1, 10, 7));
    s.command(ADD, "a", &add_shown("espresso", 1), false)
        .expect("added");
    let times = offered(&s);
    assert_eq!(times.len(), 668, "{:?}", &times[..3]);
    assert_eq!(
        times[0],
        (at(2027, 3, 1, 11, 15), "Today, 11:15 AM".to_string())
    );
    assert!(times.windows(2).all(|w| w[1].0 - w[0].0 == 15 * MINUTE));
    assert!(times.contains(&(at(2027, 3, 2, 0, 0), "Tomorrow, 12:00 AM".to_string())));
    assert!(times.contains(&(at(2027, 3, 4, 12, 0), "Thursday, 12:00 PM".to_string())));
    assert_eq!(
        times.last().cloned(),
        Some((at(2027, 3, 8, 10, 0), "Next Monday, 10:00 AM".to_string()))
    );
    clock_to(&s, at(2027, 3, 1, 10, 15));
    let later = offered(&s);
    assert_eq!(later[0].0, at(2027, 3, 1, 11, 30), "11:15 is an hour away");
    assert_eq!(later.last().map(|t| t.0), Some(at(2027, 3, 8, 10, 15)));
}

/// **A page of times is told when the first window's hour comes**: the
/// windows are compared with the clock (ADR-0304), so the page is held until
/// an hour before the first, and read again then.
#[test]
fn the_page_of_times_is_held_until_the_first_windows_hour() {
    let s = served();
    clock_to(&s, at(2027, 3, 1, 10, 7));
    s.command(ADD, "a", &add_shown("espresso", 1), false)
        .expect("added");
    let (html, _, _, _) = s
        .serve_document_settled("a", "store.page.TimePage", &Params::new(), &[])
        .expect("served");
    assert!(visible(&html).contains("11:15 AM"), "{html}");
    let doc = latest(&s.pending.lock().expect("pending"), "a");
    assert_eq!(s.timers.at(&doc), Some(at(2027, 3, 1, 10, 15)));
}

/// **A store open 07:00 to 19:00 offers the windows starting while it is
/// open**: from 07:00 to 18:45 each day, none at 19:00 or before 07:00; at
/// 18:00 today none is left, its 19:15 closed.
#[test]
fn a_stores_hours_offer_the_windows_it_is_open_at() {
    let s = served();
    set_hours(&s, "47", &daily(7 * 60, 19 * 60));
    clock_to(&s, at(2027, 3, 1, 18, 0));
    s.command(ADD, "a", &add_shown("espresso", 1), false)
        .expect("added");
    let times = offered(&s);
    let starts: Vec<i64> = times.iter().map(|t| t.0).collect();
    assert_eq!(
        times[0],
        (at(2027, 3, 2, 7, 0), "Tomorrow, 7:00 AM".to_string())
    );
    assert!(starts.contains(&at(2027, 3, 2, 18, 45)));
    for closed in [
        at(2027, 3, 2, 19, 0),
        at(2027, 3, 3, 6, 45),
        at(2027, 3, 1, 19, 15),
    ] {
        assert!(!starts.contains(&closed), "{closed}");
    }
    assert_eq!(
        times.last().cloned(),
        Some((at(2027, 3, 8, 18, 0), "Next Monday, 6:00 PM".to_string()))
    );
    assert_eq!(times.len(), 6 * 48 + 45);
}

/// **Hours past midnight offer the windows across it**: open 22:00 to 02:00,
/// 23:45 and 01:45 are offered, the second the next day's; 02:00 and 21:45
/// are not.
#[test]
fn hours_past_midnight_offer_windows_across_it() {
    let s = served();
    set_hours(&s, "48", &daily(22 * 60, 2 * 60));
    clock_to(&s, at(2027, 3, 1, 12, 0));
    s.command(ADD, "a", &add_shown("drip", 1), false)
        .expect("added");
    let times = offered(&s);
    for (open, said) in [
        (at(2027, 3, 1, 22, 0), "Today, 10:00 PM"),
        (at(2027, 3, 1, 23, 45), "Today, 11:45 PM"),
        (at(2027, 3, 2, 1, 45), "Tomorrow, 1:45 AM"),
    ] {
        assert!(times.contains(&(open, said.to_string())), "{said}");
    }
    let starts: Vec<i64> = times.iter().map(|t| t.0).collect();
    for closed in [at(2027, 3, 2, 2, 0), at(2027, 3, 1, 21, 45)] {
        assert!(!starts.contains(&closed), "{closed}");
    }
    assert_eq!(times.len(), 7 * 16, "sixteen a night, a week of nights");
}

/// The cart's two stores: Blue Bottle (47) open 07:00 to 19:00 in San
/// Francisco, Harbor Coffee (48) 09:00 to 17:00 in New York, which is 06:00
/// to 14:00 in San Francisco.
fn two_zones(s: &Server) {
    set_hours(s, "47", &daily(7 * 60, 19 * 60));
    let mut new_york = daily(9 * 60, 17 * 60);
    new_york.zone = "America/New_York".to_string();
    set_hours(s, "48", &new_york);
    clock_to(s, at(2027, 3, 1, 20, 0));
}

/// **A cart of two stores in two zones is offered the windows both are open
/// at**, each by its hours in its own zone: 07:00 to 13:45 in San Francisco,
/// said in the zone of the store the cart lists first. The control: the
/// other store listed first says the same windows in New York's.
#[test]
fn a_cart_across_two_zones_is_offered_the_windows_both_are_open_at() {
    let s = served();
    two_zones(&s);
    s.command(ADD, "a", &add_shown("espresso", 1), false)
        .expect("added");
    s.command(ADD, "a", &add_shown("drip", 1), false)
        .expect("added");
    let times = offered(&s);
    assert_eq!(
        times[0],
        (at(2027, 3, 2, 7, 0), "Tomorrow, 7:00 AM".to_string())
    );
    assert!(times.contains(&(at(2027, 3, 2, 13, 45), "Tomorrow, 1:45 PM".to_string())));
    let starts: Vec<i64> = times.iter().map(|t| t.0).collect();
    for closed in [at(2027, 3, 2, 14, 0), at(2027, 3, 2, 6, 45)] {
        assert!(!starts.contains(&closed), "{closed}");
    }
    assert_eq!(times.len(), 7 * 28);
    s.command("store.page.clear_cart", "a", &[], false)
        .expect("cleared");
    s.command(ADD, "a", &add_shown("drip", 1), false)
        .expect("added");
    s.command(ADD, "a", &add_shown("espresso", 1), false)
        .expect("added");
    let said_in_new_york = offered(&s);
    assert_eq!(
        said_in_new_york[0],
        (at(2027, 3, 2, 7, 0), "Tomorrow, 10:00 AM".to_string())
    );
    assert_eq!(said_in_new_york.len(), times.len());
}

/// **A time is held to the times offered when it is chosen**: one within the
/// hour, one between two quarter hours, one more than a week ahead and one
/// the store is closed at are each refused by name, and nothing is chosen.
/// The control: an offered one is chosen, said at checkout, and as soon as
/// possible chosen again after.
#[test]
fn a_time_not_offered_is_refused_when_chosen() {
    let s = served();
    set_hours(&s, "47", &daily(7 * 60, 19 * 60));
    clock_to(&s, at(2027, 3, 1, 10, 7));
    s.command(ADD, "a", &add_shown("espresso", 1), false)
        .expect("added");
    assert_eq!(when_said(&s), "As soon as possible");
    for (i, not) in [
        at(2027, 3, 1, 11, 0),
        at(2027, 3, 1, 12, 7),
        at(2027, 3, 8, 10, 15),
        at(2027, 3, 1, 19, 0),
    ]
    .into_iter()
    .enumerate()
    {
        let answered = choose(&s, Some(not), &format!("not-{i}"));
        assert!(!answered.committed, "{not}");
        assert_eq!(
            answered.result,
            refused(serde_json::json!({ "$case": "not-offered", "value": not })),
        );
    }
    assert_eq!(when_said(&s), "As soon as possible", "nothing chosen");
    let chosen = choose(&s, Some(at(2027, 3, 1, 12, 0)), "offered");
    assert!(chosen.committed, "{:?}", chosen.result);
    assert_eq!(when_said(&s), "Scheduled for Mon 12:00 PM");
    assert!(
        page(&s, "a", "/cart").contains("Scheduled for Mon 12:00 PM Choose a time"),
        "the cart says it"
    );
    let soon = choose(&s, None, "soon");
    assert!(soon.committed, "{:?}", soon.result);
    assert_eq!(when_said(&s), "As soon as possible");
}

/// **A time no longer offered is refused when the order is placed**, by
/// name: chosen at 10:07 for 12:00, it is an hour away at 11:00, and the
/// checkout says so; the times offered now begin at 12:15. The control: at
/// 10:59 it is placed, scheduled for 12:00.
#[test]
fn a_time_no_longer_offered_is_refused_when_the_order_is_placed() {
    let s = served();
    clock_to(&s, at(2027, 3, 1, 10, 7));
    s.command(ADD, "a", &add_shown("espresso", 1), false)
        .expect("added");
    let noon = at(2027, 3, 1, 12, 0);
    assert!(choose(&s, Some(noon), "noon").committed);
    clock_to(&s, at(2027, 3, 1, 11, 0));
    let late = place(&s, "late");
    assert!(!late.committed);
    assert_eq!(
        late.result,
        refused(serde_json::json!({ "$case": "not-offered", "value": noon })),
    );
    assert_eq!(
        when_said(&s),
        "Mon 12:00 PM is no longer offered: choose another time."
    );
    assert_eq!(offered(&s)[0].0, at(2027, 3, 1, 12, 15));
    assert_eq!(order(&s), None, "nothing placed");

    let s = served();
    clock_to(&s, at(2027, 3, 1, 10, 7));
    s.command(ADD, "a", &add_shown("espresso", 1), false)
        .expect("added");
    assert!(choose(&s, Some(noon), "noon").committed);
    clock_to(&s, at(2027, 3, 1, 10, 59));
    let placed = place(&s, "in-time");
    assert!(placed.committed, "{:?}", placed.result);
    assert_eq!(order(&s).map(|(case, _)| case), Some("scheduled".into()));
}

/// **A time a store's hours no longer cover is refused**: chosen for 18:00,
/// the store's hours then end at 17:00. The control: the same time placed
/// while the hours covered it (above).
#[test]
fn a_time_the_hours_no_longer_cover_is_refused() {
    let s = served();
    clock_to(&s, at(2027, 3, 1, 10, 7));
    s.command(ADD, "a", &add_shown("espresso", 1), false)
        .expect("added");
    let six = at(2027, 3, 1, 18, 0);
    assert!(choose(&s, Some(six), "six").committed);
    set_hours(&s, "47", &daily(7 * 60, 17 * 60));
    let refused_now = place(&s, "place");
    assert_eq!(
        refused_now.result,
        refused(serde_json::json!({ "$case": "not-offered", "value": six })),
    );
}

/// **An order for a time is taken while its store is closed now**, for a
/// time it is open: at 20:00, the store closed until 07:00, as soon as
/// possible is refused `Closed`, and tomorrow at 09:00 is placed, scheduled,
/// paid at once where the deployment takes no payments.
#[test]
fn an_order_for_a_time_is_taken_while_its_store_is_closed() {
    let s = served();
    set_hours(&s, "47", &daily(7 * 60, 19 * 60));
    clock_to(&s, at(2027, 3, 1, 20, 0));
    s.command(ADD, "a", &add_shown("espresso", 1), false)
        .expect("added");
    let soon = place(&s, "soon");
    assert_eq!(
        soon.result,
        refused(serde_json::json!({ "$case": "closed",
            "value": { "$case": "closed-now", "value": { "$case": "some",
                "value": { "day": { "$case": "tomorrow" }, "at": 420 } } } })),
    );
    let nine = at(2027, 3, 2, 9, 0);
    assert!(choose(&s, Some(nine), "nine").committed);
    let placed = place(&s, "nine");
    assert!(placed.committed, "{:?}", placed.result);
    let (case, time) = order(&s).expect("an order");
    assert_eq!(case, "scheduled");
    assert_eq!(time.map(|t| t.0), Some(nine));
    assert!(s.cart_lines("a").is_empty(), "the cart is the order's");
    assert_eq!(when_said(&s), "As soon as possible", "the choice is spent");
}

/// **An order is sent to its stores when its preparation is due, and not
/// before**: the window less the estimate's most, 35 minutes (25 to 35, no
/// address chosen), and five more. Paid at the provider, it is scheduled and
/// its sweep cancelled; its run is due at 11:20 for 12:00, the reader's; at
/// 11:19:59 nothing is due, and at 11:20 it is placed, and the reader's open
/// order page is told, once.
#[test]
fn an_order_is_sent_to_its_stores_when_its_preparation_is_due() {
    paying(|s, _| {
        clock_to(s, at(2027, 3, 1, 10, 7));
        s.command(ADD, "a", &add_shown("espresso", 1), false)
            .expect("added");
        let noon = at(2027, 3, 1, 12, 0);
        assert!(choose(s, Some(noon), "noon").committed);
        assert!(place(s, "place").committed);
        let Some((case, None)) = order(s) else {
            panic!("{:?}", order(s))
        };
        assert_eq!(case, "awaiting-payment");
        assert_eq!(
            runs(s)
                .iter()
                .map(|r| r.command.as_str())
                .collect::<Vec<_>>(),
            [SWEEP],
            "told nothing until paid"
        );
        let (html, _, _, _) = s
            .serve_document_settled("a", "store.page.OrderPage", &Params::new(), &[])
            .expect("served");
        let doc = latest(&s.pending.lock().expect("pending"), "a");
        let page_url = html
            .split("src=\"")
            .nth(1)
            .and_then(|rest| rest.split('"').next())
            .expect("the provider's page")
            .to_string();
        let paid = pay(&page_url, "4242424242424242");
        assert!(paid.contains("Paid."), "{paid}");
        let sends = noon - 40 * MINUTE;
        assert_eq!(order(s), Some(("scheduled".into(), Some((noon, sends)))));
        let pending = runs(s);
        assert_eq!(pending.len(), 1, "{pending:?}");
        assert_eq!(pending[0].command, SEND);
        assert_eq!(pending[0].due_ms, sends);
        assert_eq!(pending[0].user, s.identity.principals().user_of("a"));
        // The open page is told it is scheduled once the webhook's
        // connection is answered (ADR-0219), on that connection: waited for
        // here, so that telling is done before the clock moves.
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
        while !sets_of(s, &doc)
            .iter()
            .any(|set| visible(&written(set)).contains("Scheduled for Mon 12:00 PM."))
        {
            assert!(
                std::time::Instant::now() < deadline,
                "the open page was not told: {:?}",
                sets_of(s, &doc)
            );
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        clock_to(s, sends - 1000);
        assert_eq!(s.run_due().0, 0, "a second before");
        assert_eq!(order(s).map(|(case, _)| case), Some("scheduled".into()));
        clock_to(s, sends);
        assert_eq!(s.run_due().0, 1, "due");
        assert_eq!(order(s), Some(("placed".into(), None)));
        assert!(runs(s).is_empty(), "run once");
        let told: Vec<String> = sets_of(s, &doc).iter().map(written).collect();
        assert_eq!(
            told.iter()
                .filter(|set| set.contains("Placed: the store has your order."))
                .count(),
            1,
            "{told:?}"
        );
        clock_to(s, noon);
        assert_eq!(s.run_due().0, 0, "nothing more");
    });
}

/// **The store is told the window less the estimate's most and five
/// minutes**: the reader's estimate 20 to 50 minutes sends the order 55
/// minutes before its window, where the deployment takes no payments. The
/// control: no estimate set, 40 minutes (above).
#[test]
fn an_order_is_sent_its_estimates_most_and_five_minutes_before() {
    let s = served();
    clock_to(&s, at(2027, 3, 1, 10, 7));
    s.untold(
        "a",
        "store:host/estimates#set",
        &[
            Val::String("a".into()),
            Val::S64(20),
            Val::Option(Some(Box::new(Val::S64(50)))),
        ],
    );
    s.command(ADD, "a", &add_shown("espresso", 1), false)
        .expect("added");
    let noon = at(2027, 3, 1, 12, 0);
    assert!(choose(&s, Some(noon), "noon").committed);
    assert!(place(&s, "place").committed);
    let sends = noon - 55 * MINUTE;
    assert_eq!(order(&s), Some(("scheduled".into(), Some((noon, sends)))));
    assert_eq!(runs(&s).first().map(|r| r.due_ms), Some(sends));
}

/// **An order for a time awaiting its payment is swept as any is**
/// (ADR-0334): unpaid thirty minutes on, it is taken back, and nothing was
/// scheduled to send it. The control: one paid is sent (above).
#[test]
fn an_order_for_a_time_awaiting_its_payment_is_swept() {
    paying(|s, _| {
        clock_to(s, at(2027, 3, 1, 10, 7));
        s.command(ADD, "a", &add_shown("espresso", 1), false)
            .expect("added");
        assert!(choose(s, Some(at(2027, 3, 1, 15, 0)), "three").committed);
        assert!(place(s, "place").committed);
        later(s, 30 * MINUTE);
        assert_eq!(s.run_due().0, 1, "the sweep");
        assert_eq!(order(s), Some(("canceled".into(), None)));
        assert!(runs(s).is_empty(), "{:?}", runs(s));
        assert_eq!(s.cart_lines("a"), vec![("espresso".to_string(), 1)]);
    });
}

/// **A second order waits for a scheduled one to be sent**: the layers hold
/// one current order a reader, which a second would replace before it is
/// sent. Refused by name; the control, once sent, another is placed.
#[test]
fn a_second_order_waits_for_a_scheduled_one_to_be_sent() {
    let s = served();
    clock_to(&s, at(2027, 3, 1, 10, 7));
    s.command(ADD, "a", &add_shown("espresso", 1), false)
        .expect("added");
    let noon = at(2027, 3, 1, 12, 0);
    assert!(choose(&s, Some(noon), "noon").committed);
    assert!(place(&s, "first").committed);
    s.command(ADD, "a", &add_shown("cortado", 1), false)
        .expect("added");
    let second = place(&s, "second");
    assert!(!second.committed);
    assert_eq!(
        second.result,
        refused(serde_json::json!({ "$case": "scheduled" }))
    );
    assert_eq!(s.cart_lines("a"), vec![("cortado".to_string(), 1)]);
    assert!(
        page(&s, "a", "/order").contains("Scheduled for Mon 12:00 PM."),
        "the first stands"
    );
    clock_to(&s, noon - 40 * MINUTE);
    assert_eq!(s.run_due().0, 1);
    let third = place(&s, "third");
    assert!(third.committed, "{:?}", third.result);
}

/// A test's: the store's send of `order`, scheduled five minutes on.
const SEND_IN_FIVE: &str = r#"
// A test's: `send_to_store(order)` scheduled five minutes on.
command send_in_five(order: String)
    -> Result<Int, CartError>
    idempotent_by InteractionId
{
    schedule send_to_store(order) after minutes(5)
    Ok(1)
}
"#;

/// **A send changes only the order it was scheduled for**: one for another
/// order's reference leaves the reader's scheduled order as it is. The
/// control: its own, when due, places it.
#[test]
fn a_send_places_only_its_own_order() {
    let s = served_from_patches_in("examples", |app| format!("{app}{SEND_IN_FIVE}"), &[]);
    clock_to(&s, at(2027, 3, 1, 10, 7));
    s.command(ADD, "a", &add_shown("espresso", 1), false)
        .expect("added");
    let noon = at(2027, 3, 1, 12, 0);
    assert!(choose(&s, Some(noon), "noon").committed);
    assert!(place(&s, "place").committed);
    let other = s
        .command_answered(
            "store.page.send_in_five",
            "a",
            &[Val::String("o-another".into())],
            Some("other"),
        )
        .expect("answered");
    assert!(other.committed, "{:?}", other.result);
    later(&s, 5 * MINUTE);
    assert_eq!(s.run_due().0, 1, "another order's send");
    assert_eq!(order(&s).map(|(case, _)| case), Some("scheduled".into()));
    clock_to(&s, noon - 40 * MINUTE);
    assert_eq!(s.run_due().0, 1, "its own");
    assert_eq!(order(&s).map(|(case, _)| case), Some("placed".into()));
}

/// **A reader's press of the send is a command that is not there**: what the
/// clock runs, pressed, places nothing. The control: the same route takes a
/// command a reader presses.
#[test]
fn a_readers_press_of_the_send_is_not_a_command() {
    let s = served();
    clock_to(&s, at(2027, 3, 1, 10, 7));
    s.command(ADD, "a", &add_shown("espresso", 1), false)
        .expect("added");
    assert!(choose(&s, Some(at(2027, 3, 1, 12, 0)), "noon").committed);
    assert!(place(&s, "place").committed);
    let forged = posted(
        &s,
        "/command/store.page.send_to_store",
        "a",
        "forged-1",
        r#"["o-0000000000000000"]"#,
    );
    assert!(forged.starts_with("HTTP/1.1 404"), "{forged}");
    assert_eq!(order(&s).map(|(case, _)| case), Some("scheduled".into()));
    let control = posted(&s, "/command/store.page.clear_cart", "a", "control-1", "[]");
    assert!(control.contains("\"committed\":true"), "{control}");
}

/// **The time chosen is the reader's, kept with the cart** in the layer, as
/// their tip is, and read back as it was set; as soon as possible is none.
#[test]
fn the_time_chosen_is_kept_with_the_readers_choices() {
    let s = served();
    clock_to(&s, at(2027, 3, 1, 10, 7));
    s.command(ADD, "a", &add_shown("espresso", 1), false)
        .expect("added");
    let chosen = |s: &Server| {
        let read = s.layer_read("a", "store:data/checkout#chosen", &s.owner("a"));
        val_at(&read, &["when"]).cloned()
    };
    assert_eq!(chosen(&s), Some(Val::Option(None)));
    let noon = at(2027, 3, 1, 12, 0);
    assert!(choose(&s, Some(noon), "noon").committed);
    assert_eq!(
        chosen(&s),
        Some(Val::Option(Some(Box::new(Val::S64(noon)))))
    );
    assert!(
        s.command(
            "store.page.choose_tip",
            "a",
            &[Val::Variant("fifteen".into(), None)],
            false
        )
        .is_ok()
    );
    assert_eq!(
        chosen(&s),
        Some(Val::Option(Some(Box::new(Val::S64(noon))))),
        "a tip keeps it"
    );
    assert!(choose(&s, None, "soon").committed);
    assert_eq!(chosen(&s), Some(Val::Option(None)));
}

/// **The page of times chooses**: its first window pressed as a browser
/// presses it, through the route a page's press reaches; the cart says the
/// time. Its control: as soon as possible pressed after.
#[test]
fn the_page_of_times_lists_a_days_windows_and_its_days() {
    let s = served();
    set_hours(&s, "47", &daily(7 * 60, 19 * 60));
    clock_to(&s, at(2027, 3, 1, 20, 0));
    s.command(ADD, "a", &add_shown("espresso", 1), false)
        .expect("added");
    let shown = page(&s, "a", "/cart/when");
    assert!(
        shown.contains("Tomorrow Wednesday Thursday Friday Saturday Sunday Next Monday"),
        "{shown}"
    );
    assert!(shown.contains("Tomorrow 7:00 AM 7:15 AM"), "{shown}");
    let friday = (pw_time::days(2027, 3, 5)).to_string();
    let shown = page(&s, "a", &format!("/cart/when?day={friday}"));
    assert!(shown.contains("Friday 7:00 AM"), "{shown}");
    assert!(!shown.contains("Tomorrow 7:00 AM"), "{shown}");
    let pressed = posted(
        &s,
        "/command/store.page.choose_time",
        "a",
        "pressed-1",
        &format!(r#"[{{"$case":"some","value":{}}}]"#, at(2027, 3, 5, 7, 0)),
    );
    assert!(pressed.contains("\"committed\":true"), "{pressed}");
    assert!(
        page(&s, "a", "/cart").contains("Scheduled for Fri 7:00 AM"),
        "the cart says it"
    );
}

/// **An order for a time is placed where the deployment takes payments,
/// awaiting its payment as any is** (ADR-0332), the payment for its whole
/// total; it is the cart's lines, and the cart is empty after. Paid, the
/// order page says when, and that its total was charged.
#[test]
fn an_order_for_a_time_is_paid_now_and_its_page_says_when() {
    paying_with(
        |app| app.to_string(),
        |s, _| {
            clock_to(s, at(2027, 3, 1, 10, 7));
            s.command(ADD, "a", &add_shown("espresso", 2), false)
                .expect("added");
            assert!(choose(s, Some(at(2027, 3, 2, 18, 30)), "evening").committed);
            assert!(place(s, "place").committed);
            assert!(s.cart_lines("a").is_empty());
            let (html, _, _, _) = s
                .serve_document_settled("a", "store.page.OrderPage", &Params::new(), &[])
                .expect("served");
            assert!(visible(&html).contains("Awaiting your payment"), "{html}");
            let page_url = html
                .split("src=\"")
                .nth(1)
                .and_then(|rest| rest.split('"').next())
                .expect("the provider's page")
                .to_string();
            assert!(pay(&page_url, "4242424242424242").contains("Paid."));
            let shown = page(s, "a", "/order");
            assert!(shown.contains("Scheduled for Tue 6:30 PM."), "{shown}");
            assert!(shown.contains("Total charged: $"), "{shown}");
        },
    );
}

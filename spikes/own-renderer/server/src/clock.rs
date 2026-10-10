//! **The platform's clock, as a host answers it** (ADR-0304): whether an
//! instant has passed, a zone's date, and the zone database's conversions;
//! what each value read holds until; and the documents to read again then.
//!
//! A value kept or shown reads the wall clock only by comparing it
//! (`clock.passed`, `clock.today_in`), so the host knows the earliest
//! instant at which it would read otherwise: each comparison notes it in the
//! read that asked ([`noting`]), a kept value is held until it, and each live
//! document that shows one is read again at it ([`Timers`]).

use std::cell::RefCell;
use std::cmp::Reverse;
use std::collections::{BTreeMap, BinaryHeap};
use std::sync::{Arc, Condvar, Mutex};

use pw_host::engine::{HostFn, Val};

use crate::Doc;

thread_local! {
    /// The earliest instant each read under way on this thread holds until,
    /// innermost last.
    static UNTIL: RefCell<Vec<Option<i64>>> = const { RefCell::new(Vec::new()) };
}

/// **Run `read`, and say what it holds until**: the earliest instant a
/// comparison it made, or a kept value it was given, would answer
/// otherwise; `None` where it compared nothing. A read within a read notes
/// its instant in both.
pub fn noting<T>(read: impl FnOnce() -> T) -> (T, Option<i64>) {
    UNTIL.with(|u| u.borrow_mut().push(None));
    let out = read();
    let until = UNTIL.with(|u| u.borrow_mut().pop().flatten());
    if let Some(at) = until {
        note(at);
    }
    (out, until)
}

/// The read under way holds until `at` at the latest.
pub fn note(at: i64) {
    UNTIL.with(|u| {
        if let Some(last) = u.borrow_mut().last_mut() {
            *last = Some(last.map_or(at, |was| was.min(at)));
        }
    });
}

/// **The clock's operations, for one read at `now`** (the host's wall
/// milliseconds): every comparison in one read answers for one instant.
pub fn operations(now: i64, zones: &Arc<pw_time::Database>) -> Vec<(String, HostFn)> {
    let zone = {
        let zones = zones.clone();
        move |v: &Val| match v {
            Val::String(name) => zones.zone(name).map_err(|e| e.to_string()),
            other => Err(format!("a time zone's name, not {other:?}")),
        }
    };
    let int = |v: &Val| match v {
        Val::S64(n) => Ok(*n),
        other => Err(format!("an instant or a date, not {other:?}")),
    };
    let seconds = |ms: i64| ms.div_euclid(1000);
    let mut ops: Vec<(String, HostFn)> = Vec::new();
    ops.push((
        "pw:host/clock#passed".into(),
        Arc::new(move |args: &[Val]| {
            let at = int(args.first().ok_or("passed(at)")?)?;
            if at > now {
                note(at);
            }
            Ok(vec![Val::Bool(now >= at)])
        }),
    ));
    {
        let zone = zone.clone();
        ops.push((
            "pw:host/clock#today".into(),
            Arc::new(move |args: &[Val]| {
                let zone = zone(args.first().ok_or("today_in(zone)")?)?;
                let (day, _) = zone.local(seconds(now));
                note(zone.next_day(seconds(now)) * 1000);
                Ok(vec![Val::S64(day)])
            }),
        ));
    }
    {
        let zone = zone.clone();
        ops.push((
            "pw:host/clock#at".into(),
            Arc::new(move |args: &[Val]| {
                let [date, time, z] = args else {
                    return Err("at(date, time, zone)".into());
                };
                let zone = zone(z)?;
                Ok(vec![Val::S64(
                    zone.instant(int(date)?, int(time)? * 60) * 1000,
                )])
            }),
        ));
    }
    {
        let zone = zone.clone();
        ops.push((
            "pw:host/clock#date".into(),
            Arc::new(move |args: &[Val]| {
                let [at, z] = args else {
                    return Err("date_of(at, zone)".into());
                };
                Ok(vec![Val::S64(zone(z)?.local(seconds(int(at)?)).0)])
            }),
        ));
    }
    ops.push((
        "pw:host/clock#time".into(),
        Arc::new(move |args: &[Val]| {
            let [at, z] = args else {
                return Err("time_of(at, zone)".into());
            };
            Ok(vec![Val::S64(zone(z)?.local(seconds(int(at)?)).1 / 60)])
        }),
    ));
    ops
}

/// **The documents to read again, each at the instant what it shows holds
/// until.** One instant a document, its earliest; one read again tells it
/// what changed and sets the next.
#[derive(Default)]
pub struct Timers {
    due: Mutex<Due>,
    /// Woken when an instant is set, and when the clock is moved.
    pub changed: Condvar,
}

#[derive(Default)]
pub struct Due {
    queue: BinaryHeap<Reverse<(i64, Doc)>>,
    at: BTreeMap<Doc, i64>,
}

impl Timers {
    /// `doc` shows what holds until `until`.
    pub fn hold(&self, doc: &Doc, until: i64) {
        let mut due = self.due.lock().expect("timers");
        if due.at.get(doc).is_some_and(|&at| at <= until) {
            return;
        }
        due.at.insert(doc.clone(), until);
        due.queue.push(Reverse((until, doc.clone())));
        drop(due);
        self.changed.notify_all();
    }

    /// Each document whose instant `now` has reached, taken; and the next
    /// instant one is set for.
    pub fn take_due(&self, now: i64) -> (Vec<Doc>, Option<i64>) {
        let mut due = self.due.lock().expect("timers");
        let mut taken = Vec::new();
        while let Some(Reverse((at, doc))) = due.queue.peek().cloned() {
            if at > now {
                break;
            }
            due.queue.pop();
            // An instant a later read replaced is not the document's.
            if due.at.get(&doc) == Some(&at) {
                due.at.remove(&doc);
                taken.push(doc);
            }
        }
        let next = due.queue.peek().map(|Reverse((at, _))| *at);
        (taken, next)
    }

    /// The instant `doc` is to be read again at, where one is set.
    #[cfg(test)]
    pub fn at(&self, doc: &Doc) -> Option<i64> {
        self.due.lock().expect("timers").at.get(doc).copied()
    }

    /// Wait for a change, or `ms` at most.
    pub fn wait(&self, ms: u64) {
        let due = self.due.lock().expect("timers");
        let _ = self
            .changed
            .wait_timeout(due, std::time::Duration::from_millis(ms))
            .expect("timers");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_read_holds_until_its_earliest_comparison() {
        let ((), until) = noting(|| {
            note(500);
            note(300);
            note(900);
        });
        assert_eq!(until, Some(300));
        let ((), until) = noting(|| {});
        assert_eq!(until, None, "nothing compared, held by nothing");
    }

    #[test]
    fn a_read_within_a_read_holds_both() {
        let (inner, outer) = noting(|| {
            note(800);
            noting(|| note(400)).1
        });
        assert_eq!((inner, outer), (Some(400), Some(400)));
    }

    #[test]
    fn passed_answers_for_its_instant_and_notes_what_is_ahead() {
        let zones = Arc::new(pw_time::Database::system());
        let ops: BTreeMap<String, HostFn> = operations(1_000, &zones).into_iter().collect();
        let passed = &ops["pw:host/clock#passed"];
        let (answers, until) = noting(|| {
            [
                passed(&[Val::S64(999)]).unwrap(),
                passed(&[Val::S64(1_000)]).unwrap(),
                passed(&[Val::S64(5_000)]).unwrap(),
                passed(&[Val::S64(2_000)]).unwrap(),
            ]
        });
        assert_eq!(
            answers.map(|a| a[0].clone()),
            [
                Val::Bool(true),
                Val::Bool(true),
                Val::Bool(false),
                Val::Bool(false)
            ]
        );
        // What has passed holds for ever; what is ahead, until it passes.
        assert_eq!(until, Some(2_000));
    }

    #[test]
    fn today_is_held_until_the_zones_next_day() {
        let zones = Arc::new(pw_time::Database::system());
        // 2026-09-05 23:59 in Santiago, whose next midnight is skipped.
        let now = 1_788_667_140_000;
        let ops: BTreeMap<String, HostFn> = operations(now, &zones).into_iter().collect();
        let (day, until) =
            noting(|| ops["pw:host/clock#today"](&[Val::String("America/Santiago".into())]));
        assert_eq!(day.unwrap(), [Val::S64(pw_time::days(2026, 9, 5))]);
        assert_eq!(
            until,
            Some(1_788_667_200_000),
            "01:00 on the 6th, its first instant"
        );
        let unknown = ops["pw:host/clock#today"](&[Val::String("Nowhere/Atlantis".into())]);
        assert!(unknown.is_err(), "a zone the database lacks is refused");
    }

    #[test]
    fn a_zones_conversions_are_the_databases() {
        let zones = Arc::new(pw_time::Database::system());
        let ops: BTreeMap<String, HostFn> = operations(0, &zones).into_iter().collect();
        let la = || Val::String("America/Los_Angeles".into());
        // 02:30 on 2026-03-08, skipped: 03:30 PDT.
        let at =
            ops["pw:host/clock#at"](&[Val::S64(pw_time::days(2026, 3, 8)), Val::S64(150), la()])
                .unwrap();
        assert_eq!(at, [Val::S64(1_772_965_800_000)]);
        let date = ops["pw:host/clock#date"](&[Val::S64(1_772_965_800_000), la()]).unwrap();
        assert_eq!(date, [Val::S64(pw_time::days(2026, 3, 8))]);
        let time = ops["pw:host/clock#time"](&[Val::S64(1_772_965_800_000), la()]).unwrap();
        assert_eq!(time, [Val::S64(3 * 60 + 30)]);
        let ((), until) = noting(|| {});
        assert_eq!(until, None, "a conversion compares no clock");
    }

    #[test]
    fn a_document_is_due_once_at_its_earliest_instant() {
        let timers = Timers::default();
        let a: Doc = ("s".into(), 1);
        let b: Doc = ("s".into(), 2);
        timers.hold(&a, 500);
        timers.hold(&a, 300);
        timers.hold(&a, 900);
        timers.hold(&b, 400);
        assert_eq!(timers.take_due(299), (vec![], Some(300)));
        let (due, next) = timers.take_due(450);
        assert_eq!(due, vec![a.clone(), b.clone()]);
        // The instant A's earliest replaced still wakes the timer, and
        // takes nothing.
        assert_eq!(next, Some(500));
        assert_eq!(timers.take_due(10_000), (vec![], None));
        assert_eq!(timers.at(&a), None);
    }
}

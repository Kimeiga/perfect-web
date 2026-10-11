//! The host's tz database, read as Python's `zoneinfo` reads it (the
//! vectors were made with it from the same files, 2026-10-10): offsets at
//! instants, past the last transition by each file's rule; local times read
//! as RFC 5545 reads them; and where each local day begins.

use pw_time::{DAY, Database, Zone, days};

fn zone(name: &str) -> std::sync::Arc<Zone> {
    Database::system()
        .zone(name)
        .expect("a zone the host's database has")
}

/// (zone, instant, offset east in seconds).
const OFFSETS: &[(&str, i64, i32)] = &[
    ("America/Los_Angeles", 1782864000, -25200),
    ("America/Los_Angeles", 1796083200, -28800),
    // 2100, past the last transition a slim file lists: the footer's rule.
    ("America/Los_Angeles", 4118083200, -25200),
    ("America/Los_Angeles", 4131302400, -28800),
    // 1800, before the first transition: local mean time.
    ("America/Los_Angeles", -5364662400, -28378),
    // Either side of 2026's two transitions.
    ("America/Los_Angeles", 1772963999, -28800),
    ("America/Los_Angeles", 1772964000, -25200),
    ("America/Los_Angeles", 1793523599, -25200),
    ("America/Los_Angeles", 1793523600, -28800),
    ("Europe/Paris", 1774745999, 3600),
    ("Europe/Paris", 1774746000, 7200),
    // Half an hour of daylight time, south of the equator.
    ("Australia/Lord_Howe", 1767225600, 39600),
    ("Australia/Lord_Howe", 1782864000, 37800),
    ("Australia/Lord_Howe", 3786912000, 39600),
    ("Australia/Lord_Howe", 3802550400, 37800),
    ("America/Santiago", 1767225600, -10800),
    ("America/Santiago", 1782864000, -14400),
    ("America/Santiago", 3786912000, -10800),
    ("Pacific/Chatham", 1767225600, 49500),
    ("Pacific/Chatham", 1782864000, 45900),
    ("Pacific/Chatham", 3802550400, 45900),
    ("America/St_Johns", 1767225600, -12600),
    ("America/St_Johns", 1782864000, -9000),
    ("Asia/Kolkata", 1782864000, 19800),
    ("Asia/Tokyo", 1782864000, 32400),
    ("UTC", 1782864000, 0),
    // Samoa, either side of the day it skipped (2011-12-30).
    ("Pacific/Apia", 1325149200, -36000),
    ("Pacific/Apia", 1325242800, 50400),
];

#[test]
fn an_instants_offset_is_the_databases() {
    for &(name, instant, offset) in OFFSETS {
        assert_eq!(zone(name).offset_at(instant), offset, "{name} at {instant}");
    }
}

/// (zone, year, month, day, hour, minute, instant).
const INSTANTS: &[(&str, i64, u32, u32, i64, i64, i64)] = &[
    // Skipped: read with the offset before the gap, 02:30 is 03:30.
    ("America/Los_Angeles", 2026, 3, 8, 2, 30, 1772965800),
    // Repeated: its first occurrence, in daylight time.
    ("America/Los_Angeles", 2026, 11, 1, 1, 30, 1793521800),
    ("America/Los_Angeles", 2026, 7, 4, 21, 0, 1783224000),
    ("America/Los_Angeles", 2026, 3, 8, 0, 0, 1772956800),
    ("Europe/Paris", 2026, 3, 29, 2, 30, 1774747800),
    ("Europe/Paris", 2026, 10, 25, 2, 30, 1792888200),
    ("Australia/Lord_Howe", 2026, 4, 5, 1, 45, 1775313900),
    ("Australia/Lord_Howe", 2026, 10, 4, 2, 15, 1791042300),
    // Midnight skipped: Chile's clocks go from 24:00 to 01:00.
    ("America/Santiago", 2026, 9, 6, 0, 0, 1788667200),
    ("America/Santiago", 2026, 4, 4, 23, 30, 1775356200),
    ("America/Santiago", 2026, 9, 5, 23, 59, 1788667140),
    ("Pacific/Chatham", 2026, 9, 27, 2, 50, 1790431500),
    ("America/St_Johns", 2026, 3, 8, 2, 30, 1772949600),
    ("Asia/Kolkata", 2026, 1, 1, 0, 0, 1767205800),
    ("Pacific/Apia", 2011, 12, 31, 0, 0, 1325239200),
    // By the footer's rule, in 2100.
    ("America/Los_Angeles", 2100, 3, 14, 2, 30, 4108703400),
    ("America/Los_Angeles", 2100, 11, 7, 1, 30, 4129259400),
];

#[test]
fn a_local_time_is_read_as_rfc_5545_reads_it() {
    for &(name, y, m, d, h, mi, instant) in INSTANTS {
        assert_eq!(
            zone(name).instant(days(y, m, d), h * 3600 + mi * 60),
            instant,
            "{name} {y}-{m}-{d} {h}:{mi}"
        );
    }
}

/// A zone, an instant, and its local year, month, day, hour, minute and
/// second there.
type Local = (&'static str, i64, i64, u32, u32, i64, i64, i64);

const LOCALS: &[Local] = &[
    ("America/Los_Angeles", 1772964000, 2026, 3, 8, 3, 0, 0),
    ("America/Los_Angeles", 1793521800, 2026, 11, 1, 1, 30, 0),
    ("America/Los_Angeles", 1793525400, 2026, 11, 1, 1, 30, 0),
    ("Pacific/Apia", 1325242799, 2011, 12, 31, 0, 59, 59),
    ("Pacific/Apia", 1325242800, 2011, 12, 31, 1, 0, 0),
    ("Asia/Tokyo", 1798729200, 2027, 1, 1, 0, 0, 0),
    ("America/Santiago", 1788667200, 2026, 9, 6, 1, 0, 0),
    ("Australia/Lord_Howe", 1791041400, 2026, 10, 4, 2, 30, 0),
];

#[test]
fn an_instants_local_date_and_time_are_the_databases() {
    for &(name, instant, y, m, d, h, mi, s) in LOCALS {
        assert_eq!(
            zone(name).local(instant),
            (days(y, m, d), h * 3600 + mi * 60 + s),
            "{name} at {instant}"
        );
    }
}

#[test]
fn a_day_begins_at_its_midnight_or_where_a_transition_moves_it() {
    let la = zone("America/Los_Angeles");
    // 2026-07-04 21:00 PDT: the 5th begins at 07:00Z.
    assert_eq!(la.next_day(1783224000), 1783224000 + 3 * 3600);
    // The day the clocks go back is 25 hours long.
    let from = la.instant(days(2026, 11, 1), 0);
    assert_eq!(la.next_day(from) - from, 25 * 3600);
    // Chile skips its midnight: the 6th begins at 01:00, its first instant.
    let santiago = zone("America/Santiago");
    assert_eq!(santiago.next_day(1788667140), 1788667200);
    assert_eq!(santiago.local(1788667200), (days(2026, 9, 6), 3600));
    // A day is never skipped where the zone keeps its offset.
    let tokyo = zone("Asia/Tokyo");
    assert_eq!(tokyo.next_day(1798729200 - 1), 1798729200);
    assert_eq!(tokyo.next_day(1798729200), 1798729200 + DAY);
}

#[test]
fn a_name_outside_the_database_is_refused_before_it_is_read() {
    let db = Database::system();
    for name in [
        "../../etc/passwd",
        "/etc/passwd",
        "America/../../etc/passwd",
    ] {
        assert!(
            matches!(db.zone(name), Err(pw_time::ZoneError::Name(_))),
            "{name}"
        );
    }
    assert!(matches!(
        db.zone("Nowhere/Atlantis"),
        Err(pw_time::ZoneError::Read(..))
    ));
}

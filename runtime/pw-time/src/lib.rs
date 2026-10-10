//! **Time zones, read from the host's own tz database** (ADR-0304): an
//! instant's local date and time in a zone, and the instant a zone's local
//! date and time name.
//!
//! A zone is its IANA name, and its rules are the host's tz database's, in
//! the TZif files RFC 8536 specifies (`/usr/share/zoneinfo` on Linux and
//! macOS, or `$TZDIR`): each transition a file lists, and past its last, the
//! POSIX TZ rule its footer gives. A local time a transition skips is read
//! with the offset before it, and one a transition repeats is its first
//! occurrence, as RFC 5545 (§3.3.5) reads them. Instants are seconds since
//! 1970-01-01T00:00Z, leap seconds not counted, as POSIX time is; dates are
//! days since 1970-01-01 in the proleptic Gregorian calendar.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

/// Seconds in a day.
pub const DAY: i64 = 86_400;

/// Why a zone could not be read.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ZoneError {
    /// Not a zone's name: an IANA name is components of letters, digits,
    /// `.`, `_`, `+` and `-`, joined by `/`, none of them `.` or `..`.
    Name(String),
    /// The database has no such file, or it could not be read.
    Read(String, String),
    /// The file is not TZif, or says something RFC 8536 does not allow.
    Format(String, String),
}

impl std::fmt::Display for ZoneError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ZoneError::Name(name) => write!(f, "`{name}` is not a time zone's name"),
            ZoneError::Read(name, why) => write!(f, "time zone `{name}` could not be read: {why}"),
            ZoneError::Format(name, why) => write!(f, "time zone `{name}` is not TZif: {why}"),
        }
    }
}

impl std::error::Error for ZoneError {}

/// **A zone**: when each of its transitions happens and the offset it
/// begins, the offset before the first, and the rule past the last.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Zone {
    name: String,
    /// Each transition's instant, ascending, and the offset east of UTC, in
    /// seconds, it begins.
    transitions: Vec<(i64, i32)>,
    /// The offset before the first transition: the file's time type 0
    /// (RFC 8536, §3.2).
    first: i32,
    /// The POSIX TZ rule for instants past the last transition (§3.3).
    rule: Option<Rule>,
}

impl Zone {
    /// The zone named `name`, from the tz database at `dir`.
    pub fn load(dir: &Path, name: &str) -> Result<Zone, ZoneError> {
        if !is_zone_name(name) {
            return Err(ZoneError::Name(name.to_string()));
        }
        let bytes = std::fs::read(dir.join(name))
            .map_err(|e| ZoneError::Read(name.to_string(), e.to_string()))?;
        Zone::parse(name, &bytes)
    }

    /// The zone a TZif file's `bytes` describe, called `name`.
    pub fn parse(name: &str, bytes: &[u8]) -> Result<Zone, ZoneError> {
        let wrong = |why: &str| ZoneError::Format(name.to_string(), why.to_string());
        let first = header(bytes).ok_or_else(|| wrong("no TZif header"))?;
        // A version 2 file or later repeats its data with 64-bit times, and
        // ends with the rule for what follows: the 32-bit block is skipped.
        let (counts, data, wide) = if first.version >= b'2' {
            let at = 44 + first.block(4);
            let second = bytes
                .get(at..)
                .and_then(header)
                .ok_or_else(|| wrong("no second header"))?;
            (second, at + 44, true)
        } else {
            (first, 44, false)
        };
        let size = if wide { 8 } else { 4 };
        let take = |from: usize, len: usize| {
            bytes
                .get(from..from + len)
                .ok_or_else(|| wrong("shorter than its counts"))
        };
        let times = take(data, counts.timecnt * size)?;
        let indices = take(data + counts.timecnt * size, counts.timecnt)?;
        let types = take(data + counts.timecnt * (size + 1), counts.typecnt * 6)?;
        if counts.typecnt == 0 {
            return Err(wrong("no local time type"));
        }
        let offsets: Vec<i32> = types
            .chunks_exact(6)
            .map(|t| i32::from_be_bytes([t[0], t[1], t[2], t[3]]))
            .collect();
        let mut transitions = Vec::with_capacity(counts.timecnt);
        for (i, &index) in indices.iter().enumerate() {
            let raw = &times[i * size..(i + 1) * size];
            let at = if wide {
                i64::from_be_bytes(raw.try_into().expect("eight bytes"))
            } else {
                i64::from(i32::from_be_bytes(raw.try_into().expect("four bytes")))
            };
            let offset = *offsets
                .get(usize::from(index))
                .ok_or_else(|| wrong("a transition to no type"))?;
            if transitions.last().is_some_and(|&(last, _)| last >= at) {
                return Err(wrong("transitions out of order"));
            }
            transitions.push((at, offset));
        }
        let rule = if wide {
            let end = data + counts.block(8);
            let footer = bytes.get(end..).ok_or_else(|| wrong("no footer"))?;
            let text = footer
                .strip_prefix(b"\n")
                .and_then(|f| f.split(|&b| b == b'\n').next())
                .ok_or_else(|| wrong("a footer not between newlines"))?;
            let text = std::str::from_utf8(text).map_err(|_| wrong("a footer not ASCII"))?;
            if text.is_empty() {
                None
            } else {
                Some(Rule::parse(text).ok_or_else(|| wrong("a footer that is no TZ rule"))?)
            }
        } else {
            None
        };
        Ok(Zone {
            name: name.to_string(),
            transitions,
            first: offsets[0],
            rule,
        })
    }

    /// The zone's name.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// **The offset east of UTC, in seconds, in effect at `instant`.**
    pub fn offset_at(&self, instant: i64) -> i32 {
        let passed = self.transitions.partition_point(|&(at, _)| at <= instant);
        match (&self.rule, passed) {
            (Some(rule), n) if n == self.transitions.len() => rule.offset_at(instant),
            (_, 0) => self.first,
            (_, n) => self.transitions[n - 1].1,
        }
    }

    /// **The local date and time at `instant`**: the day, and the seconds
    /// since its midnight.
    pub fn local(&self, instant: i64) -> (i64, i64) {
        let local = instant + i64::from(self.offset_at(instant));
        (local.div_euclid(DAY), local.rem_euclid(DAY))
    }

    /// **The instant that `day`'s local time `seconds` (since its midnight)
    /// names** (RFC 5545, §3.3.5): where a transition repeats it, its first
    /// occurrence; where one skips it, read with the offset before the gap,
    /// so 02:30 on a day the clocks go from 02:00 to 03:00 is 03:30.
    pub fn instant(&self, day: i64, seconds: i64) -> i64 {
        let local = day * DAY + seconds;
        // The offsets in effect a day either side: no zone has had two
        // transitions so close that a third lies between.
        let before = self.offset_at(local - DAY);
        let after = self.offset_at(local + DAY);
        let read = |offset: i32| {
            let at = local - i64::from(offset);
            (self.offset_at(at) == offset).then_some(at)
        };
        match (read(before), read(after)) {
            (Some(a), Some(b)) => a.min(b),
            (Some(a), None) | (None, Some(a)) => a,
            (None, None) => local - i64::from(before),
        }
    }

    /// **When the local day after `instant`'s begins**: its midnight, or,
    /// where a transition skips midnight, the instant the day begins.
    pub fn next_day(&self, instant: i64) -> i64 {
        let (day, _) = self.local(instant);
        self.instant(day + 1, 0)
    }
}

/// Whether `name` is shaped as an IANA zone's name: never a path out of the
/// database, nor an absolute one.
pub fn is_zone_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 255
        && name.split('/').all(|part| {
            !part.is_empty()
                && part != "."
                && part != ".."
                && part
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'_' | b'+' | b'-'))
        })
}

/// **The host's tz database**, its zones read once each.
pub struct Database {
    dir: PathBuf,
    zones: Mutex<BTreeMap<String, Arc<Zone>>>,
}

impl Database {
    /// The database at `dir`.
    pub fn at(dir: impl Into<PathBuf>) -> Database {
        Database {
            dir: dir.into(),
            zones: Mutex::new(BTreeMap::new()),
        }
    }

    /// The host's own: `$TZDIR` where it is set (POSIX's convention), else
    /// `/usr/share/zoneinfo`.
    pub fn system() -> Database {
        Database::at(
            std::env::var_os("TZDIR")
                .map(PathBuf::from)
                .unwrap_or_else(|| PathBuf::from("/usr/share/zoneinfo")),
        )
    }

    /// The zone named `name`.
    pub fn zone(&self, name: &str) -> Result<Arc<Zone>, ZoneError> {
        if let Some(zone) = self.zones.lock().expect("zones").get(name) {
            return Ok(zone.clone());
        }
        let zone = Arc::new(Zone::load(&self.dir, name)?);
        self.zones
            .lock()
            .expect("zones")
            .insert(name.to_string(), zone.clone());
        Ok(zone)
    }

    /// The database's version, where it says: `+VERSION` (macOS), or the
    /// `# version` line of `tzdata.zi` (the tz project's own, and Debian's).
    pub fn version(&self) -> Option<String> {
        if let Ok(text) = std::fs::read_to_string(self.dir.join("+VERSION")) {
            return Some(text.trim().to_string()).filter(|v| !v.is_empty());
        }
        let text = std::fs::read_to_string(self.dir.join("tzdata.zi")).ok()?;
        text.lines()
            .find_map(|l| l.strip_prefix("# version "))
            .map(|v| v.trim().to_string())
    }
}

/// A TZif header's version and counts (RFC 8536, §3.1).
#[derive(Clone, Copy)]
struct Header {
    version: u8,
    isutcnt: usize,
    isstdcnt: usize,
    leapcnt: usize,
    timecnt: usize,
    typecnt: usize,
    charcnt: usize,
}

impl Header {
    /// The length of the data block after this header, its times
    /// `size` bytes each.
    fn block(&self, size: usize) -> usize {
        self.timecnt * size
            + self.timecnt
            + self.typecnt * 6
            + self.charcnt
            + self.leapcnt * (size + 4)
            + self.isstdcnt
            + self.isutcnt
    }
}

fn header(bytes: &[u8]) -> Option<Header> {
    let head = bytes.get(..44)?;
    if &head[..4] != b"TZif" {
        return None;
    }
    let count = |i: usize| {
        let at = 20 + i * 4;
        usize::try_from(u32::from_be_bytes(head[at..at + 4].try_into().ok()?)).ok()
    };
    Some(Header {
        version: head[4],
        isutcnt: count(0)?,
        isstdcnt: count(1)?,
        leapcnt: count(2)?,
        timecnt: count(3)?,
        typecnt: count(4)?,
        charcnt: count(5)?,
    })
}

/// **A POSIX TZ rule** (POSIX.1-2017, §8.3, as RFC 8536 §3.3.1 extends
/// it): standard time's offset, and where there is daylight time, its offset
/// and the days and times it begins and ends.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Rule {
    /// Standard time's offset east of UTC, in seconds.
    standard: i32,
    daylight: Option<Daylight>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct Daylight {
    /// Daylight time's offset east of UTC, in seconds.
    offset: i32,
    /// The day it begins, and the local standard time it begins at.
    start: (Day, i32),
    /// The day it ends, and the local daylight time it ends at.
    end: (Day, i32),
}

/// A day of a year, as a rule names one.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Day {
    /// `Jn`: the n-th day, 1 to 365, February 29th never counted.
    Julian(u32),
    /// `n`: the n-th day, 0 to 365, February 29th counted.
    Ordinal(u32),
    /// `Mm.w.d`: day `d` (0 is Sunday) of week `w` (5 is the last) of month
    /// `m`.
    Month(u32, u32, u32),
}

impl Rule {
    /// The rule `text` says, or nothing where it is not one.
    pub fn parse(text: &str) -> Option<Rule> {
        let mut rest = text;
        name(&mut rest)?;
        let standard = -offset(&mut rest, 24)?;
        if rest.is_empty() {
            return Some(Rule {
                standard,
                daylight: None,
            });
        }
        name(&mut rest)?;
        // Daylight time's offset, an hour ahead of standard time's where
        // the rule says none.
        let offset_east = if rest.starts_with(',') {
            standard + 3600
        } else {
            -offset(&mut rest, 24)?
        };
        // POSIX leaves the days out to the implementation where a rule
        // names none; the tz database's files always give them.
        let transition = |rest: &mut &str| {
            *rest = rest.strip_prefix(',')?;
            let day = day(rest)?;
            let time = match rest.strip_prefix('/') {
                Some(after) => {
                    *rest = after;
                    offset(rest, 167)?
                }
                None => 2 * 3600,
            };
            Some((day, time))
        };
        let start = transition(&mut rest)?;
        let end = transition(&mut rest)?;
        rest.is_empty().then_some(Rule {
            standard,
            daylight: Some(Daylight {
                offset: offset_east,
                start,
                end,
            }),
        })
    }

    /// The offset east of UTC, in seconds, in effect at `instant`.
    pub fn offset_at(&self, instant: i64) -> i32 {
        let Some(daylight) = &self.daylight else {
            return self.standard;
        };
        let year = civil((instant + i64::from(self.standard)).div_euclid(DAY)).0;
        let begins = |(day, time): (Day, i32), offset: i32| {
            day.of(year) * DAY + i64::from(time) - i64::from(offset)
        };
        let start = begins(daylight.start, self.standard);
        let end = begins(daylight.end, daylight.offset);
        let daylight_time = if start < end {
            start <= instant && instant < end
        } else {
            // Daylight time across the new year, as south of the equator.
            instant < end || start <= instant
        };
        if daylight_time {
            daylight.offset
        } else {
            self.standard
        }
    }
}

impl Day {
    /// The day of `year` this names, as days since 1970-01-01.
    fn of(self, year: i64) -> i64 {
        let january = days(year, 1, 1);
        match self {
            Day::Julian(n) => {
                let n = i64::from(n);
                january + n - 1 + i64::from(leap(year) && n >= 60)
            }
            Day::Ordinal(n) => january + i64::from(n),
            Day::Month(month, week, weekday) => {
                let first = days(year, month, 1);
                let next = if month == 12 {
                    days(year + 1, 1, 1)
                } else {
                    days(year, month + 1, 1)
                };
                let into = (i64::from(weekday) - (first + 4)).rem_euclid(7);
                let mut day = first + into + 7 * (i64::from(week) - 1);
                while day >= next {
                    day -= 7;
                }
                day
            }
        }
    }
}

/// A rule's zone abbreviation, alphabetic or `<…>`-quoted, taken from
/// `rest`.
fn name(rest: &mut &str) -> Option<()> {
    if let Some(quoted) = rest.strip_prefix('<') {
        let close = quoted.find('>')?;
        let inner = &quoted[..close];
        (inner.len() >= 3
            && inner
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'+' || b == b'-'))
        .then_some(())?;
        *rest = &quoted[close + 1..];
    } else {
        let len = rest.bytes().take_while(u8::is_ascii_alphabetic).count();
        (len >= 3).then_some(())?;
        *rest = &rest[len..];
    }
    Some(())
}

/// `[+|-]hh[:mm[:ss]]` in seconds, its hours at most `hours`, taken from
/// `rest`.
fn offset(rest: &mut &str, hours: i32) -> Option<i32> {
    let mut sign = 1;
    if let Some(after) = rest.strip_prefix('-') {
        sign = -1;
        *rest = after;
    } else if let Some(after) = rest.strip_prefix('+') {
        *rest = after;
    }
    let mut seconds = 0;
    for (i, (limit, unit)) in [(hours, 3600), (59, 60), (59, 1)].into_iter().enumerate() {
        if i > 0 {
            match rest.strip_prefix(':') {
                Some(after) => *rest = after,
                None => break,
            }
        }
        let digits = rest.bytes().take_while(u8::is_ascii_digit).count();
        if digits == 0 || digits > 3 {
            return None;
        }
        let value: i32 = rest[..digits].parse().ok()?;
        if value > limit {
            return None;
        }
        *rest = &rest[digits..];
        seconds += value * unit;
    }
    Some(sign * seconds)
}

/// A rule's day, `Jn`, `n` or `Mm.w.d`, taken from `rest`.
fn day(rest: &mut &str) -> Option<Day> {
    let number = |rest: &mut &str| -> Option<u32> {
        let digits = rest.bytes().take_while(u8::is_ascii_digit).count();
        if digits == 0 || digits > 3 {
            return None;
        }
        let value = rest[..digits].parse().ok()?;
        *rest = &rest[digits..];
        Some(value)
    };
    if let Some(after) = rest.strip_prefix('J') {
        *rest = after;
        let n = number(rest)?;
        return (1..=365).contains(&n).then_some(Day::Julian(n));
    }
    if let Some(after) = rest.strip_prefix('M') {
        *rest = after;
        let month = number(rest)?;
        *rest = rest.strip_prefix('.')?;
        let week = number(rest)?;
        *rest = rest.strip_prefix('.')?;
        let weekday = number(rest)?;
        return ((1..=12).contains(&month) && (1..=5).contains(&week) && weekday <= 6)
            .then_some(Day::Month(month, week, weekday));
    }
    let n = number(rest)?;
    (n <= 365).then_some(Day::Ordinal(n))
}

/// Whether `year` has a February 29th.
pub fn leap(year: i64) -> bool {
    year % 4 == 0 && (year % 100 != 0 || year % 400 == 0)
}

/// **The days since 1970-01-01 of `year`-`month`-`day`**, in the proleptic
/// Gregorian calendar (Howard Hinnant's `days_from_civil`).
pub fn days(year: i64, month: u32, day: u32) -> i64 {
    let y = if month <= 2 { year - 1 } else { year };
    let era = (if y >= 0 { y } else { y - 399 }) / 400;
    let yoe = y - era * 400;
    let mp = i64::from((month + 9) % 12);
    let doy = (153 * mp + 2) / 5 + i64::from(day) - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

/// **The year, month and day of `days` since 1970-01-01** (Howard Hinnant's
/// `civil_from_days`).
pub fn civil(days: i64) -> (i64, u32, u32) {
    let z = days + 719_468;
    let era = (if z >= 0 { z } else { z - 146_096 }) / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = u32::try_from(doy - (153 * mp + 2) / 5 + 1).expect("a day of a month");
    let month = u32::try_from(if mp < 10 { mp + 3 } else { mp - 9 }).expect("a month");
    let year = yoe + era * 400 + i64::from(month <= 2);
    (year, month, day)
}

/// The day of the week of `days` since 1970-01-01, 0 for Monday to 6 for
/// Sunday (ISO 8601's order): 1970-01-01 was a Thursday.
pub fn weekday(days: i64) -> u32 {
    u32::try_from((days + 3).rem_euclid(7)).expect("a weekday")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_civil_calendar_round_trips() {
        for (y, m, d, n) in [
            (1970, 1, 1, 0),
            (2000, 2, 29, 11_016),
            (2026, 10, 10, 20_736),
            (1969, 12, 31, -1),
            (1600, 3, 1, -135_080),
        ] {
            assert_eq!(days(y, m, d), n, "{y}-{m}-{d}");
            assert_eq!(civil(n), (y, m, d), "{n}");
        }
        for n in -800_000..800_000 {
            let (y, m, d) = civil(n);
            assert_eq!(days(y, m, d), n);
        }
    }

    #[test]
    fn weekdays_count_from_monday() {
        // 1970-01-01 a Thursday; 2026-10-10 a Saturday.
        assert_eq!(weekday(0), 3);
        assert_eq!(weekday(days(2026, 10, 10)), 5);
        assert_eq!(weekday(-1), 2);
    }

    #[test]
    fn a_rule_is_read_as_posix_and_rfc_8536_write_it() {
        let la = Rule::parse("PST8PDT,M3.2.0,M11.1.0").expect("a rule");
        assert_eq!(la.standard, -8 * 3600);
        let daylight = la.daylight.as_ref().expect("daylight time");
        assert_eq!(daylight.offset, -7 * 3600);
        assert_eq!(daylight.start, (Day::Month(3, 2, 0), 7200));
        assert_eq!(daylight.end, (Day::Month(11, 1, 0), 7200));
        // Quoted names, minutes, and a time past 24 hours (§3.3.1).
        let lord_howe = Rule::parse("<+1030>-10:30<+11>-11,M10.1.0,M4.1.0").expect("a rule");
        assert_eq!(lord_howe.standard, 10 * 3600 + 1800);
        assert_eq!(
            lord_howe.daylight.as_ref().expect("daylight").offset,
            11 * 3600
        );
        let rule = Rule::parse("<-03>3<-02>,M3.5.0/-2,M10.5.0/-1").expect("negative times");
        assert_eq!(rule.daylight.as_ref().expect("daylight").start.1, -7200);
        assert!(
            Rule::parse("EST5EDT,0/0,J365/25").is_some(),
            "daylight all year"
        );
        assert_eq!(
            Rule::parse("UTC0"),
            Some(Rule {
                standard: 0,
                daylight: None
            })
        );
        for wrong in [
            "",
            "PS8",
            "PST",
            "PST8PDT,M3.2",
            "PST8PDT,M13.2.0,M11.1.0",
            "PST25",
        ] {
            assert_eq!(Rule::parse(wrong), None, "{wrong}");
        }
    }

    #[test]
    fn names_never_leave_the_database() {
        for name in [
            "America/Los_Angeles",
            "UTC",
            "Etc/GMT+5",
            "America/Port-au-Prince",
        ] {
            assert!(is_zone_name(name), "{name}");
        }
        for name in [
            "",
            "/etc/passwd",
            "../x",
            "America/../../x",
            "a//b",
            "a b",
            "a\\b",
        ] {
            assert!(!is_zone_name(name), "{name}");
        }
    }
}

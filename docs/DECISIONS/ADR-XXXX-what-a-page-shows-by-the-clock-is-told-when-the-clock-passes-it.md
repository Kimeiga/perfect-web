# ADR-XXXX: what a page shows by the clock is told when the clock passes it

Status: accepted under the owner's delegation of 2026-10-02. Date:
2026-10-10. Milestone: E14. The first of the store hours' steps (NEXT, the
DoorDash customer app's item 4): the platform's clock, read, kept and told;
then the store's hours, shown everywhere and holding orders while closed;
then orders scheduled. Builds on ADR-0180 (`Instant`), ADR-0127 (a query's
policies, its kept values), ADR-0219 (a live document is told).

## Context

- **The clock answered 0.** `clock.now()` (`clock.read`), `now_wall()` and
  `today()` (`clock.wall`) were stubs, and no host implemented a Pleris
  clock. ADR-0180 gave `Instant` and claimed no time zone.
- **Nothing was told when time passed.** `freshness` is checked when a read
  arrives (`pw-resource`), and no host ran anything at a time
  (KNOWN_LIMITATIONS: a change reaches an open page "within the query's
  freshness, or not at all"). A store that closes at 21:00 would say "Open"
  on every open page until something else changed, and take the orders the
  host then refused: the failure DoorDash's sellers report when a closure
  does not reach the app.
- **A wall read could not be kept for others** (PW0401): an entry made at
  one moment and served to every reader holds a value true when it was
  made. Whether a store is open is the same for every reader, and the value
  a delivery app shares most.
- **What others do** (read 2026-10-10):
  - Materialize's temporal filters: `mz_now()` is allowed only in a
    comparison with a value that does not read it, and rows enter and leave
    a maintained result as the clock passes their bounds;
  - Convex fixes `Date.now()` for a query's run and runs a subscription
    again only when its data changes, so a query that reads the time goes
    stale; its advice is a mutation scheduled for the moment state should
    change, or a time rounded and passed in;
  - Phoenix LiveView renders again from a process's own timer;
  - time zones: IANA's tz database, as TZif files (RFC 8536), whose footer
    is a POSIX TZ rule for the years after its last transition; a local
    time a transition skips, or repeats, read as RFC 5545 §3.3.5 reads it
    (Temporal's `compatible`).

## Decision

1. **A value kept or shown reads the wall clock only by comparing it.**
   `clock.passed(at: Instant) -> Bool`, whether an instant has passed, and
   `clock.today_in(zone: TimeZone) -> LocalDate`, a zone's date, are effect
   `clock.compare`. No value of now itself is given, so nothing that is
   shown changes without a comparison to say when. `now_wall()` stays, a
   moment for a command to record, and PW0401 still refuses it in what is
   kept for others.
2. **A value holds until the earliest instant it compared.** The host notes,
   for each read, each instant ahead that `passed` was asked of, and the
   zone's next day for `today_in` (where a transition skips a midnight, the
   day's first instant). A query's kept entry, shared or not, is held until
   then and read again past it; a reader its flight answers is held to it
   too.
3. **And told then.** Each live document that shows such a value is read
   again at its instant, and sent what changed, in its session's turn, as a
   change of the session's would send it; its next instant is set as it is
   read. A document no page is asking for is read when one asks, as any
   change reaches it (ADR-0297).
4. **One clock for the host**: the system's at its start, moved since as
   its query clock is, by time and by a test: `/bench/clock` moves it
   forward, to `at` or by `advance` milliseconds, never back. Freshness,
   instants and timers read it alike.
5. **Where it may not be read**: a page built once (`placement build`,
   PW5005 and PW0401) and a public materialization (PW0401), which are made
   again by a build or by an event, and no event says an instant passed.
6. **Time zones and the calendar, the host's database's** (`pw-time`, no
   dependency): `TimeZone`, an IANA name the database has; `LocalDate`, days
   since 1970-01-01; `LocalTime`, minutes since midnight, 0 to 1439 (ADR-0179's
   invariant); `Weekday`, Monday first (ISO 8601). `clock.at(date, time,
   zone)` (RFC 5545), `clock.date_of` and `clock.time_of`, effect
   `clock.zone`; `zone`, a checked `time`, `weekday` and `next_day` in the
   platform's own code. Both effects are capabilities a host grants; the
   development origin, which answers them, grants both.

## Found

1. **The clock was never read.** Every `clock` function was a stub
   answering 0, `""` or nothing, since E2C, and no corpus program, nor the
   store, could tell.
2. **An effect that needs no authority is never linked.** A host function
   whose effect declares `capability none` is left out of the linker (only a
   granted import, or a read, is installed), so a component that imported
   one failed to instantiate. The tz database is a capability now, which a
   host that has one grants; whether a `capability none` host function
   should be linked is left as it was.

## Alternatives

- **The clock's value, kept with a freshness**: a page shows a store open
  for as long as the freshness, after it closed. A freshness is a bound on
  staleness, and "until it closes" is no bound a program can write.
- **Ticking**: reading every page again each minute. A page told a minute
  late, and every page read again for nothing the other 1,439 minutes.
- **A scheduled command at each opening and closing** (Convex's advice):
  the same fact written twice, the hours and a flag a scheduler keeps, and
  a host that missed its moment shows the flag, not the hours.
- **A time zone crate** (`jiff`, `chrono-tz`): a new dependency for what the
  host already has, its own tz database, and its rules updated with the
  host's. The parsing is RFC 8536's and a POSIX rule, a few hundred lines,
  held to Python's `zoneinfo` from the same files.

## Acceptance

`just e14-clock` records each (docs/evidence/E14/clock.txt), with the tz
database's version:

- **`pw-time`** (9 tests): offsets at instants, before the first transition
  and past the last by each file's rule (2100), in Los Angeles, Paris, Lord
  Howe's half hour, Santiago, Chatham's +12:45, St John's, Kolkata, Tokyo,
  Samoa's skipped day; local times as RFC 5545 reads them; where each local
  day begins, Santiago's skipped midnight among them; a name outside the
  database refused before it is read; the civil calendar, and a POSIX rule.
- **The checker** (`effects`, `tests/clock_compared.rs`): a page built once
  may not compare the clock, and served it may; a shared query may; a
  public materialization may not.
- **The host** (10 tests): `passed` answers for its instant and notes what
  is ahead; `today_in` held until the zone's next day; the zones'
  conversions; a document due once, at its earliest instant; a page told
  when the instant it compared passes, and not a second before, and held
  again until the day ends; a kept value past its instant read again, a
  second reader's page held to it; a page that compares no clock held by
  nothing; the host's clock the system's, moved forward as a test says,
  never back.
- **`scripts/clock_mutations.py`**: 25 mutants.

## Not claimed

- **The store's hours.** The next step: this ADR's acceptance is the host's,
  and the browser's comes with them.
- **A countdown** ("closes in 12 minutes"): no value of now itself.
- **A handler's clock**, the browser's.
- **A build-time page or a public materialization told when time passes.**
- **More than one host's clock**, and a clock moved back.
- **A tz database changing under a kept value**, until the value is read
  again.
- **Leap seconds**, as POSIX time does not count them.

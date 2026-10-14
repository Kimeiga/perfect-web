# ADR-XXXX: a command runs when the clock passes its time

Status: draft (the integrator's; ADR-0332's not-claimed sweep and NEXT 4's
third step, orders scheduled). Date: 2026-10-14. Milestone: E14. Builds on
ADR-0304 (the host's clock, which nothing a program reads but by
comparison), ADR-0325 (an interaction's answer committed with its writes),
ADR-0332 (`invoked_by Payments`, a command the platform runs on a reader's
behalf) and ADR-0280 (`navigate`, a statement that names a declaration and
checks its arguments as a call's).

## Context

- **Two things wait for a time, and nothing runs at one.**
  - ADR-0332 does not claim a sweep: "an order no one pays or takes back
    awaits for good", its cart held and its code used.
  - NEXT 4's third step is orders scheduled: an order placed now, sent to
    its store at the time its reader chose.
  ADR-0304 rejected a command scheduled at each opening and closing for a
  store's hours, which a comparison with the clock decides better. An
  order's deadline is not a value shown: it is something done, once.
- **What the host has** (read 2026-10-14):
  - its one clock (ADR-0304), which `/bench/clock` moves in a test, and
    timers that read a live document again when an instant it compared
    passes, held in memory, by document;
  - an outbox written with a command's writes, which is no queue: a host
    just opened deletes it, and the committing connection delivers and
    deletes its rows;
  - `invoked_by Payments` (ADR-0332): a command no reader presses, run by
    the host on behalf of the reader who began a payment, keyed so it
    commits once (ADR-0325);
  - no program reads the time: `now()` and `now_wall()` answer 0 and no host
    grants `clock.wall`; an instant a program holds comes from its data or
    from `clock.at(date, time, zone)`.
  - a command that writes no rows commits nothing and records no answer.
- **Practice** (each read 2026-10-14):
  - **Convex** ("Scheduled Functions"): `runAfter` and `runAt` from a
    mutation are atomic with it: if the mutation fails, "no function will be
    scheduled"; a scheduled mutation is "executed exactly once", internal
    errors retried, a developer's error failed; `cancel` stops one not yet
    started; each is a row of `_scheduled_functions` (Pending, Success,
    Failed, Canceled); "resilient against unexpected downtime or system
    restarts".
  - **Oban** (v2.24.1, `Oban`): a job inserted "along with other database
    changes, ensuring that everything is committed or rolled back
    atomically"; scheduled "at any time in the future, down to the second";
    unique by its arguments; `cancel_job` leaves a completed job completed;
    retries "scheduled in the future with backoff".
  - **River** ("Transactional enqueueing"): a job enqueued in the
    transaction of the change it follows finds its data committed with it.
  - **Cloudflare Durable Objects** (alarms): one alarm per object, which
    `setAlarm` replaces; "guaranteed at-least-once execution", retried from
    a 2 s backoff, at most six times.

## Decision

1. **`invoked_by Clock`**: a command the platform runs when the clock passes
   the time a command scheduled it for, on behalf of the user (or the
   session) whose command scheduled it. As `invoked_by Payments`: no reader
   presses it (PW6404), it holds no reader to a predicate (PW6401), and it
   is `idempotent_by` its schedule (PW6405). Unlike it: any parameters, as
   its scheduler gives them, and as many such commands as a program needs
   (PW6402 and PW6403 are Payments' alone).
2. **`schedule cmd(args) at when`** and **`schedule cmd(args) after
   duration`**, statements of a command's body:
   - `cmd` is a command `invoked_by Clock`, written as a call (PW6501); its
     arguments are checked as a call's;
   - written in a command, never a page, a view, a handler or a query: what
     a page shows schedules nothing (PW6502);
   - `when` is the clock's `Instant`; `duration` its `Duration`, the time
     from the scheduling command's commit, read from the host's clock then:
     no program reads the time to compute one (PW6503, each type by its
     declaration, so a program's own `Instant` is another);
   - its command performs `clock.schedule` (placement origin), as `emits`
     performs `outbox.write`.
3. **One pending run for each command and its arguments.** Scheduled again,
   it moves; `unschedule cmd(args)` cancels it, and is nothing where none is
   pending. A command retried schedules once, and nothing keeps an id
   (Oban's jobs unique by their arguments; a Durable Object's one alarm).
4. **Written with the scheduling command's writes**, in its transaction: a
   command that answers a declared error, or fails, schedules nothing.
5. **Run once.** The host runs what is due one at a time, in the order of
   its instants, each in a transaction of its own, keyed by its schedule
   (ADR-0325), on behalf of its scheduler (`identity::on_behalf`). The
   schedule's row is removed in that transaction, so a run commits once
   even where its command writes nothing: a host stopped before the commit
   runs it again, after it never. A declared error is the program's
   answer, and done. A trap or a refusal runs again, after 2 s, doubling to
   a minute, five times at most; then the schedule is kept, failed, and said
   in the host's log and its `/metrics`.
6. **Durable**: the program's database holds the schedules (the
   materializer's for the in-memory layers; a migration for each PostgreSQL
   layer). A host started arms its timer at the earliest pending; two hosts
   on one database commit a run once, as two commit an answer once.
7. **The first use, ADR-0332's sweep**: an order awaiting its payment past
   thirty minutes is taken back, its payment canceled at the provider, as
   its reader's cancel takes it back; a settlement or a cancel unschedules
   it. Orders scheduled (NEXT 4's third step) use the same statement, in a
   track of their own.

## Not claimed

- A recurring schedule (cron): neither use needs one.
- A run's precision: it starts once its instant has passed, within a second
  of a host that is running; a host stopped runs what is past due when it
  starts.
- A program reading the time (`now`): `after` is resolved by the host.

## Alternatives

- **A schedule id a program keeps** (Convex's `Id<"_scheduled_functions">`):
  a field a program must store, and a retried command schedules twice.
- **The outbox as the queue**: it is delivered and deleted at its commit,
  and a host just opened clears it.
- **`now()` for a program**: every value computed from it is the hazard
  ADR-0304 holds back; a duration the host resolves needs none.
- **A clause, `schedules cmd(args) after d`**: a clause sees only the
  command's parameters, and an order's reference is made in its body.

## Acceptance

To come with the track: the checker's tests and the corpus; the host's, on
each layer (a run at its instant and not before, once across a restart, a
schedule moved and canceled, nothing scheduled by a command that fails, a
run on its scheduler's behalf, a trapping run retried then failed, a press
refused); the sweep end to end; mutation controls.

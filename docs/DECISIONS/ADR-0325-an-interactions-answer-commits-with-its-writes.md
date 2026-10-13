# ADR-0325: an interaction's answer commits with its writes

Status: accepted under the owner's delegation of 2026-10-02 (the
integrator's). Date: 2026-10-12. Milestone: E14. Takes NEXT's item 5 and the
"Not claimed" of ADR-0246 and ADR-0298, idempotency committed with the
writes, before a payment depends on it (item 6). Builds on ADR-0121 (an
idempotent command runs once per interaction), ADR-0207 (PW0348) and
ADR-0218 (the data layer's staged writes).

## Context

- **The checker promised what the host did not keep.** PW0348 refuses an
  idempotent command whose source commits in no transaction, and says why:
  "A command sent again answers what it answered the first time, by a record
  of the interaction committed with its writes. Without a transaction, a
  write could commit and its record not, and the command would run twice."
- **The host kept the record in its memory** (ADR-0121): `pw-resource`'s
  reservations, the last 64 interactions of each session. The writes
  committed in the data layer's transaction, and the record did not.
  - On PostgreSQL (ADR-0246, ADR-0298) the writes outlive the process, and
    the record did not. A host started after another, or a second host,
    ran a retried interaction again: two lines where the press added one,
    two posts, an order placed twice.
  - In memory, a session's 65th interaction forgot its first, whose writes
    were still the session's. ADR-0121 stated this as its bound's cost.
- **Practice.** Stripe's API saves the first request's status and body under
  its idempotency key once its execution begins, answers a retry with them,
  compares a retry's parameters with the first's and refuses another, and
  may remove a key once it is at least 24 hours old, after which the key is
  a new request. The IETF's Idempotency-Key draft (draft 07, 2025-10-15)
  asks a resource to define and publish its expiration policy. Both read
  2026-10-12.

## Decision

1. **The record commits in the command's transaction**, beside its writes
   and its outbox: the interaction's key (the session, the command and the
   interaction), the arguments it was first sent with, and its answer as
   ADR-0121 keeps it, at the host's instant. A command whose transaction
   rolls back records nothing.
   - On PostgreSQL, a table of each layer's schema, `interactions`, keyed by
     the key: the store's migration 0005 and the feed's 0008. A layer whose
     writes outlive the host keeps its records (`DataLayer::
     keeps_interactions`).
   - In memory, the host's materializer's transaction, which commits the
     layer's rows and its events (ADR-0019): `pw-materialize`'s
     `interactions` table.
2. **A command recalls a committed record before it runs.** One found
   answers it, and nothing runs; one with other arguments refuses it, as in
   memory (ADR-0121).
3. **Two hosts at once.** The second's commit meets the first's record by
   its key and rolls back; it answers what the first recorded.
4. **A record answers a day**, on the host's clock: a recall reads none
   older, and each commit forgets those older in its own transaction. A
   retry later than that is a new request, as at Stripe. The host's
   reservations keep a session's last 64 as well, so a recent one is
   answered from memory whatever its age: a record answers at least a day.
5. **The reservations stay** (`try_command`): a duplicate on one host waits
   for the first and shares its outcome, rather than running into its key.

## Alternatives

- **Keep the record in a store of its own** (a cache, a second database).
  Rejected: a write and its record in two stores commit apart, which is
  what PW0348 refuses.
- **Record before running, and run after.** A record of an interaction
  that has not run says nothing a retry can be answered with. Stripe saves
  its result once execution begins; here it is saved with the writes it
  answers for.
- **Keep records for good.** Rejected: a key is random per press, so a
  retry a day late is no retry a client makes, and the table would grow
  with every press.

## Acceptance

- `runtime/pw-materialize`: its tests, the table created with the schema.
- The host, in memory: a session's oldest interaction, past its 64,
  answered by its record, and refused with other arguments; within a day
  answered, a day and a minute later run again.
- On PostgreSQL (`tests/store_pg.rs`, `tests/feed_pg.rs`):
  - a host started after another answers the first's interaction as it
    did, the cart holding one line, and refuses it with other arguments;
  - a second host's commit that meets the first's record answers what the
    first recorded and writes nothing;
  - a host a day and a minute later runs it again, and its commit forgets
    the old record;
  - the feed's post, the same across hosts.
- `scripts/interactions_mutations.py`, 10 mutants, each killed
  (`just e14-interactions`, on a database).

## Not claimed

- **A declared error records nothing**: the command committed nothing, and
  a retry after the host's memory forgot it runs again, and may commit then.
- **A command that writes nothing records nothing**, as it commits nothing.
- **A retry answered from a record is not marked as one.** Stripe marks a
  replayed answer `Idempotent-Replayed: true` (read 2026-10-12); here the
  page's handler is given the command's answer, as it was, and cannot tell.
  Nothing reads the difference yet; a payment's confirmation may (item 6).
- **A key reused with another payload is answered as an error of the
  request**, as before (ADR-0121), and not the IETF draft's 422 by name.

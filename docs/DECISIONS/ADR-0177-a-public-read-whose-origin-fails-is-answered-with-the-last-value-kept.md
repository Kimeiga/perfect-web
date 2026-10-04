# ADR-0177: a public read whose origin fails is answered with the last value kept

Status: accepted under the owner's delegation of 2026-10-02. Date: 2026-10-04.
Milestone: E14, the audit's seventh gap, charter §15.6 test 18: "Origin
failure follows last-known-good policy only for declared public data."

## Context

- **`fallback last_known_good` was read by nothing.** The checker held its
  value to its domain (PW0335, ADR-0089). No rule kept it to public data,
  and the query runtime answered a failed origin with the failure, whatever
  it had kept.
- **The store's page answered 503 when its store's origin failed**
  (ADR-0147), though the store's last value was in the cache: `pw-resource`
  keeps a value past its freshness until a read replaces it, and a failed
  read does not.
- **A read could be made to fail since ADR-0174**: a session's cart. Nothing
  could fail a public read, to see what a page shows then.

## Decision

1. **PW0343: a last-known-good fallback serves only public data.** A query
   labelled `session` or `private`, or kept privately, may not declare
   `fallback last_known_good`. A shared materialization is public by
   ADR-0128's rule, and an unlabelled declaration is public to the manifest.
2. **The query runtime answers a public read whose origin failed with the
   last value kept**, where the read's manifest declares it. A failed load
   or a timeout is an origin failure; a stopped read is not, and neither is
   a declared error such as `StoreError.NotFound`, which is the origin's
   answer.
   - **For public data alone, by two checks.** The manifest must be public,
     and so must the value kept, whatever a manifest says. A public read
     never answers with a private value kept under its key, and a private
     read never answers with a public one.
   - `Fetched::LastKnownGood`, and the runtime's trace says so.
3. **The plan carries a query's `fallback`, and the host's manifest reads
   it.**
4. **The store's `Store` and `Menu` declare `fallback last_known_good`.**
5. **`POST /bench/store?fail=next`: the store's origin fails once**, for
   every reader. What was kept of the page's queries is expired and kept, so
   the next read asks the origin and the fallback has a value to answer with.

## Alternatives

- **The last value for private data too, to the session that owns it.** It
  would keep a copy of private data past the freshness its declaration
  allows, and a stale cart would hide the session's own writes, which
  `read_your_writes` promises it.
- **The materializer's last-known-good.** `pw-materialize` keeps one for a
  materialized fragment (charter §14 M6), and a page's queries are read
  through the query cache, not through it.
- **Answer 503 and let the page read again.** The store's name and menu are
  the same for every reader, and the last ones are still true of no one in
  particular; a page that cannot show them shows nothing.

## Acceptance

Recorded by `just e14-last-known-good` in
`docs/evidence/E14/last-known-good.txt`:

- `pw-resource` (`tests/last_known_good.rs`):
  - a public read declared `last_known_good` whose origin fails is answered
    with the last value kept, and the next is the origin's;
  - nothing else is: a read that declares none, a private one whatever it
    declares, a public read whose key holds a private value, a private read
    whose key holds a public one, and a read with nothing kept;
  - an expired value is read again, and kept for its fallback.
- PW0343 (`rules.rs`): refused for a session's query and for one kept
  privately; not for a public query or a shared materialization.
- The development server: while the store's origin fails, a page is served
  the last store kept, its origin asked and failed; a session's cart whose
  read fails is not served from anything kept.
- `e2e/controls.spec.mjs`, in Chromium, Firefox and WebKit: the store's
  origin failing, the page is shown with the last store and menu kept, and
  the origin was asked. The cart's failing read is answered 503, as before.
- `scripts/last_known_good_mutations.py`: 11 mutants.

## Not claimed

- **The page does not say what it shows is the last kept.** A page cannot
  yet tell a reader the store's name is from before its origin failed.
- **A stopped read** is not answered with the last value, by design, and no
  test stops one.
- **`fallback empty`** is held to its domain and read by nothing.

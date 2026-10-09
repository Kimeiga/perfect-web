# ADR-XXXX: a change is derived outside the table, and what reaches a document is recorded

Status: accepted under the owner's delegation of 2026-10-02, on CI's
finding (WebKit's "Load more", failing on CI since 2026-10-07). Date:
2026-10-09. Milestone: E14.

## Context

- **WebKit's "Load more" failed on CI, again and again** (runs
  37826467131, 37867532722, 37877464406; and every branch's WebKit job on
  2026-10-09): the feed's rows stayed at twenty. The runtime's own record
  showed the Load-more read answered `{"applied":1}` on an open stream,
  with no reconnect, and no frame applied in the four seconds after.
  ADR-0271 had told each reader once per burst of commits; the failure
  went from 13 of 160 runs to 2, and on CI it came back.
- **Reproduced here with the server's records** (3 of 120, six workers):
  every commit's telling derived each open document of the session it told
  inside the host's one subscriber table (`pending`), 50 to 200 ms a hold
  under parallel load, back to back, and every stream and every read on the
  host waited for it: the page's stream waited up to 1.6 s a pass, and the
  read's frames never reached it in time.
- **A derivation reads what the document shows**, and pushes the patches
  that turn it into the new value; it must be pushed only while the
  document still shows what it was derived against, or the page is patched
  from a state it is not in (ADR-0145, ADR-0161).
- **Nothing recorded where a document's frames went.** The runtime's record
  ends at the stream; a recipe keeps no attachment; a failure seen on CI
  alone left nothing to read.

## Decision

1. **A telling derives a document's change outside the table**: against
   what the document shows, read in a moment of its own; then, in the
   table, it pushes the change only if the document still shows exactly
   that (each shown value is replaced whole, never changed in place, and
   compared by pointer). Otherwise it derives again. The last of
   `DOCUMENT_ATTEMPTS` (3) is derived inside the table, where nothing can
   reach the document meanwhile, so a document every telling changes is
   still told.
2. **A keyed read is applied the same way** ("Load more"'s): derived
   outside, applied only if it is still the latest read and the document
   still shows what it was derived against.
3. **What reaches a document is recorded**: a trail of its latest 400
   notes (subscribed, each frame pushed or dropped behind, each telling
   with how long it derived and waited, each stream opened, written and
   ended, each pass that waited 50 ms or more for the table), and the
   table's holds past 50 ms, the latest 200, by site.
   `GET /bench/records?doc=N` answers a session its own document's trail
   and the holds, as the development server's other bench routes answer
   its tests.
4. **A failing test says what it recorded where it fails**: the feed's
   tests attach each page's runtime record and the server's, and print
   them on one line each (`pw-record`); a mutation script's red baseline
   shows a failing command's records (`mutation_baseline.explain`), where
   a recipe keeps no attachment.

## Acceptance

- **The development server's tests**: the table is free while a telling
  derives, and while a keyed read derives; a document changed while its
  change is derived is derived against again; a document changed at every
  attempt is still told, each attempt but the last outside; a document's
  trail says what reached it, and only to its own session.
- **`scripts/tests/test_mutation_baseline.py`**: a failing test's records
  are shown as it printed them, the last four, each cut; a passing
  command's are not.
- **The browser**: WebKit's "Load more" in CI's WebKit jobs.
- **`scripts/derived_outside_mutations.py`**, recorded by `just
  e14-derived-outside`: each part undone fails a test.

## Not claimed

- **A derivation's cost is unchanged**: outside the table it no longer
  holds every stream and read, but a telling of many documents still takes
  as long; ADR-0294 made each render linear in its lists.
- **The records are the development server's**, for its tests; a
  production host's tracing is E8's.

## Alternatives

- **A lock per document**: the table is also the frames' queue, and a
  telling pushes several frames that must reach a page together, in one
  hold; a lock per document would split what a page must see whole.
- **Deriving outside with no check**: a change derived against a state the
  document has left would patch the page from a state it is not in.
- **Retrying outside for ever**: a document a burst of commits changes at
  every attempt would never be told.

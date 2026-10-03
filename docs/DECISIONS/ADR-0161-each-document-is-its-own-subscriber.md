# ADR-0161: each document is its own subscriber

Status: accepted under the owner's delegation of 2026-10-02. Date: 2026-10-03.
Milestone: E14. The second step of the audit's route gap: a session can show
two stores only if each of its pages is kept current on its own.

## Context

The development server kept **one subscriber per session**:
- one queue of frames;
- one record of what the page shows (ADR-0145), against which each change is
  derived;
- one set of keyed reads (ADR-0152).

Serving a document cleared the queue and replaced the record, so the
session's newest document was its only page. With two tabs of one session
that is wrong in three ways:

1. **A change waiting for the first tab was lost.** The first tab was served,
   an add reached it, and before its page asked for it the second tab was
   served. The second serve cleared the add.
   `a_change_waiting_for_one_tab_survives_another_tab_being_served` failed on
   the server before this decision: "the first tab was never sent the add".
2. **Two tabs shared one queue.** A request's cursor acknowledges every
   frame up to it, and both tabs' requests acknowledged the one queue. So a
   frame one tab had applied could be dropped before the other read it. This
   is read from the code; no test caught it in the act.
3. **Two stores in two tabs would be sent each other's patches.** A change
   was derived against the newer document's record and sent to both. A text
   patch to the store's name applies cleanly to either, so nothing would
   notice.

The browser already named its document in a keyed read (`doc=`), and the
server used that only to drop reads from a replaced page.

## Decision

1. **A served document is numbered**, server-wide and from one. Its number
   is its first cursor, so a cursor is never zero.
2. **Each document has its own**, under `(session, number)`:
   - subscriber;
   - record of what it shows;
   - keyed reads.

   A session's second tab is a second document, beside the first and not
   replacing it.
3. **A change to a session reaches every document of the session.** Each is
   read with its own keys and patched against what it shows. A public
   change, the menu's, reaches every document.
4. **The browser names its document** in its subscription,
   `/stream?doc=..&since=..`, as it did in a keyed read. A request naming a
   document the server does not hold is told to reload, as a forgotten page
   was.
5. **A document is registered before it is read.** ADR-0151's check holds:
   a change that reaches the session while the document is read reaches its
   subscriber, and the document is read again.
6. **A document is forgotten when idle; a session goes with its last.** Its
   cart entry, private query values and interactions stay while any of its
   pages is open.
7. **A keyed read is its document's.** A read for a document the server does
   not hold takes nothing.

   This revises ADR-0152. ADR-0152 applied a read only "for a page that is
   still the session's". No page replaces another now.

## Found on the way

- **The browser suite served a build made with an older runtime.**
  `dist-keyed` was built by `keyed-store.sh` before the subscription named
  its document, and test 7 failed in three engines for that reason alone.
  - The suite's configuration now refuses a `dist` whose `pw-runtime.mjs` is
    not the one in `public/`.
  - A stale `dist-keyed` is not served. The keyed suite is left out with a
    warning naming the script to run. Refusing it would have failed, for no
    test's reason, every mutation control that changes the runtime and runs
    another spec.
- **A mutation run stopped from outside left its mutant in the source.**
  - Python's default SIGTERM runs no `finally`.
  - A mutant that adds beside its anchor leaves the anchor matching once,
    so `mutation_anchors.py` did not see it. It now reports such a mutant,
    found whole in its file.
  - `documents_mutations.py` bounds each run, kills a hung one with
    everything it started, and restores its source on SIGTERM. One of its
    mutants had hung on a lock taken twice.
- **Re-verifying the earlier scripts found two survivors.**
  - `patch_set`'s "a session's list is not rendered" had survived since
    ADR-0146. That ruling put every binding's whole value in a document's
    environment, so the loop the mutant removed set each list a second time,
    to the same value. The loop and the mutant are removed.
  - `stream_ack`'s "the stream acknowledges what it wrote" survived this
    change. Its test had a page and its replacement share a subscriber,
    which per-document subscribers rule out. It now tests one document
    whose stream wrote to a connection that dropped.

## Acceptance

- Server tests:
  - a change waiting for one tab survives another tab being served (it
    failed before);
  - a change reaches every document of its session, and no other session's;
  - a session is forgotten with its last document, not its first;
  - a keyed read is its own document's, and one for a document the server
    does not hold takes nothing.
- `e2e/tabs.spec.mjs`, three engines, both transports: two tabs of one
  session each hear the other's add, and neither reloads.
- The browser suite.
- `scripts/documents_mutations.py`: 6 mutants, each putting back one piece of
  the session-wide subscriber, the browser's for each transport. Recorded by
  `just e14-documents`.
- Seven mutants in six earlier scripts are re-anchored on the code they
  guard, and every mutant of those scripts is killed:
  - `document_reads`, 4;
  - `keyed_reads`, 10;
  - `optimistic_transitions`, 6;
  - `patch_set`, 14;
  - `query_values`, 7;
  - `stream_ack`, 2.

## Not claimed

- **A page's parameters per document.** Every document is still store 47's.
  The next ruling serves the store at its route.

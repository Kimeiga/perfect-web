# ADR-0139: a frame is forgotten when the page says it applied it

Status: accepted under the owner's delegation of 2026-10-02. Date: 2026-10-02.
Milestone: E14. A correction to the stream adapter (E7-P's transport), found
while building ADR-0138.

## Context

The server queues each session's frames, and drops a frame when a client's
next request acknowledges it: `/stream?since=N` says the page applied
everything through `N`. The long poll does exactly that. The stream adapter
holds a connection open for up to two seconds and writes frames as they
come, and its comment says:

> Written, but NOT acknowledged. The cursor advances here only for this
> connection's own reads; the subscriber's copy is dropped on the next
> request, which is the client saying it applied them.

The code did otherwise. Each pass of the loop acknowledged the cursor it had
just written, so a written frame was dropped 25 ms later by the same
connection.

That is harmless while the connection's reader is alive. It is not when the
page has gone:
1. A page is reloaded, or a second tab opens in the same session.
2. The old page's stream is still held on the server, writing into a socket
   nobody reads.
3. The new document is served with a cursor of its own.
4. A change made in those two seconds is written to the dead socket and
   dropped. The page that replaced it never receives it, and shows the old
   value until the next change.

The keyed-list suite does exactly that: reload, then a command. It failed
intermittently, 1 run in 10 at the commit before ADR-0138 and 4 in 10 after
it. The timing moved; the defect did not. Two suite runs during ADR-0138
failed this way, and passed when re-run.

## Decision

**A frame is forgotten when a client acknowledges it, never because it was
written.** The stream acknowledges the cursor its request sent, once. Within
the connection it reads past what it has written without dropping anything.
The page's next request, which carries the cursor it applied, drops what it
applied.

Delivery is at least once per session, which the cursor already made safe:
- a page reconnecting says what it applied, and is sent what follows;
- a frame written to a page that is gone stays for the page that replaced it.

## Alternatives

- **A queue per document instead of per session.** Each served document
  would get its own subscriber. That is correct too, and larger. It changes
  what a cursor names and how many queues a session holds, and the bounded
  queue (`MAX_WAITING`) and idle rule would apply per document. The defect
  is the early acknowledgement, and removing it restores the design the
  comment describes.
- **Detect the dead connection.** A write into a socket whose reader has
  gone usually succeeds into the kernel's buffer, so the server cannot know
  in time. What it can know is what the page says it applied.

## Acceptance

- **`a_frame_a_stream_wrote_to_a_page_that_is_gone_still_reaches_the_next`**
  (server unit test). An old page's stream is held on a socket nobody reads,
  a new document is served, and a command changes the cart. The test fails
  at the commit before, with "the change was written to a page that is gone,
  and dropped". Controls:
  - the old stream did write them;
  - a page that acknowledges them drops them.
- **`a_stream_drops_what_its_page_says_it_applied`**: the other half, so a
  live page's queue does not grow.
- **The keyed-list suite**, run alone: 20 runs of 20 pass with the fix,
  against 6 of 10 and 9 of 10 before it.
- The own-renderer suite passes across its browsers.
- **Mutation controls:** `scripts/stream_ack_mutations.py`, `just
  e14-stream-ack`, 2 mutants.

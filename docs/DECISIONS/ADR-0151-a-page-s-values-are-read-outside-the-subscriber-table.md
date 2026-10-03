# ADR-0151: a page's values are read outside the subscriber table

Status: accepted under the owner's delegation of 2026-10-02. Date: 2026-10-03.
Milestone: E14. A correction to the development server, found designing T02.

## Context

T02 asks for a duplicate request storm to be fixed. The store page shows the
kitchen's prep time, which is never kept, and asking the kitchen is slow.
Many customers open the page at once, and the kitchen is asked once for each.
In Pleris the fix is a policy: `concurrency one_per_key` instead of
`parallel`, so reads of one key while a flight is under way share it.
`pw-resource` has run that policy since ADR-0127.

Designing the task found that the server never let it apply to a page:
- **A document read its values while holding the subscriber table.** The
  hold was there so that a change could not land between clearing what was
  waiting and rendering, and be lost. But every query the page reads ran
  inside it. So page loads ran one after another, server-wide. A slow query
  held up every other page, and every command's frames, which need the table
  too. Two pages were never read at once, so they never shared a flight,
  whatever their query's `concurrency` said.
- **A command read every binding for the cart's count.** `drain` reads the
  count's text to regenerate the cart's entry, and read it through every
  binding the page has. After the command it read them all again for the
  patches. A slow query was asked twice per command, and each page load
  asked it once more than it showed it.

## Decision

1. **A document's values are read, and it is rendered, outside the
   subscriber table.** The table is held only to install it: what was waiting
   is cleared, the document takes its cursor, and what it shows is recorded.
2. **A change must not be lost between the read and the install.**
   - The session's subscriber is registered before the read.
   - Every frame pushed to it is counted, whether kept or dropped.
   - If one reached it during the read, that document is not installed, and
     the page is read again.
   - The last of three attempts is read inside the table, as before, so a
     page is always served.
3. **What counts is frames, not sequence numbers.** A document's cursor moves
   the sequence too. If sequence numbers counted, two pages of one session
   read at once would each make the other read again.
4. **The cart's count is read from the cart's binding alone.**

## Acceptance

The server's tests:
- Eight pages read at once, with the kitchen's query `one_per_key`, ask it
  at most twice. They take about one ask's time, not eight.
- Declared `parallel`, the same eight pages ask it eight times, at once.
- A page read before a change, whose frames reached the session during the
  read, is not installed. Read again, it shows the change.
- A command asks the page's other queries once, for what the page shows
  after it.

Also:
- `scripts/document_reads_mutations.py`: each of four pieces undone fails a
  test (`just e14-document-reads`).
- T02's controls on all three stacks.
- The browser suite.

## Not claimed

- **A session's two tabs.** What a document shows is recorded per session,
  as before: the later of two pages read at once is the one patched.
- **Fairness under load.** A page that keeps losing its read to changes in
  its session is read inside the table on its third attempt.

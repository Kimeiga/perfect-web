# ADR-0150: one change reaches a page whole, and a fragment shows its query's value

Status: accepted under the owner's delegation of 2026-10-02. Date: 2026-10-03.
Milestone: E14. Two corrections to the development server: one found hunting
an intermittent browser failure, one found designing T09.

## Context

**One change, two batches.** Recording ADR-0148's evidence, the browser suite
failed once in three runs: Firefox's "two fast presses are one fetch", whose
count was not 2 within five seconds. Eight more runs of the whole suite caught
a second rare failure: Chromium's "only cart-related part ids update". There,
the page's record of the patches it applied stayed empty for five seconds.
A WebKit failure of the same test had been seen once before, and not
explained.

After a command, the server pushed the change's frames in two holds of the
subscriber table:
1. the entry's new version, and its `patch_set`;
2. the table let go, the cart read again, and its `entry_value` pushed in a
   second hold.

So a subscriber could be sent one change in two batches. A test that reads
what the last batch updated then sees the second batch, which updated
nothing: the Chromium failure.

The second read also took the cart's value after any command committed in
between, under the earlier version's number. That is a frame claiming a
value at a version the value is not from. Its later frames correct it, so a
page only shows it for a moment. The Firefox failure ran through that path.
Its stuck value was never recorded, so this cause is likely but not proven.

**A fragment kept past its value.** The menu is materialized once and shared
by every reader (E7-P). The fragment was kept until a command invalidated it.
When the `Menu` query's value changed without one, the next document still
showed the old fragment: its source changed and its freshness was then spent.

## Decision

1. **One change's frames are pushed in one hold of the subscriber table**:
   the version, the `patch_set` and the `entry_value`. The value is taken from
   the snapshot the patches are derived from, never read again.
2. **A materialized fragment is kept while it shows its query's value.** It
   is rendered again, as a new version, when the value it would show differs.
   A page already open is not patched for such a change: no event announced
   it. A new document shows it.
3. **The browser suite's evidence records a failure's expected and received
   values**, so the next intermittent failure says what it saw.

## Acceptance

- The server's tests, 54 of them, including that a menu changed at its
  source shows once its value is read again. That test fails before.
- ADR-0122's control on the speculating page's value now anchors on the
  one-hold push.
- The browser suite, after the change: see `browser-suite.txt` at the
  commit that records this ADR's evidence.

## Not claimed

- **That the Firefox failure is explained.** It did not recur in 50 runs of
  its test alone, nor in eight runs of the whole suite before this change. Its
  stuck value was never recorded; decision 3 records the next one.
- **No sequential test fails before decision 1.** The race needs two commands
  interleaved with a drain. The fix removes the window rather than narrowing
  it.

# ADR-0234: a view's instance given a speculated value is rendered again with it

Status: accepted under the owner's delegation of 2026-10-02. Date: 2026-10-06.
Milestone: E14. Found building toward an optimistic reply on the feed's
thread page, which shows its thread through `Replies`, a view that contains
itself.

## Context

- **A view that contains itself is an instance made at run time**
  (ADR-0203): `<Replies post={thread} />` is one part of the page, rendered
  by the view's template, which the document carries.
- **A speculation renders again what reads the value it changes**
  (ADR-0172): each attribute, block and loop at the top of the page, and
  each text part from the module's functions.
- **An instance was none of these.** Found with a session's thread shown
  through such a view, a count beside it and a reply a press speculates:
  - the module listed no region, `regions: { thread: [] }`;
  - a press would have set the count to three and left the thread at two
    replies until the server answered, the page showing two values at once;
  - nothing refused it.

## Decision

1. **A view's instance given a speculated value, at the top of the page, is
   a region of the speculation.** It is rendered again as a block a
   speculated value decides is: by its template, from the speculated value
   and the page's signals. The browser renders an instance a signal gives
   the same way (ADR-0203).
2. **What it is given is what the browser holds**: the speculated value, a
   signal, or a name bound around it. Given anything else, a query's value
   the speculation does not change, it is refused by name. That value would
   be missing when the browser renders the instance.
3. **One nested where no region renders it is refused by name**, as a
   speculated value read there is (ADR-0172). Inside a block a query's value
   decides, a speculation would not reach it.

## Acceptance

- **`compiler/pw-core/tests/speculated_instances.rs`, 2 tests**, a
  session's thread shown through a view that contains itself:
  - the instance is a region of `thread`, rendered as a block, and listed in
    the page's manifest. A view given a value the page does not speculate on
    is no region.
  - one inside a block a query's value decides, and one given a value the
    browser does not hold beside the speculated one, each refused by name.
- **`scripts/speculated_instances_mutations.py`: 3 mutants**, recorded by
  `just e14-speculated-instances`.
- **`cart_lines_mutations.py`'s "a region's read of what the browser does
  not hold is not refused"** is re-anchored, its line repeated by the
  instance's check.
- **The workspace, 2,110 tests; the browser suite, 773 in three engines.**

## Not claimed

- **In browsers.** No page of the feed speculates on a value a view that
  contains itself is given, until the thread page's optimistic reply (ruling
  0122-d). The browser's call, `render_part`, is the one a signal's
  instance takes (ADR-0203).
- **An instance given a speculated value through a field**, `<Replies
  post={thread.first} />`: rendered again the same way, and not tested.

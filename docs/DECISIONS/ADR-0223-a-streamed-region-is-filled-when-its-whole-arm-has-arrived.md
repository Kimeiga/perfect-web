# ADR-0223: a streamed region is filled when its whole arm has arrived

Status: accepted under the owner's delegation of 2026-10-02. Found by an
intermittent failure of `slots.spec.mjs` in ADR-0220's and ADR-0222's
browser runs. Date: 2026-10-05. Milestone: E14.

## Context

- **A streamed region** (ADR-0148, ADR-0165) is sent pending.
  - Its placeholder sits between `<?start name="pw-N">` and `<?end>`.
  - Later in the same response comes its settled arm, as `<template
    for="pw-N">`.
  - A browser with the platform's out-of-order streaming applies the
    template at its end tag. In one without, the runtime applies it, from a
    `MutationObserver` on the body.
- **Two failures, in two of seven full runs, both in the recommendations
  slot.** Both ran while another tree's mutation controls loaded the machine.
  - **The first: the slot said "No recommendations right now".** The test
    had the recommender answer after 2.5 s, and the query's timeout is 3 s.
    On a loaded machine the read ran past it, and the page rightly showed
    the query's failure. The test outwaited a delay half a second short of
    the program's own limit.
  - **The second, found once the test held the recommender instead:** the
    slot showed one recommendation of two.
    - Its list ended in the second row's start marker.
    - The runtime had applied the template as soon as it saw it, while the
      parser had read only part of it, and removed the template.
    - What the parser read after that went into a template no longer in the
      document.
    - Reproduced in Chromium, an Add pressed while the slot was pending: in
      two runs of two, and in a test that does only that.

## Decision

- **The runtime applies a `<template for>` only once it has all arrived**:
  when the parser has put a node after it, or has read the whole document.
  This is the platform's own rule, which applies a template at its end tag.
- **The renderer writes a comment after each template**, `<!--/pw-N-->`, in
  the same write.
  - The parser puts it after the template's end tag, so the runtime need
    not wait for whatever the response sends next.
  - Without it, a region that settled first would wait for a slower one: the
    estimate for the recommendations.
  - The comment is no part's anchor: those begin `pw:`.
- **A test that needs a page seen before a slot fills holds the
  recommender, and lets it go once it has seen it**:
  - `/bench/recommendations?hold=1` closes a gate the recommender waits at,
    for ten seconds at most;
  - `/bench/recommendations/release` opens it;
  - setting the recommender again opens any gate it held.

  The tests that slept 2.5 s hold it instead. A test reads the page's own
  content, the estimate, and the placeholder, then releases it, well inside
  the query's timeout.
- **`/bench/split-fills?ms=N` writes each fill in two parts, `N` ms apart**,
  as a network that delivers a response in parts would. A test reproduces
  the part-parsed template with it, in every engine.

## Acceptance

- **`runtime/pw-render/tests/streams.rs`**: the patch is the template and the
  comment after it.
- **`e2e/slots.spec.mjs`, in three engines:**
  - `a region whose fill arrives in two parts is shown whole`, new. Without
    the runtime's wait it fails in Chromium, Firefox and WebKit.
  - The store's content, then the estimate while the recommendations are
    still held, then the recommendations once released.
  - An Add answered while they are held.
- **The development server's two tests that read a streamed response's
  end** expect each arm's comment before it.
- **The workspace, 2,055 tests; the browser suite, 752 in three engines**,
  with no failure.
- **`scripts/whole_fills_mutations.py`: 3 mutants**, recorded by `just
  e14-whole-fills`:
  - a template applied before its end;
  - no comment after a template;
  - a recommender not held.

## Not claimed

- **Why Chromium's parser stopped inside the template when an Add was
  pressed.** The fix does not depend on it: a template is applied whole,
  however its parts arrive.

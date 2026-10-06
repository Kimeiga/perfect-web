# ADR-0229: a computed condition decides its block

Status: accepted under the owner's delegation of 2026-10-02. It is ruling
0073-a for a block's subject and inside a block (ADR-0210). Ruling 0071-a
waits on it, so that its repair, `n > 0`, builds. Date: 2026-10-05.
Milestone: E14.

## Context

- **A block's subject was read by path** (ADR-0073). `{#if !b}`, `{:else if
  n > 0}` and `{#match List.get(xs, 0)}` checked and did not build: "an
  `{#if}` condition must be a value path".
- **ADR-0226 to ADR-0228 compiled a value a template computes**, at the top
  of a page and in a loop's row. One inside any other block was refused.
- **Ruling 0071-a** makes a condition a `Bool`, or a `List` or `String`
  tested non-empty, and refuses an `Int` "with the repair `n > 0`". It is to
  "land with or after 0073-a, so the repair builds".
- **The feed needs two.**
  - A draft too long should say so: `{#if String.length(draft) > 280}`.
  - A thread with no reply should say that: `{#if List.length(thread.replies)
    == 0}`.

## Decision

1. **A block's subject may compute.** `{#if}`, `{:else if}` and `{#match}`
   each read theirs by the path the compiler names, `#feed.app.PostPage~3`,
   with what it reads, as a text part's value is read (ADR-0226).
   - **From a query's value, a host's.** It is computed into what the page is
     rendered with. Its block is among those the host renders again when the
     value changes (ADR-0146).
   - **From a signal's, the browser's.** The host renders the first, from the
     signal's first value (ADR-0227). The block is one the browser renders
     again, as one a signal decides (ADR-0130):
     - its subject is computed by the page's module;
     - it is rendered again when the signal changes;
     - what is inside it may read the page's signals alone (ADR-0137).
2. **A value computed inside a block a host renders is the host's**, from a
   query's value, the subject's or one in it. It is computed into what the
   block is rendered with. It is not sent as a text part of its own: the
   block is rendered again whole.
3. **The browser's renderer is given what the page computes now.** A block
   it renders again is given each value the page's module computes from the
   signals as they are, beside the signals.
4. **Refused, by name**, each citing ruling 0073-a, which later steps
   build. They named ADR-0230 until ADR-0230 became ruling 0071-a's.
   - a value the browser would compute inside a block, which it computes at
     the top of the page alone;
   - a value from a query's inside a block a signal decides;
   - a block whose subject the browser computes, whose arms hold a view's
     signals. They start again when it shows another arm (ADR-0144), which
     the browser would know only by computing first.
   - a subject computed from a value the page speculates on.
5. **The feed:**
   - a draft too long says "Too long to post." as an alert, as it is typed;
   - a thread with no reply says "No replies yet.".

## Found

- **Each subject is read in one place.** `{#if}`, `{:else if}` and `{#match}`
  each read theirs where each was lowered, three copies. One function,
  `subject_of`, reads a path or a computed value for all three.
  `row_reads_mutations.py`'s three mutants of those reads are one now.

- **A test read the author's row before the server's had arrived.** In one
  run, Firefox's "a post reaches the author's timeline" read the author's
  link while it was still the pending row, by "You", as ADR-0222 shows it.
  The test waits for the server's row now. The page did as it should; the
  test raced it.

## Acceptance

- **`compiler/pw-core/tests/computed_conditions.rs`, 3 tests:**
  - the feed's two conditions: the thread page's computed into what it is
    rendered with and among its blocks; the home page's a live block, read at
    `draft`, rendered again for it, and the module's function;
  - an `{:else if}`, a `{#match}` and a value inside a block, each the
    host's, and none a text part of its own;
  - five refusals by name, with two controls.
- **ADR-0073's `template_values.rs`**: `{#if !b}` and `{:else if n > 0}`
  build.
  - ADR-0226's to ADR-0228's tests: a value in a block a host renders builds,
    and what moved names ruling 0073-a.
  - ADR-0227's counts what is set in place apart from a block.
- **The development server's tests:**
  - `a_condition_a_host_computes_decides_its_block_and_again_when_it_changes`:
    "No replies yet." for a thread with none, and a `{#if thread.likes > 2}`
    block sent rendered again after another session's like;
  - `a_condition_the_browser_computes_is_rendered_at_its_first_value`.
- **`e2e/feed.spec.mjs`, in three engines**: `a condition is computed: a
  draft too long, and a thread with no reply`.
- **`scripts/computed_conditions_mutations.py`: 12 mutants**: 10 against the
  compiler's and the server's tests, and 2 against the feed in three
  engines. Recorded by `just e14-computed-conditions`.
  - Eleven older mutants are re-anchored, two of them retired into one, in:
    - `computed_signals_mutations.py`;
    - `provide_mutations.py`;
    - `query_blocks_mutations.py`;
    - `row_reads_mutations.py`;
    - `signals_render_again_mutations.py`.
- **The workspace, 2,091 tests; the browser suite, 770 in three engines.**

## Not claimed

- **Still refused** (ruling 0073-a, later):
  - a value from several values;
  - one the browser would compute inside a block;
  - one from a page's parameter;
  - a hole in an attribute's text;
  - one in an arm from the names it binds;
  - one in a view that contains itself;
  - a subject from a speculated value.
- **0071-a**, which this lets land: next.

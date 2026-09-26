# ADR-0110: a resumable handler reads what it captures

Status: accepted under the owner's instruction of 2026-09-26 ("keep working
until its perfect"). Date: 2026-09-26. Milestone: E10 (charter §7.7,
ADR-0033, ADR-0058).

## Context

A resumable handler runs later, in the browser, with what it captured and
nothing else. `resumable(captures = { item }) => add_to_cart(item.id, ..)`
writes `item` into the document when the page renders, and the compiled
handler reads it back when the button is pressed (ADR-0033). PW5008, PW5016
and PW5007 hold each capture to what may be written: its type, its schema,
its privacy. Nothing held the handler to reading only its captures.

On 2026-09-26, at e2dcf9e, the store page with its Add handler
written `resumable() => add_to_cart(item.id, PositiveInt(1))` checked. The
handler reads `item`, the `{#each}` item, and captures nothing. `pw
emit-handlers` refused it: "`item` is not bound here". `pw check` passed a
program its build refuses. That is the reverse of ADR-0090, where the build
compiled what the checker refused.

## Decision

**A resumable handler reads what it captures, and what it binds itself**
(PW5025). A name in the handler's body is refused when all three hold:
- it resolves lexically to a binding outside the handler, such as a
  parameter of the page, an `{#each}` item or a `let` of the page;
- it is not the root of one of the handler's captures;
- it is not bound by the handler itself, through a parameter, a `let` or a
  pattern.

A record's shorthand field, `Pick { n: 1, item }`, reads `item` as a name
does. A declaration's name, a command or a function, is not a binding and is
not captured. Each name is reported once per handler, at its first read, with
the descriptor related.

## Acceptance

- **`compiler/pw-core/tests/handler_captures.rs`**, 3 tests. Each fails at
  efaa313, the code at e2dcf9e:
  - an `{#each}` item read and not captured, against the store's handler,
    and against one whose own `let` and the program's declarations are read;
  - a page parameter read twice and not captured, reported once;
  - an item read only through a shorthand field. Its captured control is not
    this rule's; the capture paths miss a shorthand read, which PW5017
    reports, and that is ADR-0111.
- **`function_captures.rs`** (ADR-0086) had handlers capturing a function
  `f` and reading the view's `n` uncaptured, so each held two defects. They
  capture `n` too now, and the function stays the one defect.
- **Corpus.** No rejected, rule or generality fixture's diagnostics change.
  The store, kiokun and the accepted corpus check clean: every handler in
  them captures what it reads.
- **Mutation controls:** `scripts/handler_capture_mutations.py`,
  `just e10-handler-captures`, 6 mutants.

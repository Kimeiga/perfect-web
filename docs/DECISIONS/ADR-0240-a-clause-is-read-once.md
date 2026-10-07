# ADR-0240: a clause is read once

Status: accepted under the owner's delegation of 2026-10-02. Date: 2026-10-07.
Milestone: E14. A correction, found with ADR-0237.

## Context

- **A clause naming declarations by their keys was read twice.** That is
  `depends_on`, `invalidates_on`, `emits` and `invalidates`.
  - Lowering read it with the grammar into its keys (ADR-0088). The checker,
    the contracts and the backend read those.
  - The resource graph split its text at its commas, counting brackets
    (`graph.rs`, `calls` and `split_args`).
- **The two readings disagreed:**
  - a value ADR-0237 reads past a missing comma was a key and no edge. The
    feed's thread with `invalidates_on Liked(id) Posted(_)` was refused by
    PW5107 beside PW0016, for a listener its clause names;
  - a parenthesis inside a key's string ended the key early. `emits
    Searched(")"), Other(1)` had no edge to `Other`, an event nothing
    declares, and it checked. ADR-0237 named a comma inside a string here.
    The splitter counted brackets, so a comma inside one was safe; a
    parenthesis was not.
- **PW5100 underlined the whole clause**, not the key that names nothing.
- **An interface's keys were lowered by nothing.** A declaration with no
  body is an interface (`effects.rs`), and its clauses' terms are lowered
  into its body's arena. ADR-0237 parsed an interface's clauses for their
  errors alone. So the graph's text was their only reading:
  - never bound: `invalidates_on Changed(nosuch)` on a `query` with no body
    checked, where one with a body is PW5104;
  - never typed or resolved;
  - and an interface command's `optimistic` clause was checked by no rule.

## Research

- **A second reader of one text is the mistake ADR-0237's research names.**
  rustc and Svelte each read a construct once with their grammar, and every
  later phase reads the tree. Here the graph read the clause's text again,
  with a smaller grammar of its own.
- **TypeScript checks an ambient declaration as it checks any**: `declare
  function greet(greeting: string): void;` has no body, and calls are
  checked against its signature
  ([declaration files by example](https://www.typescriptlang.org/docs/handbook/declaration-files/by-example.html)).
  An interface's clauses are part of what it declares.

## Decision

1. **The resource graph reads the keys lowering made.** Each key carries
   its arguments as written (`ClauseKey::written`), their whitespace
   collapsed as a policy's value's is. The edge labels are unchanged: the
   store's committed graph is byte for byte the same. The text splitter is
   gone.
2. **A key that names nothing is refused at its name** (PW5100).
3. **An interface's clause terms are lowered into an arena of their own**
   (`Decl::terms`), a body whose root is an empty block, which says nothing
   is implemented. `Decl::terms_body()` is that arena or the body's. `body`
   still says whether a declaration is implemented.
4. **Each rule that reads a term reads it from `terms_body()`**:
   - a key's names resolved (PW0021);
   - a listener's key bound to a parameter (PW5104);
   - a key typed against what it names (PW0605);
   - a command's speculations gathered (PW5107);
   - a transition's effects (PW0330), its value (PW0331) and its binders
     (PW0028).

   What reads an implementation still reads `body`: effects, placements,
   contracts, the backend, and PW0015's walk. An error node in a term comes
   only from a parse error, already reported.

## Acceptance

- **`compiler/pw-core/tests/clauses_read_once.rs`, 5 tests**, each with
  controls:
  - the feed's thread with a missing comma is PW0016 alone, and the graph
    has both its edges, labelled `id` and `_`;
  - `emits Searched(")"), Other(1)` is PW5100 at `Other`;
  - an interface's listener key unbound is PW5104, and typed wrongly PW0605;
  - an interface's clause with a missing comma is PW0016, once;
  - an interface command's clauses: PW0021, PW5107, PW0330, PW0331 and
    PW0028, each alone.
- **`pw-cli`'s committed store graph** matches, unchanged.
- **`scripts/clauses_read_once_mutations.py`: 11 mutants**, recorded by
  `just e14-clauses-read-once`.
- **`read_whole_mutations.py`'s "an interface's clauses are not parsed"**
  is re-anchored on the lowering of an interface's terms.
- **Run whole, re-anchored or near what this changed**:
  `read_whole_mutations.py` 18 of 18 killed, `computed_holes_mutations.py`
  23 of 23, `declared_once_mutations.py` 10 of 10, `listener_mutations.py`
  9 of 9, `nested_scope_mutations.py` 12 of 12.
- **The workspace, 2,138 tests; the browser suite, 782 in three engines.**

## Not claimed

- **A transition producing another type, written as a literal.** Found
  here, and its own ADR. PW0331 reads the transition's type through the
  older typer, which has no answer for `"no"`. So `optimistic Thing(x) as t
  => "no"`, where `Thing` holds an `Int`, checks, with a body or without.
- **An interface's terms in the backend.** An interface is not built.

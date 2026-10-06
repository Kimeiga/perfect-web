# ADR-0230: a condition is a `Bool`, or tested non-empty

Status: accepted under the owner's delegation of 2026-10-02. It is ruling
0071-a (ADR-0210), which was to "land with or after 0073-a, so the repair
builds": ADR-0229 builds it. Date: 2026-10-05. Milestone: E14. Amends
ADR-0071, whose "(ruling needed)" it answers.

## Context

- **ADR-0071 related a template's condition to a truth** (PW0609). It
  refused a sum type, taken apart with `{#match}`, and kept "the renderer's
  truth for the rest: a `Bool`, an `Int`, a string, a list and a record".
- **It left a mark**: "(ruling needed) Whether a template condition must be a
  `Bool`, as a code `if`'s is (ADR-0043)."
- **Ruling 0071-a**: "An `{#if}`/`{:else if}` condition or boolean attribute
  is a `Bool`, or a `List`/`String` tested non-empty. `Int`, a record or
  anything else is PW0609, with the repair `n > 0`." Its reason: "A record is
  always true; `Int` truthiness is JSX's `{count && …}` bug."

## Research

- **React** calls `messageCount && <p>New messages</p>` "a common mistake":
  with a count of `0` it renders the `0`. Its fix makes the left side a
  `Bool`, `messageCount > 0 && …`
  ([conditional rendering](https://react.dev/learn/conditional-rendering)).
- **Here the renderer showed nothing for `0`**, as JSX does not. But
  `{#if balance}` was true for a negative balance, and `{#if count}` does not
  say whether it means `count > 0` or `count != 0`. The repair says it.
- **A record is always true**: it is a value, and no value of it is false.
  An `{#if post}` decides nothing.
- **A list or a string tested non-empty is kept.** ADR-0042 writes `{#if xs}`
  around an `{#each}`, and kiokun's search page tests a list, `{#if hits}`,
  and a string, `{:else if q}`.

## Decision

1. **A condition is a `Bool`, or a `List` or a `String` tested non-empty.**
   An `{#if}`'s, an `{:else if}`'s, and a boolean attribute's.
2. **A number has no truth** (PW0609, revision 2, `template_condition_is_a_number`):
   "`{#if}` tests an `Int`, a number, which has no truth", with the repair
   "say what it tests: `n > 0`". A `Float` likewise.
3. **Anything else has none** (`template_condition_has_no_truth`): a record,
   an opaque type over anything, a map, a set, a function. The repair is
   "test a `Bool` it holds, or compare it".
4. **As before:**
   - a sum type is taken apart with `{#match}` (`template_condition_is_a_case`);
   - an `Option` or a `Result` is PW0600's (ADR-0074);
   - a value whose type is unknown decides nothing.
5. **The messages say "an" before a vowel**: "an `Int`".
6. **The renderer's truth is kept as it is**, for a program built before
   this, and what the checker no longer admits it never meets.

## Acceptance

- **`compiler/pw-core/tests/template_truth.rs`, 3 tests:**
  - an `Int`, a `Float` in `{:else if}`, a record in a boolean attribute and
    an opaque type, each with its message and repair;
  - a `Bool`, a list and a string tested non-empty, in each place, and the
    repair, `n > 0` and `score < 0.5`;
  - the repair builds.
- **ADR-0071's `template_operands.rs`**: `{#if n}` over an `Int` is refused,
  and `{#if n > 0}` is not. ADR-0074's `template_text.rs`: `disabled={n}` is
  refused, and `disabled={n > 0}` is not.
- **Corpus C17**: R-059, `{#if post.likes}` over an `Int`, and
  `operand_type`'s witnesses. Generality is 44 / 44. No existing program,
  fixture or witness tested a number or a record.
- **`scripts/condition_truth_mutations.py`: 6 mutants**, recorded by `just
  e14-condition-truth`. `template_operand_mutations.py`'s "no value has a
  truth" is re-anchored: the values that have one are named now.
- **The leftovers of ruling 0073-a cite the ruling.** ADR-0229 named their
  later step ADR-0230, which this is not; they say "(ruling 0073-a)" now.
- **The workspace, 2,094 tests; the browser suite, 770 in three engines.**

## Not claimed

- **A `Map` or a `Set` tested non-empty.** The ruling names a list and a
  string; one is compared, `Map.size(m) > 0`.

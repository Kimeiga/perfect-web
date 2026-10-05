# ADR-0225: a `String`'s length is an invariant

Status: accepted under the owner's delegation of 2026-10-02. It is the feed
reference app's "typed length limit on a post", which the owner's scope
names (NEXT 24). Date: 2026-10-05. Milestone: E14. Extends ADR-0179.

## Context

- **ADR-0179 made an opaque type's invariant bounds on an `Int`.**
  - `opaque type PositiveInt = Int where value >= 1`.
  - Every construction is shown to hold it at build (PW0622).
  - Every value from outside is held to it by the host, where the contract
    says.
  - PW0623 refused anything else, "a `String`'s length" by name, "until a
    program needs it".
- **The feed needs it.** A post's text was a `String`.
  - A browser's request could post an empty text, or a book.
  - The program could say neither "at least one character" nor "at most
    280" where the type is declared.

## Research

- **What a length counts.** The language's `String.length` counts code
  points (ADR-0040), in both backends and on the host.
  - **JSON Schema's `maxLength`** counts "characters as defined by RFC
    8259", which are code points (2020-12, validation §6.3.1).
  - **HTML's `maxlength`** counts the Infra standard's string length, in
    UTF-16 code units. An emoji is two there.
  - The invariant counts as the language does, so the build, a component
    and a host agree. A browser's `maxlength`, were one written, would
    count differently above U+FFFF.
- **The checks a host can make without the program's code** stay bounds
  (ADR-0179): a length's are two integers, as a value's are.

## Decision

1. **An opaque type over a `String` may bound its length**: `opaque type
   PostText = String where String.length(value) >= 1 &
   String.length(value) <= 280`.
   - Each bound compares `String.length(value)` with an integer, either way
     round, and bounds are joined by `&`.
   - An invariant bounds one thing, a value or a length.
   - PW0623 (revision 2) refuses:
     - a length of an `Int`, and a value of a `String`;
     - mixed bounds;
     - any other call;
     - a range no length holds.
2. **Every construction is shown to hold it, at build** (PW0622), as an
   `Int`'s bounds are.
   - A literal is its length in code points.
   - A bounded value's `.value` is its bounds.
   - A `let`'s local is its initializer, and a branch either of its values.
   - A test of `String.length(x)` narrows `x`'s length, in its branch, its
     negation in the `else`, and through `&` and `|`. The call is known by
     what it resolves to, however it is written.
   - Anything else is a `String` of any length, and the diagnostic says so:
     "it is a `String` of any length", "it is 281 code points long".
3. **The contract states the measure**: `"measure": "length"`, with the
   bounds. A value's is not written, so every contract that bounds none is
   as it was. The host counts a `String`'s code points where the contract
   says, and refuses one outside, by name: "value 1 is 281 code points long,
   and `feed.app.PostText` holds `…`".
4. **The feed's post is a `PostText`**, from 1 to 280.
   - `post(text: PostText)` and the data layer's `publish` take one.
   - The page builds one with the program's own checked constructor,
     `post_text(draft) -> Option<PostText>`, and sends nothing when there is
     none.

## Acceptance

- **`compiler/pw-core/tests/string_invariants.rs`, 5 tests:**
  - a literal is its length in code points: three emoji, 15 accented
    letters of 30 bytes, two too few and 16 too many;
  - a test narrows, its negation in the `else`, and a test of another
    value or of an `Int` narrows nothing;
  - a bounded value's `.value`, a `let` and a branch;
  - PW0623's refusals, and their controls;
  - the feed's contract bounds `post`'s argument's length from 1 to 280.

  ADR-0179's twelve tests pass with two messages reworded.
- **`runtime/pw-host/tests/bounded.rs`**:
  - a length held at both ends;
  - 280 emoji are 280, whatever their bytes;
  - a value that is no `String` is refused.
- **The development server**: an empty and a 281-character post refused
  before the command runs, nothing posted, and 280 emoji posted.
- **`e2e/feed.spec.mjs`, in three engines**: a 281-character draft is not
  sent and is kept; 280 emoji are sent and posted, one request in all.
- **`scripts/string_invariants_mutations.py`: 12 mutants**, recorded by
  `just e14-string-invariants`.
  - `invariants_mutations.py`'s five anchors in the code this changed are
    re-anchored.
  - `backend/js.rs`'s comment, which said the server holds an opaque value
    to nothing, is corrected.

- **The workspace, 2,063 tests; the browser suite, 755 in three engines.**

## Not claimed

- **A length counted otherwise.** No grapheme clusters, no weighting as
  Twitter's own count does, no normalization: a code point is one.
- **A text's other properties**, its characters or its form. A predicate
  other than bounds stays refused, as ADR-0179 decided.
- **A browser that says why a post was not sent.** The page keeps the draft.
  A count of what is left wants a computed hole (ruling 0073-a), the feed's
  next step.

# ADR-0074: what a template writes has a text form

Status: accepted under the owner's instruction of 2026-09-26 ("keep working
until its perfect"). Date: 2026-09-26. Milestone: E10 (charter §7.1, §8.1).

## Context

The renderer writes a `String`, an `Int` or a `Bool` as text, and refuses
anything else when it renders. The host passes an opaque type as its
representation, and drops a `Float` (kiokun's `value`). A probe run after
ADR-0073 found that nothing related what a template writes. On 2026-09-26,
at 8cd966f, each of these checked and failed only when rendered:
- **A value with no text form, written.** A `List`, a record, a function
  or a `Float` in a text hole (`{xs}`), an attribute (`title={xs}`), or a
  URL's hole (`href="/x/{r}"`).
- **A value that may be absent, written.** An `Option` in the same places.
  PW0600 read only an `{#if}`'s subject.
- **A correction to ADR-0071: `{:else if o}` over an `Option`.** ADR-0071's
  truth relation left an `Option` or a `Result` to PW0600, which says the
  same of an `{#if}`, and PW0600 did not read an `{:else if}`. So the
  condition was related to nothing, and the renderer refuses it.
- **A boolean attribute given a case**: `disabled={shape}`, over a declared
  sum type. The renderer tests a boolean attribute as a condition, and a
  case has no truth.
- **A loop keyed on a value with no text form, or on a field its element
  does not have**: `(x)` over records, `(k.r)` a record, and `(x.missing)`.
- **A loop over a field its value does not have**: `{#each s.itemz as x}`.
  A directive's fields were typed and never checked.

## Decision

- **What a template writes has a text form** (PW0609, "`{..}` is
  `List<Int>`, which has no text form"). That covers:
  - a text hole, and an attribute's value;
  - each hole of an interpolated attribute;
  - a loop's key.
  A `String`, an `Int` or a `Bool` has one, and so does an opaque type over
  one, written as its representation. A `Float` has no format yet
  (KNOWN_LIMITATIONS).
- **A value that may be absent is taken apart** (PW0600) wherever a
  template writes or tests one:
  - an `Option` or a `Result` in the same places;
  - in `{:else if}`;
  - in a boolean attribute.
  An `{#if}`'s subject stays `template_blocks`'s, as before.
- **A boolean attribute has a truth**, as a template condition does
  (ADR-0071).
- **A directive's fields are fields their values have** (PW0610): an
  `{#each}`'s list and its key, one field at a time. A key's head is
  PW5021's (ADR-0073).
- **Not text:**
  - a stream's `query`;
  - the name a stream's part binds (`<ready as={items}>`);
  - the attributes of an element that mounts a resource
    (`<map-container resource={StoreMap} center={c} />`, A-007), which
    are the resource's arguments.
  A directive other than `on:` does not build (ADR-0073), and is not
  related.
- **The name check reads a computed list as computed.**
  `{#each same(xs) as x}` was "`same(xs)` does not resolve". It checks
  now, and the build refuses it: "`{#each}` reads its list by path"
  (ADR-0073).

## Acceptance

- **`compiler/pw-core/tests/template_text.rs`**, 5 tests, each with
  controls. The fifth reads the relations of A-007 and A-008 themselves. All
  five fail at 8cd966f, the commit before, as does the computed-list case
  added to ADR-0073's `template_values.rs`.
- **Two generality witnesses were ill-typed, and are corrected.**
  - `unkeyed_list/caught.pw` read `g.items` over a `List<StoreId>`, whose
    elements have no `items`. It declares a record with `items` now.
  - `forbidden_effect/caught.pw` rendered a `Result`. Its helper answers a
    `String` now.
  Each emits its invariant's diagnostic alone, as before.
- **The rejected R-037 had a second defect, and is corrected.** It keyed
  a loop on a `Float` width, `(w)`, which has no text form. Each width
  travels with its element now, which keys the loop, and the fixture emits
  its declared `forbidden_effect` alone, as the rejected corpus requires. No
  other rejected, rule or generality fixture's diagnostics change.
- **The store, kiokun and the accepted corpus check clean.** Every relation
  in the store is decided.
- **The store's and kiokun's artifacts are byte-identical,** apart from
  ADR-0058's two handlers.
- **Mutation controls:** `scripts/template_text_mutations.py`,
  `just e10-template-text`, 16 mutants.

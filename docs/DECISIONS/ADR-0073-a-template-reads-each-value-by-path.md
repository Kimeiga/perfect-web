# ADR-0073: a template reads each value by path

Status: accepted under the owner's instruction of 2026-09-26 ("keep working
until its perfect"). Date: 2026-09-26. Milestone: E10 (charter §8.1, §8.4).

## Context

The renderer looks each value up by its path among the values it is given:
a name, or fields read from one. A probe run after ADR-0072 found four ways a
template built wrong. On 2026-09-26, at 4e64814, each checked and built:
- **A computed hole lowered with an empty path.** `{n + 1}` in text,
  `title={"lit"}` and `disabled={!b}` each carried an empty path, so every
  render failed (`MissingValue`). `{mk().a}` carried the path `.a`.
  KNOWN_LIMITATIONS said a hole must be a value path, and nothing enforced
  it.
- **`pw build` wrote a part the renderer refuses.** `{#if !b}` and
  `{:else if n > 0}` lowered to a `Blocked` part. `pw emit-template` refuses
  one; `pw build` exited 0 and wrote it.
- **A `style:` directive built as an attribute named `style:width`**, which
  a browser ignores, so the style was never applied. Rejected fixtures and
  the accepted A-018 write `style:` directives; no built program did.
- **A loop's key was read by its last segment.** `{#each ks as k (k.r.id)}`
  keyed on `k.id`, and `{#each rs as x (item.id)}` keyed on `x.id`,
  silently. A key read from the wrong field can give two elements one key,
  or change an element's identity when an unrelated field changes.

## Decision

- **A computed hole checks, and does not build.** It is valid Pleris: the
  charter's own sketch writes `disabled={!item.available}` (§8.1), and the
  rule fixtures compute in holes to exercise effects and privacy. This
  backend cannot render one, so the template IR lowers it as a `Blocked`
  part, with the reason. That covers:
  - a text hole, and an attribute's value, a boolean attribute's included;
  - each hole of an interpolated attribute;
  - the subject of `{#if}`, `{:else if}` and `{#match}`;
  - an `{#each}` list, which the name check already refuses, as a name
    that does not resolve.
- **`pw build` refuses a template with a `Blocked` part**, as
  `pw emit-template` does.
- **A directive other than `on:` does not build.** That covers `style:`,
  `class:` and the like. An XML namespace (`xml:`, `xlink:`, `xmlns:`) is
  part of an attribute's name.
- **A loop's key is its element, or a field read from it** (PW5021, "a
  loop's key is its element, or a field read from it"). Its head is the
  loop's name. The template IR keeps the key as the path from the element:
  `r.id` from `(k.r.id)`, and empty from `(x)`. The renderer follows the
  path when it renders the loop, when it renders an inserted instance, and
  when it derives a patch's token, through one function.

A view used in another view is refused at check (ADR-0072), because its
props would otherwise be checked by nothing. A computed hole's operands are
checked where they are written, so it is refused where it is built.

**(ruling needed)** Whether a computed hole should compile. It could be a
value the page computes before it renders, one per hole, which the renderer
would read by a path the compiler names.

## Acceptance

- **Tests, each with controls.** All seven fail at 4e64814, the commit
  before:
  - `compiler/pw-core/tests/template_values.rs`, 5 tests;
  - `runtime/pw-render/tests/keys.rs`, 2 tests.
- **No rejected, rule or generality fixture's diagnostics change.** The
  fixtures that compute in a hole (R-016, R-017, R-024, the layout fixtures,
  A-018) check as before, and none is built.
- **The store, kiokun and the accepted corpus check clean.** Their artifacts
  are byte-identical, apart from ADR-0058's two handlers: every key they
  write is `(x.id)`.
- **Mutation controls:** `scripts/template_value_mutations.py`,
  `just e10-template-values`, 11 mutants.

## Found while building it

The last-segment audit (`compiler/pw-core/last-segment-audit.txt`) filed
`template_ir.rs:parse_each` as `syntax`: "pure text handling of source
syntax; nothing is resolved". It resolved a key by its last segment, which
is the audit's `forbidden` category, and the wrong key above is what that
cost. The site is gone, and so is its entry.

The name check reads an `{#each}` list as one name, so it reports a
computed list as a name that does not resolve: "`same(xs)` does not
resolve", and "`List` does not resolve" for `List.reverse(xs)` in a module
that imports `List`. The program is refused, with the wrong reason. The name
check's scope walk is the next item in NEXT.

# ADR-0097: data embedded in a page cannot end its script element

Status: accepted under the owner's instruction of 2026-09-26 ("keep working
until its perfect"). Date: 2026-09-26. Milestone: E10 (charter §14 M7; the
architect's escaping ruling of 2026-08-06).

## Context

A page with dynamic parts carries its parts manifest as data:
`<script type="application/json" id="pw-parts">{json}</script>`. Both the
render binary (`pw-render`) and the development server wrote it that way.
Each broke only `</script`, in lowercase, and the binary's comment said
that was "the only sequence that ends the element". Two things falsify it:
- the HTML tokenizer ends a script element at `</script` in any case, so
  `</SCRIPT>` in the JSON ends it too, and what follows is parsed as HTML;
- `<!--` followed by `<script` moves the tokenizer to a state in which the
  element's own `</script>` does not end it, and the rest of the page is
  swallowed.

The manifest holds what the compiler wrote, template paths and part names,
not a reader's data, so no page served today is known to carry either
sequence. The escape is still wrong for the one context it exists for.

## Decision

**Data embedded in a script element holds no `<`.**
`pw_render::escape::json_in_script` writes each `<` as `<`, which a
JSON parser reads as the same character, and which JSON can hold only
inside a string. The render binary and the development server both call it.
Neither the store's nor kiokun's artifacts change, since none of their JSON
holds a `<`.

## Acceptance

- **`runtime/pw-render/tests/security.rs`**, a fifteenth test:
  - `</SCRIPT>`, `</script >` and `<!--<script>` in a value leave no `<` in
    the embedded text, and the JSON reads back as the same data;
  - JSON with no `<` is written as it is.

  The test does not build at the commit before, where no such escape
  exists. Its first mutation control is that commit's escaping, and the test
  fails under it.
- **Mutation controls:** `scripts/embedded_json_mutations.py`,
  `just e10-embedded-json`, 2 mutants.

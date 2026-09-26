# ADR-0095: an attribute's context is read as HTML reads its name

Status: accepted under the owner's instruction of 2026-09-26 ("keep working
until its perfect"). Date: 2026-09-26. Milestone: E10 (the architect's
escaping ruling of 2026-08-06).

## Context

The template IR chooses how a value is escaped from the name of the
attribute it sits in (`Context::of_attribute`):
- `href`, `src`, `action` and seven more are URLs, whose executing schemes
  the renderer refuses;
- `style` is a style.

HTML lowercases an attribute's name, so `HREF` is `href`. The IR compared
the name as written. On 2026-09-26, at 59ee245:
- **`<a HREF={msg}>`, `<img Src={msg}>` and `<p STYLE={msg}>` were escaped
  as ordinary attributes.** An ordinary attribute refuses no scheme, so
  `msg = "javascript:alert(1)"` in `HREF` ran when the link was followed.
- **`<a HREF="javascript:go()">` escaped ADR-0094's rule**, which asks the
  same table whether an attribute is a URL.

## Decision

**An attribute's context is read as HTML reads its name**: its local part,
after any namespace, lowercased. `HREF`, `xlink:HREF` and `formAction` are
URLs, and `STYLE` is a style. Nothing in the corpus writes an attribute in
capitals, and no artifact changes.

## Acceptance

- **`compiler/pw-core/tests/attribute_case.rs`**, 2 tests, each with a
  control: the table, and ADR-0094's refusal of a script URL however its
  attribute is spelled. Both fail at b521ec5, the commit before.
- **No rejected, rule or generality fixture's diagnostics change.** The
  store's and kiokun's artifacts are byte-identical, apart from ADR-0058's
  two handlers.
- **Mutation controls:** `scripts/attribute_case_mutations.py`,
  `just e10-attribute-case`, 2 mutants.

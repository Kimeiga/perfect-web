# ADR-0232: an opaque value's representation is read in a template

Status: accepted under the owner's delegation of 2026-10-02. Date: 2026-10-06.
Milestone: E14. Found by ADR-0231's probes.

## Context

- **An opaque type's `.value` is its representation** (ADR-0048). The
  checker allows it in the module that declares the type. At run time an
  opaque value is its representation (ADR-0054): a `PostId` is its text.
- **A template read `.value` as a field.** It reads a value by a path, and
  `{p.id.value}` became the path `p.id.value`. The renderer found no field
  `value` in the text `"p1"`. In the feed:
  - `{p.id.value}` in the home page's rows, an attribute's text,
    `href="/post/{p.id.value}"`, and an attribute's whole value;
  - `{id.value}`, the thread page's parameter, and `{thread.id.value}`, its
    query's;
  - `{r.id.value}` in a `{#match}` arm, and `{post.id.value}` in a view.

  Each checked and built. Then every request for its page was answered
  503: `MissingValue { path: "p.id.value" }`.
- **The template lowering knew no types.** It wrote a path from the
  expression's shape alone. A record may have a field named `value`, as
  `Pair { key, value }` does, so the shape cannot tell the two apart.

## Decision

1. **A `.value` of an opaque value is read by the opaque value's own
   path.** `{p.id.value}` is read as `p.id`: the same value at run time.
   - Wherever a template reads a value by its path: a text part, an
     attribute's whole value or its text, a block's subject, a loop's list,
     an instance's argument, a title and metadata.
   - An opaque type over another is read through each: `{o.value.value}` is
     `o`.
2. **A record's field named `value` is a field.** What the base is, the
   checker says: the template lowering is given the program's signatures
   and types each body it lowers, a page's or a composed view's.
3. **Where the checker's types leave a base untyped, the value relations
   type it.** They solve a call: under `{#match List.get(thread.replies,
   0)}`, the arm's `r` is a `Post`, and `r.id` a `PostId`.
4. **The speculation module compares a view's path with the template's as
   the template reads it** (ADR-0172). Read as a field, a view's
   `{tally.n.value}` and the template's `t.n` differed past their first
   name, and the page's speculation was refused.

## Acceptance

- **`compiler/pw-core/tests/template_representations.rs`, 3 tests:**
  - the feed's reads above, each by the opaque value's path, and the thread
    page's plan reading `thread.id`;
  - a record's field named `value` a field, and an opaque type over another
    read through both;
  - a speculated tally whose count is an opaque type, read through a view
    and at the top of the page, compiled.
- **The development server's test**
  `an_opaque_values_representation_is_rendered`: the home page and the
  thread page, each answered 200, with the row's attribute, link and text,
  the parameter's, the query's, the arm's and the view's text.
- **`scripts/template_representations_mutations.py`: 4 mutants**, recorded
  by `just e14-representations`.
- **Five older mutants re-anchored** on the path as the template reads it:
  - `template_value_mutations.py`'s three;
  - `row_reads_mutations.py`'s "what an `{#if}` decides by is not a read";
  - `signals_render_again_mutations.py`'s "the plan does not know what a
    handler captures", on the lowering given the signatures.
- **The workspace, 2,103 tests; the browser suite, 773 in three engines.**

## Not claimed

- **A `.value` neither typer types the base of.** It is read as a field, as
  before, and does not render.
- **No existing program changes.** None read `.value` in a template.

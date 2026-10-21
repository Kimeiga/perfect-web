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

## Train-12 proof amendment (2026-10-20)

Hosted run 39526960864 at `f15117de` preserved a real gap in this decision's
proof: the original four-test baseline passed, but only three of the four
controls failed. ADR-0351 now supplies the original concrete match binding's
type through `Types::of`; removing decision 3's fallback no longer changes
that old witness. The failed hosted report remains historical evidence.

The fallback is still needed. In a valid generic function over
`List<Id<T>>`, where `opaque type Id<T> = String`, `Some(item)` from
`List.get(items, 0)` has a caller-owned type parameter. The value solver
knows the opaque nominal head; its `ResolvedType` projection is absent.
The new fourth core test checks that the whole program checks and builds,
asserts this absent projection, and asks the existing public
`value_path_typed` consumer for `item.value`. It must return `item`.
The unchanged original fallback-removal operator instead returns
`item.value`: exactly one native assertion fails while the other three
core tests pass. No compiler behavior or operator has been changed.
This is a generic function/path-consumer witness, not a claim that generic
view declarations are supported by the syntax.

All four original fault models remain byte-for-byte unchanged. The campaign
now requires complete named Cargo inventories (four core tests and one host
test), native exit agreement, and failure at an assertion macro's exact
source line and column through the shared `cargo_assertion_results`
authority. Build errors, ordinary panics, missing/ignored tests, timeouts,
and memory-bound stops earn no kill. Expected build/template/plan results
are asserted before extraction, preserving useful contract failures.
`producer_process` owns process cancellation; `mutation_source` restores
each changed source and preserves cancellation identity. The unused browser
producer copy is removed; the original final store/feed/server restoration
build is retained and its status now checked. Cancellation guarantees source
restoration, not that generated build outputs have been rebuilt afterward.

Eleven focused producer consumer tests and ten independent assertion controls
hold this wiring, including an explicit no-`KILLED` check for stopped
producers as well as a failing campaign status. They reuse the existing
native Cargo and Python receipt authorities rather than introducing another
parser. The recipe explicitly runs the producer control script once before the original
four-fault script. The original runner does not call it indirectly. This keeps
both count blocks owned by the literal commands the evidence gate reads.

Local WSL proof on base `8017e328` plus this amendment's source changes:
all five native baseline tests passed; all four original faults and all ten
producer controls were killed by valid assertions, with zero refusals,
survivors, or memory stops; the final restoration build succeeded. Exact
commands, source hashes, the historical failed report, and native before/red/
green output are retained in
[`representations-proof.txt`](../evidence/tooling/representations-proof.txt).
The hosted integration campaign remains a separate required check; this
local result does not rewrite or relabel the original failed run.
The integration review caught that nested campaign calls are intentionally
not discovered by `evidence_counts`. The recipe now names both commands,
and the producer uses the existing count/verdict format and class-qualified
witness metadata. A focused eleventh consumer test requires each command
once. An actual generated evidence file combines a fresh native ten-control
report with the retained unchanged four-fault report: count/owner validation
passes; omitting either the producer command or its output block fails.
This command relocation does not change any fault or validation rule and
was checked without repeating the expensive four semantic fault runs.
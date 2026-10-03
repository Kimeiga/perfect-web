# ADR-0149: `pw diff`, what a change means, for review

Status: accepted under the owner's delegation of 2026-10-02. Date: 2026-10-03.
Milestone: E14-D, gate item 1. Charter §14 M14 task 3, §19.2.

## Context

Charter §19.2: review should focus on product meaning, permissions,
consistency and tests, "rather than mechanically rediscovering
compiler-enforceable rules". A line diff of a Pleris change shows text. A
reviewer needs what the text did:
- which effects a declaration gained;
- which capabilities a node must now grant;
- what is now cached, for whom, and for how long;
- what a page now reads and waits for;
- how many bytes reach the browser.

E14's gate item 1 is "Semantic diffs are generated for the store app." Until
now there was no such tool.

## Decision

1. **`pw diff OLD NEW [--json]`.** Each side is a directory, and its program
   is every `.pw` file under it, with the standard packages added to both. A
   benchmark sandbox, the store as the task left it, is exactly such a
   directory. Each side is checked and built as `pw build` builds it.
2. **A side that does not check has no meaning to compare.** `pw diff`
   refuses it, naming its first errors. For most of the benchmark's unsafe
   patches, that refusal is the review.
3. **Each side is reduced to a model** (`pw_core::semantic::Model`): plain,
   sorted data, free of spans, so a comment or a reflow changes nothing. It
   holds:
   - types: records' fields, and sums' cases and payloads;
   - declarations: kind, visibility, declared effect row, and policies;
   - components: capabilities, placements, imports, and bytes;
   - handlers: identity and bytes; speculations' bytes;
   - pages: what each reads, streams, and holds as signals;
   - the resource graph's edges;
   - unsafe escape hatches, by declaration;
   - warnings.
4. **The report is §19.2's**, one section each:
   - domain, effects, capabilities, privacy, placement;
   - cache and freshness, invalidation, pages;
   - client impact, server components, obligations;
   - unsafe, diagnostics.

   Each change is one line: what, where, and the old value beside the new. An
   empty section says `none`, so its absence reads as checked.
5. **A policy no section names is reported under "Other policies"**, never
   dropped. A table that ignores what it does not know is how a policy goes
   unreviewed.
6. **A handler is reported by its name, or by its identity when it has none.**
   A changed identity is reported as a change: a press on a page already
   served then loads new code.

## A finding, on the first use

Diffing T05's reference reported that the store page now needed
`network.fetch`. A page's contract counted the call in `<stream
query={Recommendations(id)}>` as the page's own. A `let` of the same query
was not counted. The host runs the stream's query as its own component,
under its own contract, as it runs a handler.

So the call is now excluded from the page's authority, as a handler's body
is, and the page asks nothing more to render. Least authority: the page was
granted what it never uses.

## Acceptance

- `compiler/pw-core/tests/semantic_diff.rs`, 7 tests:
  - the same program changes nothing, and every section says so;
  - a comment changes nothing;
  - T04's reference is one domain change: `Ready`;
  - T05's reference is one page change, a stream;
  - T05's setup brings its query's capability, visibility and policies;
  - T09's reference is one freshness change;
  - a program that does not check has no model.
- `stream_plan.rs`: a page that streams a query does not take its authority.
- `just e14-diffs` records `pw diff` over every task's Pleris reference and
  unsafe patch (`docs/evidence/E14/semantic-diffs.txt`): gate item 1.

## Not claimed

- **State machines.** Pleris has no construct for one yet. A sum type's
  cases are domain changes.
- **Effects** are the declared rows. A query's effects are inferred, and its
  component's capabilities report them.
- **Client bytes** are the compiled handlers and speculations. The runtime is
  the platform's, and is not counted.
- **Tests and obligations** are what the program declares: a command's
  `requires`, `idempotent_by`, `optimistic` and `rollback`.

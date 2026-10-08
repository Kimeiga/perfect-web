# ADR-0273: a materialization is a value its body derives

Status: accepted under the owner's delegation of 2026-10-02; the first part
of ruling 10's last piece, "materializations made real" (ADR-0195,
ADR-0210's 0092-a, queued by ADR-0255). Date: 2026-10-08. Milestone: E14.

## Context

- **A materialization was declared and inert** (KNOWN_LIMITATIONS): its
  clauses checked, its edges in the graph, and no body, type or generator;
  no page could read one. `materialize M(..) -> T` parsed, and the `-> T`
  was never written; statements after the clauses were never checked. The
  store's `MenuFragment` is wired by hand in the development server, which
  never names it.
- **The charter's materialization** (§9.4, Milestone 6) is "a materialized
  view over resources, not a route timer", and ruling 0092-a (ADR-0210) put
  derivation there: "A query does not `depends_on` another resource;
  derivation is a materialization."
- **A page states what it reads by reading it**, `let menu = query
  Menu(id)`, and the graph takes those reads as its edges: "requiring one
  [clause] would make the graph a second place to state a fact the body
  already states" (`graph.rs`). Inference already typed such a read in any
  body as the resource's value (ADR-0146).
- **A materialization was in the namespace of views** (`Namespace::Ui`),
  from when it was a fragment of a page: `query M(..)` never named one, so a
  read of one was untyped and unchecked, and a test of a chain passed on
  nothing.
- **Inside a block, a clause's value ended only at a known head**: a
  clause, a declaration, a statement or a markup noun. A view's or a page's
  body always begins with one; a materialization's need not, and `regenerate
  on_invalidation` took a body `n` as `on_invalidation n`.
- **Prior art.** Next.js 16.4's `revalidateTag` marks a tag's data stale
  and rebuilds it on the next request, serving the stale value meanwhile; a
  timeline at Twitter was a list kept per reader, written on each post of
  those followed (R. Krikorian, "Timelines at Scale", QCon SF 2012); Noria
  (OSDI '18) keeps views over a dataflow, built on demand. Each keeps a
  derived value, and each is told when what it derives from changes; how it
  is rebuilt, and when, is what they differ on.

## Decision

1. **A materialization that declares its type derives it in its body**:
   `materialize M(p) -> T { clauses; body }`, the body an expression of `T`,
   held to it as a query's is. One that declares no type is a fragment the
   host renders, its body its clauses, as before.
2. **Its body reads what it depends on as a page does**, `query R(..)`, and
   the graph takes those reads as its edges: a cycle through them is
   PW5109, a shared one reading a private one PW5101, as through
   `depends_on`. A `depends_on` beside a body is PW5110: the fact stated a
   second time.
3. **A materialization is a term**, read by `query` as a query is; nothing
   calls one still (PW0027): the materializer runs it.
4. **One that derives its value is read by another that derives its own**
   (PW5108 otherwise); a fragment that declares no type derives no value, and
   is read by none; and a page reads one once the host serves it.
5. **Inside a block, a clause's value ends with its line, unless it cannot
   have**: a bracket still open, or a line ending in a comma or an operator,
   goes on. A body after the clauses is the body, whatever it begins with. A
   header's clauses end at the body's `{`, and read on across a missing
   comma, which a check names (ADR-0237).

## Acceptance

- **`compiler/pw-core/tests/materialization_bodies.rs`, 6 tests**, each with
  its control: a body held to its type; `depends_on` beside a body refused,
  and kept by a fragment; a chain of two that derive, a field the read value
  has not refused, and a cycle through reads refused; a fragment read by
  none; a page that reads one refused, the host serving none yet; and a body
  beginning with a name, a literal, a string, a constructor or a call, where
  a line ending in a comma reads on.
- **`calls_name_terms.rs`**: a materialization called is PW0027, as before.
- **`scripts/materialization_body_mutations.py`, 11 mutants**: the body not
  held to its type; a materialization no term; `depends_on` beside a body
  not refused; a body's reads no edges; one that derives its value reading
  none that derives its own; a fragment read as a value; a materialization
  called; `depends_on` looking among the views; and, inside a block, a
  clause's value reading on past its line, a header's ending with it, and a
  line ending in a comma ending its clause. Recorded by `just
  e14-materialization-bodies`.
- **The scripts over the code this changed, each run whole**, every mutant
  killed, 130: `call_name` (two re-anchored to `names_no_term`'s new
  argument), `clauses_read_once`, `query_reads`, `clause_key`, `listener`,
  `materialization_chains`, `any_type`, `e9_value`, `bare_cases`,
  `computed_holes`, `every_entry`, `nested_scope` and `optimistic_posts`.
- **The whole workspace's tests**, the corpus among them.

## Not claimed

- **Its generator, and what serves it**: no materialization runs yet. Its
  body compiled, its reads made host calls, the materializer regenerating it
  on invalidation, in a chain's order, and a page reading it, are the next
  parts.
- **A fragment the host renders** keeps its clauses and no body; the
  store's `MenuFragment` is still wired by hand.
- **A read whose key is not the materialization's parameters**, a timeline
  reading who you follow and then their posts, checks as any read does; its
  edges' keys are the reads', as a page's are.

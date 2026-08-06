# E6F — Canonical frontend convergence

**Status: COMPLETE.** Inserted before E7 by architect ruling, 2026-08-06.

## Question

> Is there only one parser that decides what a `.pw` program means?

**There was not.** `pw-syntax` shipped two: a Rowan tree grammar and a
hand-rolled declaration parser with its own AST, behind `pw explain` and the
declaration rules. They are now one.

```text
one source grammar
    ↓
Rowan syntax tree
    ↓
HIR
    ↓
all semantics and tools
```

## Why it was inserted here rather than done later

E6 produced the evidence, and it is not the usual duplication argument.

The two parsers had **four independent tables describing one language** —
`DECL_STARTERS`, the UI/resource noun tables, `POLICY_KEYWORDS`, and their own
whitespace normalisation for policy values. Adding `event` updated one of each,
so the same file was a declaration to one parser and "expected a declaration" to
the other, in the same build.

Worse, and the reason this is correctness infrastructure rather than cleanup:
the tree grammar learned to read a `materialize` block's policies and the other
did not. `decl.policies` was empty for every materialization in the corpus for a
whole milestone, and the analyses downstream built a coherent story on
information that was never there.

> Architect ruling: That is exactly the kind of architecture I would **not**
> carry into a from-scratch renderer. Doing it afterward would mean debugging
> renderer behavior while being unsure which interpretation of the source
> produced it.

## What moved

| consumer | was | is |
|---|---|---|
| `pw explain` | `ast::SourceFile` | `hir::Hir` |
| `rules.rs` (declaration rules) | `ast::SourceFile` | `hir::Hir` |
| generality validity pipeline | legacy `parse` | `parse_tree` |
| robustness suites | legacy `parse` | `parse_tree` + lowering |
| fuzz harness target | legacy `parse` | `parse_tree` + lowering |
| `pw check`'s syntax errors | `ParseError` | `SyntaxError` |

1,484 lines deleted: `parser.rs` (1,043), `ast.rs` (169), and the corpus test
that asked the legacy parser its questions (180, ported).

`hir::Decl` gains `name_span`. The AST had one; the HIR did not. A diagnostic
that says "`Cart` is declared here, so its result is `Session<SessionId>`" wants
to underline the name, and pointing at the whole declaration says the same words
about twenty lines.

## The gate is not the line count

> There is only one parser that decides what a `.pw` program means. No semantic
> correctness path should still ask the legacy parser a question.

`just one-parser`, five tests:

| test | what it establishes |
|---|---|
| `every_declaration_keyword_reaches_the_hir_as_a_kind` | the `event` control generalised: a keyword in the one table is understood by the one pipeline, with no second definition to update |
| `the_keywords_excluded_from_that_check_are_still_parseable` | the exclusion list is not an escape hatch — without it the first test gets greener the more of the language stops working |
| `a_policy_keyword_reaches_the_hir_from_both_positions_it_can_be_written_in` | E6's actual defect, pinned: one table, two syntactic positions, and lowering must find both |
| `every_policy_keyword_is_read_as_a_policy_and_not_as_an_expression` | the generalisation of the same |
| `no_crate_outside_pw_syntax_builds_a_second_semantic_tree` | the architectural property, structurally — a second parser can be added tomorrow and every behavioural test above would stay green |

**The tests read the real tables.** `DECL_STARTERS` and friends are `pub` and
enumerated directly. A test with its own list of keywords would be the fifth
copy of the thing this milestone removed, going stale the same silent way, in
the direction that looks like a pass.

## What it found

**One, and it is the sharpest instance in the project's log.**

`examples/generality/task_detached/neighbour.pw` wrote `task.spawn(scope
component)` — a policy-clause spelling inside an argument list, which is not the
language. The legacy parser accepted it; the real grammar never did.

The **validity pipeline** — the machinery built specifically to establish that a
witness is valid evidence — asked the *legacy* parser whether the file parsed,
while every analysis ran on the tree parser. So it certified "parses without
recovery" for a file no analysis could read. The valid neighbour proving the
affine rule does not ban all spawns reported nothing, because nothing had read
it.

A witness that proves nothing, certified by the instrument built to detect
witnesses that prove nothing. `docs/RISK_QUEUE.md` instance 22.

Fixed to `scope = component`, the named-argument form the grammar has — the same
shape as `bounded_exponential(max = 3, jitter = true)` in the corpus.

## What is retained rather than ported

`corpus_parses.rs`'s attribute checks — `@id`, `@category`, `@rule`,
`@expect-error` — are not ported. `corpus-check` validates all four on every
fixture, and the old test's own comment said it existed only "until the parser
exposes them, [so] that duplication can go away". Two readers of one fact is the
shape this milestone exists to remove.

Its other four properties are in `pw-core/tests/corpus_declarations.rs`, against
the HIR: every file lowers a module, every file lowers a substantive
declaration, effect rows survive lowering, and policies survive lowering — the
last counted **per declaration kind**, because E6's defect was confined to one
and a total count would have stayed comfortably large while materializations had
none.

## What is not claimed

**The grammar is not proved correct.** One parser means one interpretation of
the source, not the right one. What changed is that a wrong interpretation is
now a single wrong interpretation, visible to every test rather than to half of
them.

**`pw-syntax` still has two ways to walk a tree** — `SyntaxNode` traversal and
`flat_tree` for the losslessness tests. Those are two *views* of one tree, not
two trees, and the structural test is written to allow that distinction.

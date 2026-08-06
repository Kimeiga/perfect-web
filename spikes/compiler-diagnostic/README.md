# spike: compiler-diagnostic

**Charter reference:** §14 Milestone 0 task 11 — *"build a tiny Rust CLI that
parses one toy declaration and emits a source-span diagnostic. This validates the
development ergonomics before choosing parser libraries."*

## Question

Can a hand-written Rust lexer/parser carry precise byte spans far enough to
render the diagnostic quality the charter demands in §16.3 — naming the rule, the
**origin** of the offending label, the **boundary** that rejected it, the
inferred label, and at least one legal alternative — before Milestone 2 commits
to a parser library?

## Run it

```bash
just spike-compiler-diagnostic
# or directly:
cargo test -p spike-compiler-diagnostic
cargo run  -p spike-compiler-diagnostic -- --demo --plain
cargo run  -p spike-compiler-diagnostic -- --demo --plain --explain
cargo run  -p spike-compiler-diagnostic -- path/to/file.pw
```

Evidence: `docs/evidence/E0/spike-compiler-diagnostic.txt`.

## Scope

One declaration form (`query`), three error-level semantic rules, one
warning-level rule. The grammar is deliberately trivial; the *diagnostic
pipeline* is the artifact under test.

The chosen semantic rule is charter §7.8's `SharedCache<Cart@Session>` — the
smallest honest instance of the project's central claim, and the one for which
§16.3 already specifies the target output.

## Result — answered YES

The target diagnostic is reachable. Rendered from real parsed spans:

```text
error: [PW0100] cannot materialize `cart` in a shared public cache
 --> rejected/session-cart-in-shared-cache.pw:4:5
  |
1 | session query cart(id: SessionId)
  | ------- `cart` is declared here, so its result is labeled `Session<SessionId>`
...
4 |     cache shared
  |     ^^^^^^^^^^^^ this cache is shared across all sessions
  |
  = note: a shared cache may contain only `Public` values; this query's result is `Session<SessionId>`
  = help: change this to `cache private`, or move the value into a private streamed slot
  = help: if the value really is public, declare the query `public`
```

Compare with the charter's §16.3 target — it matches on all five required
elements, and adds the two things the charter's prose version lacks: a stable
error code and the exact source spans.

## Findings

**F-1 — `annotate-snippets` 0.12 is the right renderer; adopt it.**
It is the crate rustc itself uses, it is `MIT OR Apache-2.0` (compatible with
this project's dual license), and it natively supports the primary/context
two-span shape §16.3 requires via `AnnotationKind::Primary` / `::Context`.
`Renderer::plain()` produces byte-stable ANSI-free output, which is exactly what
the Milestone 2 compile-fail snapshot tests need. Recorded as ADR-0009.

**F-2 — a hand-written lexer with `Range<usize>` spans is sufficient and cheap.**
~150 lines gave exact byte spans that survived unchanged into rendered output.
No parser-combinator or generator framework was needed to reach charter-quality
diagnostics. This supports the charter's own §14 M2 guidance to prefer a
hand-written parser and to keep Tree-sitter for editor support only.
It does **not** yet answer the *lossless* tree question (comments/whitespace
retention for `pw fmt`); that remains open for Milestone 2.

**F-3 — error recovery must be designed in from the first line, not retrofitted.**
`recover_to_policy_boundary()` is what makes `two-independent-typos.pw` report
*both* mistakes instead of only the first. The charter's AI-reliability goal
(§1.16, §19) depends on an agent seeing the whole error batch per compile; a
parser that dies on the first token cannot deliver that.

**F-4 — semantic checks must be suppressed when parsing produced errors.**
Discovered by running the spike, not by reasoning about it. On
`missing-paren.pw` the parser recovered by inventing a plausible declaration,
and the semantic pass then emitted `PW0200` about *the recovery*, not about the
user's code. Two diagnostics, one of which was noise pointing at a construct the
user never wrote. Fixed by gating `check()` on an empty parse-diagnostic list.
Milestone 2 needs a general policy here (rustc's "derived error suppression"),
not a one-off.

**F-5 — `pw explain` output must be tested for generated-file leakage, not
merely intended to avoid it.** The Milestone 2 gate requires that the compiler
"print an effect summary without exposing generated-file paths". That is an
assertable property, so `explain_reports_derived_placement_without_naming_generated_files`
asserts the string contains no `koka`/`marko`/`.pw-build`/`generated/`. Cheap
now; load-bearing once real code generation exists.

**F-6 — derived placement is worth printing even in a toy.** `--explain` derives
`Origin` vs `Edge` from visibility + cache partition alone. Seeing the derivation
made it obvious that placement is a *function of the policy annotations already
being written*, which is the §7.9 claim. Nothing extra had to be authored.

## Limitations

- No lossless syntax tree: comments and whitespace are discarded, so this spike
  cannot yet support `pw fmt` idempotence (Milestone 2 gate item).
- No module system, no types, no effects — those are Milestone 2 and 9.
- The `.pw` syntax used here is provisional and not the outcome of ADR-0010's
  eventual grammar decision. Only the *diagnostic pipeline* is being validated.
- Column numbers come from `annotate-snippets`' own byte→display mapping; this
  has not been tested against non-ASCII source or wide characters.

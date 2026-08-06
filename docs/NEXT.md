# Next

The next executable tasks, in order, with acceptance criteria. Charter §3.4.

**E1 is closed** on RQ-2's measured Outcome 1. **E1A is algorithmically
implemented, source integration pending.** **E2 is in progress** and now owns
body parsing, HIR, and source-to-checker integration — see `docs/MILESTONES.md`.

---

## The revised plan

Fixed by the project architect after reviewing E2's results. Items 1–3 are done.

| # | task | status |
|---|---|---|
| 1 | Correct the E7 evidence wording — Marko is a resumption/DOM oracle, not a lazy-loading one | **done** |
| 2 | Adopt Rowan | **done** — green tree, `SyntaxKind`, invariants carried over, 68/68 corpus files round-trip |
| 3 | Write the formatter ADR, do not implement | **done** (ADR-0013) |
| 4 | Implement the durable core body grammar | **done** — 68/68 corpus files parse and round-trip; see `docs/milestones/E2.md` |
| 5 | Lower bodies into HIR | **done** — ADR-0014; id-indexed arenas, span on every node, 11 tests |
| 6 | Move declaration rules from `pw-cli` into `pw-core` | **done** |
| 7 | Connect the tested `pw-core` algorithms to `.pw` source | **partial** — exhaustiveness, the E2A-S scope graph, privacy, placement, markup rules, effect inference and the layout relations all run on source; ABI and capability still unconnected, and the placement solver still reads *declared* effect rows rather than inferred ones |
| 8 | Ratchet semantic corpus enforcement upward from 4/44 | **done — 44/44**, ratcheted in `checking_source.rs`; per-fixture table at `docs/evidence/E2D/corpus-enforcement.txt` via `just evidence-corpus` |
| 9 | Implement `pw fmt` after the syntax/HIR boundary stabilises | **done** — gate in the ADR-0013 amendment; 23 of 68 corpus files reformatted |
| 10 | Begin E2A-R only once body-level task operations can be represented | **unblocked** — `Expr::Keyword` represents them |

### The single biggest architectural decision

> **Do not build an effect-only body parser. Build the real lossless expression
> syntax now, but stage how much semantic meaning is implemented over it.**

Both alternatives were rejected with reasons worth keeping:

- **An effect-only grammar** creates a second mini-language. The real parser
  eventually sees one program and the effect parser currently sees another; it
  can misassociate lambda bodies, call arguments, operator precedence, nested
  blocks or match arms while still producing plausible-looking effect results.
  Generic callbacks are specifically a load-bearing test, so syntactic
  approximation is the wrong layer to economise on.
- **Declarations alone** cannot analyse
  `List.map(items, item => database.read(item.id))`. It would prove only direct
  declared-call propagation — not higher-order source programs, which is the
  exact failure mode the corpus exists to prevent.

### Where item 4 starts

The tree layer is in place and proven, so item 4 is now a contained task:

1. **Port the declaration parser to emit into `TreeBuilder`.** It currently
   builds the hand-rolled AST directly. `checkpoint()` / `start_at()` handle the
   cases where a node's kind is only known after its first token.
2. **Make `ast.rs` typed wrappers over `SyntaxNode`** rather than an independent
   structure — the same accessors, backed by the tree.
3. **Add the expression grammar**, filling the kinds already reserved at 200..
   and the patterns at 300...
4. Keep both losslessness suites green throughout. They are the regression net:
   the token stream and the tree are compared against each other, so a mapping
   bug shows up immediately.

### What "core body grammar" means

Not every final language feature. The durable core:

```text
names and qualified names      literals
member/field access            function calls
lambdas                        blocks
let bindings                   if/else
match and patterns             operators with real precedence
records, tuples, collections   type applications where syntactically relevant
parenthesised expressions      error/recovery nodes
```

Queries, commands, subscriptions, resources and tasks should be declarations or
typed constructs **lowering into the same core expression representation**. E4
extends the tree; it must not replace its foundation.

---

## N-0 — Linux CI (deferred, still open)

The only unmet part of the E0 gate. Risk R11 remains the highest-likelihood open
risk. Deferred by operator decision to before **E3**; it is cheap now and
expensive once generated file paths and asset casing start to matter.

| # | part | state |
|---|---|---|
| 1 | `.github/workflows/ci.yml` on ubuntu-24.04 x64 **and** arm64 | written; YAML parses; **never executed** |
| 2 | `scripts/bootstrap.sh` Linux branches, SHA-256 verified | written; checksums computed from the downloaded artifacts; macOS path re-verified; Linux path not executed |
| 3 | case-collision check | **done** — `just case-check` in `just ci`, with a synthetic collision as its control |
| 4 | `cargo deny` + Node advisory gate | **done** — `just audit`, both clean; one esbuild advisory accepted with a written reason |

**Acceptance, restated honestly:** items 3 and 4 are met. Items 1 and 2 are met
only in the sense that the code exists; neither has run on Linux. Do not record
R11 as retired until a real run is green.

---

## The next executable task

The corpus gate is met at 44/44. What is left is **not** more compile-fail
cases — it is replacing the narrow rules that met those cases with the general
analyses their milestones actually name. `docs/evidence/P0/readiness.txt` lists
each shortcut; they are, in the order a reader would notice them:

1. **E9C — control flow.** `affine.rs` asks whether a `return` sits textually
   between an acquisition and the first release. A release in one branch of an
   `if` with a return in the other passes today. A real CFG replaces the
   `early_return` function and nothing else; the acquire/release model, the
   diagnostic and the controls all stay.

2. **E9 — type inference.** `annotations.rs` fires only where the author wrote
   a type. Dropping `: Option<Store>` from R-008 makes it compile. Inference
   would reach the same three fixtures without the annotation, and the existing
   rules become the reporting layer over it.

3. **Interpolation as an expression.** `check.rs::carried_names` reads `{name}`
   out of a string literal's text, so `{token.value}` is invisible to the
   privacy-sink rule. This is a parser change — lower an interpolation as an
   expression — and it is the smallest of the four.

4. **E7 — the manifest generator.** `resume.rs` decides what a capture *is*
   from the enclosing declaration's parameter list. Generating the manifest
   would settle it from the real value.

**Acceptance for each:** the corpus stays at 44/44 with the three numbers
equal, every existing control stays green, and at least one NEW control is
added that the narrow rule would have passed and the general one catches.
That last clause is the point — replacing a shortcut with a real analysis
should be *visible* in the test suite, or there is no evidence it happened.

### Also open

E4 (5/7) and E5 (3/5) have gate items that need a store demo and the resource
generator. Those are product work, not checker work.

One KNOWN_GAP remains: `PW3011`, which no corpus fixture declares.

---

## E2's last open gate item

Item 2 — *≥40 rejected examples report errors at original `.pw` spans* — stands
at **33 of 44**. It is the only E2 gate item still open and it is not E2's to
close; the remaining eleven belong to E5, E9, E9C, E7 and E6 as listed above.

Items 4 (Koka execution) and 3 (formatting) are closed; 1, 5 and 6 were already.

---

## Standing obligations (every milestone)

Charter §3.1, plus the admissibility rule now in `docs/RISK_QUEUE.md`:

1. Inspect the repository and `docs/STATUS.md` first.
2. **Re-verify upstream versions and APIs against primary sources.** E0 found
   seven wrong assumptions this way.
3. Write or update an ADR **before** a consequential design change.
4. Add failing tests or a reproducible benchmark before implementation.
5. **Give every check a negative control** proving it can go red. Five
   measurement bugs so far say this is not optional.
6. Implement the smallest end-to-end vertical slice.
7. Run all relevant checks.
8. Record measured results, limitations and unexpected findings.
9. Make a focused local commit.
10. Update the milestone gate checklist.
11. Proceed only when the gate passes, or document precisely why it cannot.

## Carried-forward amendments

- **Exhaustiveness compile-fail cases must declare a total effect row.** Koka
  accepts a non-exhaustive match in any function declaring `exn`, so a case
  written without that constraint passes vacuously (E0 finding F-8).
- **Do not claim nominal domain types are enforced by Koka.** Single-field
  `value struct`s are erased at the JS boundary (F-4).
- **A multi-operation effect needs ONE `handler` block.** Chained
  `with fun op(..)` shorthands leave later operations unhandled (F-6).
- **`.kki` and the value representation both need golden fixtures per pinned
  Koka version** (ADR-0011).

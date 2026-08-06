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
| 5 | Lower bodies into HIR | **next** — the single missing link between the corpus and the tested checkers |
| 6 | Move declaration rules from `pw-cli` into `pw-core` | **done** |
| 7 | Connect the tested `pw-core` algorithms to `.pw` source | after 5 |
| 8 | Ratchet semantic corpus enforcement upward from 4/44 | continuous |
| 9 | Implement `pw fmt` after the syntax/HIR boundary stabilises | after 5 |
| 10 | Begin E2A-R only once body-level task operations can be represented | after 5 |

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

1. `.github/workflows/ci.yml`: run `just ci` on `ubuntu-latest` (x64 and arm64).
2. Give `scripts/bootstrap.sh` a Linux branch — the same upstream releases ship
   `koka-v3.2.3-linux-{arm64,x64}.tar.gz` and
   `wasmtime-v47.0.3-{aarch64,x86_64}-linux.tar.xz`. Keep the SHA-256 verification.
3. Add a case-collision check; macOS will not catch it.
4. Add `cargo-deny` (licenses + advisories) and `pnpm audit`. Charter §3.6
   requires both in CI and neither runs.

**Acceptance:** `just ci` green on Linux; a deliberately case-colliding path
fails the build; `cargo-deny check` passes against the licenses recorded in
`docs/research/technology-matrix.md`.

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

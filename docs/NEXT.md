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

Both scores are closed. Neither can go higher, and that is the point — what
is left is not counting.

```text
corpus conformance        44 / 44   closed at C1
single-defect isolation   44 / 44
generality-tested         29 / 29
headline matrices          8 / 8
known narrow witness       0 / 29
robustness                 9 suites, 0 panics
```

### 1. Resumption's third question

Charter §8.5 asks three independent things of a resumable capture, and only
two are checked:

```text
Can this value be serialized?              checked  (its type)
May it cross this privacy boundary?        checked  (its label)
Can it be resumed under THIS CODE VERSION? NOT CHECKED
```

A capture satisfying both of the first two can still be restored into a
handler whose code has changed. `PW3011` is the reserved code, this is the
only remaining `KNOWN_GAP`, and it is the only part of a headline construct
with no analysis at all.

### 2. A coverage-guided fuzzer

`just robustness` reports nine generator families rather than a bare zero,
because this project has the receipt for why the distinction matters:
reverting the constructor-arity fix left corpus-mutation and byte-soup green
while only the generator aimed at that seam went red. Structured generation
found both panics; breadth found neither. A real fuzzer is what retires "NOT
the compiler cannot crash" from `readiness.txt`.

### 3. E4 and E5's remaining gate items

E4 is 5/7 and E5 3/5. Both need a store demo and the resource generator —
product work, and the only items here not about the compiler.

### 4. Claim-by-claim P0 review

Corpus conformance and generality are two of P0's claims. Several others rest
on Marko and Koka behind adapters and several are unstarted;
`docs/EVIDENCE_LEDGER.md` governs which sentence each may support.

### How to add work here

A new invariant needs, before it counts: a corpus fixture (C1 is frozen, so
this opens C2 — see `docs/CORPUS.md`), a challenge witness, a valid
neighbour, and a registry entry with a symbol. `just generality` and
`just ci` will refuse it otherwise, which is the intent.

---

## Standing obligations (every milestone)

Charter §3.1, plus the admissibility rule in `docs/RISK_QUEUE.md`: a
measurement or checker result is not admissible evidence until its instrument
has a negative control proving it can detect the corresponding failure.

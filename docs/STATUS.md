# STATUS

<!-- Charter §3.4 requires exactly these sections. Keep them. -->

**charter version:** **v2** (`PROJECT_CHARTER.md`, 3,206 lines). Adopted
2026-08-05 mid-session; v1 archived at `docs/research/charter-v1-superseded.md`.
See `docs/ASSUMPTIONS.md` A-008.

**numbering:** engineering `E0`–`E15`, public proofs `P0`–`P9`, risk-retirement
experiments `RQ-*`. Never a bare `M`. See `docs/MILESTONES.md`.

**current milestone:** **E7 — the own renderer (E7-R/E7-P/E7-L).** Everything
before it is closed: E0, E1A, E2, E2A, E2B, E2C, E2D, E3, E4, E5, E6 and the
inserted E7V. The three items the architect required before E6 could start are
also closed — coverage-guided fuzzing (`just fuzz`), the compatibility decision
on the real browser handler path, and typed `{#each}` captures
(`just each-typing`).

**E6 is complete** (`docs/milestones/E6.md`): the dependency graph is derived
from declarations and serialized (`pw emit-graph`), three build-time rules
enforce what an edge may be, and `runtime/pw-materialize` consumes committed
events from a transactional outbox. Changing one menu item regenerates 1 of
1000 fragments and leaves 999 untouched — counted from the causality record,
with the argument-less event that regenerates all 1000 as its control.

Implementing it found that **every graph edge in the corpus pointed at
nothing**: a `materialize` block's policies were being parsed as expressions, so
`decl.policies` was empty for every materialization, and seven fixtures named
resources and events no file in their program declared. Corpus **C2** opened.

**next milestone:** **E7's own renderer** (E7-R/E7-P/E7-L), then E8's Wasm
capability host and E9's permanent type checker. Those are the large ones and
none is started; `docs/MILESTONES.md` has the register.

Three of the four shortcuts `readiness.txt` named this morning are closed:
branch-aware affine analysis, Option inference that does not need the
annotation, and string holes lowered as expressions. Each replacement is proved
by a program the narrow rule would have passed.

E0, E1A, E2, E2A, E2B, E2C, E2D, E3, E4, E5, E6 and E7V are complete. E1 closed
on RQ-2's Outcome 1. E7 onward are not started.

**risk-retirement queue** (`docs/RISK_QUEUE.md`):

| | | |
|---|---|---|
| RQ-1 | Marko resumption, Chrome + Safari | **done** — passed its pre-registered rule |
| RQ-2 | Koka higher-order effect propagation | **done** — Outcome 1, clean pass |
| RQ-3 | `pw` exhaustiveness + typed ABI | **partial** — checker done; no parser until E2 |
| RQ-4 | structured concurrency | **done** — E2A-S runs on source; E2A-R is `runtime/pw-tasks` |
| RQ-5 | artifact capability audit | **partial** — rule done; not wired to a build |
| RQ-6 | effect-family rejection coverage | **partial** — layout/DOM families done |
| RQ-7 | E→P evidence ledger | **done** |

**E2 has landed a front end.** `just ci` now runs `pw check` over all 68 corpus
files and they all parse, with precise spans and error recovery. The
specification is executable in CI rather than merely well-formed.

**Corpus enforcement, as three numbers** (architect ruling, 2026-08-06 — a
single figure hides the difference between a red diagnostic, the *right* red
diagnostic, and the whole declared invariant being checked):

```text
46 / 46  rejected fixtures produce a compile error
46 / 46  emit their declared canonical code
46 / 46  fully enforce the complete declared invariant
```

**Charter §14 M2 gate item 2 — ≥40 of 44 — is met**, at corpus version **C3**
(`docs/CORPUS.md`), with the three numbers equal and no wrong-reason catches.
The floor is now the directory rather than the constant `44`: a corpus that
grows past a hardcoded floor leaves its new fixtures unenforced while the
assertion still passes.

There is now a **second gate, and it is open**:

```text
corpus conformance        46 / 46   at C3
single-defect isolation   46 / 46
generality-tested         30 / 31   1 known narrow
headline matrices          9 / 9
resume compatibility      E7V closed — 34 matrix rows, 6 fuzz targets
robustness                11 suites, 0 panics, 3 regressions retained
coverage-guided fuzzing    6 targets, 3600 execs, 0 findings
resource graph            E6 closed — 6 gate items, 24 materializer tests
historical compatibility   9 / 10   the miss classified
KNOWN_GAPs                 0
```

Fuzzing is reported on its own line and never folded into the robustness one.
Structured generation and coverage feedback fail in different directions, and a
single "fuzzed" figure would let one cover for the other.

An invariant counts as *generally* enforced only when a program its fixture did
not anticipate is caught. `examples/generality/` holds those programs — and
also `slips-through.pw` files, which are known gaps written as code that must
compile clean, so `just ci` fails the moment a gap closes and the witness needs
promoting. `just generality` scores it.

Read the second number as the honest one. It moved 7 → 30 today; the first
moves only when the corpus does.

**30 / 31, not 31 / 31.** `private_in_shared_materialization` has two executable
known gaps — a private value reaching a shared fragment through a public query's
body, by a helper and by a branch — so it is *known narrow* rather than
generality-tested. Both are programs in
`examples/generality/private_in_shared_materialization/`, and `just generality`
fails the moment either stops compiling clean.

Up from four when E2 began, each with a primary span, an origin span, a note and
a legal alternative. `just evidence-corpus` regenerates
`docs/evidence/E2D/corpus-enforcement.txt`, the per-fixture table — written by
the same test the ratchet asserts on, so the published number cannot drift from
the enforced one.

The three numbers have stayed equal at every step, which is the point of
reporting three. Twice they diverged and both times it was treated as a
regression rather than banked: once when a count rose to 27 by reporting two
fixtures for effects they had declared, and once when `R-012` was caught for a
second defect that masked its declared invariant.

Zero false positives across the 24 accepted files, four of which (`A-018`,
`A-020`, `A-021`, `A-023`) exist specifically as negative controls for the
layout rules — each differs from its rejected twin in exactly the one way the
rule is about. Coverage is ratcheted by a test so it cannot silently regress, and every rule
has a negative control in `examples/rules/**` that differs from its rejected
twin in exactly the one way the rule is about.

**The front end is complete through HIR.** Rowan is adopted (ADR-0012); the
body grammar parses all 68 corpus files and they round-trip byte-for-byte;
lowering (ADR-0014) turns them into id-indexed arenas with a span on every node,
and `pw-core`'s checkers consume those ids without ever seeing a syntax node —
enforced by a test, not by convention.

**`pw fmt` ships** (ADR-0013 amendment). Idempotent, token- and
comment-preserving on all 68 files, `--check` wired into `just ci`. It is a
canonical *spacing*, not yet canonical line breaking, and the ADR says so.

**E2A is complete — both halves.** The static checker rejects `.pw` programs
(R-013, R-039) and `runtime/pw-tasks` (ADR-0016) proves the same semantics
behave as specified under real concurrency: cancellation propagation, ordered
cleanup, refusal to commit into a dead scope, and no leaked tasks. 12 tests,
0 failures in 25 consecutive runs. They are **behaviour** results and RQ-4's
rule holds — neither half may be described as covering the other.

**`pw` renders, streams and resumes in a browser. E3 is complete.** Three
applications written in `.pw` generate Marko (ADR-0017), build, and pass 30
tests across Chromium, Firefox and WebKit. The static and streamed routes ship
**zero JavaScript** — no script tag, no request; the streamed page's shell was
usable **1,201 ms before** its 1,200 ms region arrived, in two chunks; the
counter resumes without re-rendering the inert part of the page. Those are
Marko's behaviours measured through `pw`, and the evidence ledger says so.

**`pw` code executes.** `pw emit-koka` lowers the pure subset (ADR-0015) and
`just spike-pw-to-koka` compiles and runs it under the pinned Koka 3.2.3,
matching hand-computed values. Two negative controls run with it, including one
proving the generated `total` annotation is load-bearing rather than
decorative. **E2 gate: five of six items pass.** The open one is the
rejected-corpus count, which E1 and E5 own.

**public claims:** governed by `docs/EVIDENCE_LEDGER.md`. The **corpus** gate is
met at 44/44, but P0 as a whole still cannot be published: it has other claims,
several unstarted, and several resting on Marko and Koka behind adapters rather
than on `pw`'s own implementation. `docs/evidence/P0/readiness.txt` states what
may and may not be said today, and the second list is the longer one.

**last passing commit:** `7630593` — Linux CI and supply-chain scanning.
`just ci` passes at that commit on macOS 26.5.2 / arm64, with 207 tests.
`just audit` is clean; `just spike-pw-to-koka` executes generated Koka.

---

## completed gate items

All seven charter §14 M0 gate items:

1. **`just doctor` works on the Mac** — exits 0, read-only, warns on the 16 GiB host deviation.
2. **All six spikes run from documented commands** — `just spikes`, six evidence files in `docs/evidence/E0/`. Charter v2 allows recording a blocker instead; none was needed.
3. **Versions and licenses pinned** — `tools/versions.lock`, `rust-toolchain.toml`, `pnpm-lock.yaml`, SHA-256-verified release tarballs, license column in the technology matrix.
4. **≥10 accepted / ≥20 rejected examples** — now **24 and 44**, covering **24/24** and **44/44** charter §16 categories including v2's layout/DOM families. Enforced by `tools/corpus-check` in `just ci`.
5. **Reuse/fork/tape/build matrix complete** — `docs/research/technology-matrix.md`, with *measured* vs *read* clearly distinguished.
6. **Known failures documented honestly** — `docs/KNOWN_LIMITATIONS.md`.
7. **One clean `just ci`** — passes on macOS.

---

## failing gate items

**None.** The E0 gate is fully closed as of 2026-08-06.

- **None.** Gate item 7 closed on 2026-08-06: `.github/workflows/ci.yml` ran on
  `ubuntu-24.04` and `ubuntu-24.04-arm` and passed on the first attempt, along
  with the supply-chain job. Charter §13.5 case checking and §3.6 license and
  vulnerability scanning both run in CI. **Risk R11 is retired**; evidence in
  `docs/evidence/E0/linux-ci.txt`.

---

## exact commands to reproduce

```bash
just doctor        # read-only environment check; exits 0 when E0 tools are present
just bootstrap     # fetch pinned Koka 3.2.3 + Wasmtime 47.0.3 into .toolchain/, pnpm install
just ci            # fmt-check + clippy -D warnings + 18 unit tests + corpus check  -> "ci: OK"
just spikes        # all six spikes; rewrites docs/evidence/E0/*.txt

# individually
just spike-compiler-diagnostic
just spike-koka
just spike-wasmtime
just spike-marko
just spike-layout    # charter v2 §7.5A forced-layout instrumentation
just spike-bonsai    # charter v2 Incremental/Bonsai study (needs opam switch pw-bonsai)

just env-record    # regenerate docs/environment/macbook.md + tools/versions.lock
```

First run on a clean machine: `just bootstrap` then `just doctor` then `just ci`.

---

## known environmental issues

- **Host is an M2 Pro / 16 GiB**, not the M3 Max / 64 GB the charter assumes
  (`docs/ASSUMPTIONS.md` A-001). Milestone 11's VM table allocates 14 GB of
  guests and does not fit; revised sizing is recorded. **All performance numbers
  are incomparable to figures from an M3 Max.**
- **Node is v22.21.1** (maintenance LTS) rather than v24 (active LTS). Satisfies
  every declared engine range (A-002).
- **WASI 0.3 is unreachable** through Rust's stable `wasm32-wasip2`; imports
  resolve at `@0.2.9` (ADR-0008).
- **Koka's `.kki` format is internal and unstable.** `node/kki.mjs` pins
  version 3.2.3 and throws on any other.
- **`just bootstrap` is macOS/arm64 only** — it refuses elsewhere with a clear
  message rather than doing something wrong.

---

## last benchmark summary

**No benchmarks yet.** Benchmarking begins in Milestone 3 (charter §14 M3 task 8);
baselines against Next/React, SvelteKit and Marko are §18.1.

The Milestone 0 spike measurements below are **single runs on one machine over
localhost** — spike evidence, not benchmark results (charter §18.5 requires
sample counts and distributions):

```text
marko /static         588 B html, 0 script tags, 0 downloaded JS
marko /stream         shell 3.1 ms | 400 ms subtree at 408 ms | 1200 ms at 1206 ms
marko resumption      HTML 9.6x larger -> route-specific client JS 1.03x
wasm component        no_std 5,276 B (1 import) vs std 43,837 B (15 imports)
layout thrash/phased  79.1 -> 0.3 ms at n=400; 678.6 -> 0.8 ms at n=1200 (848x)
containment           3,000-row subtree build+layout 23.8 -> 5.7 ms (4.18x)
incremental           5 unrelated updates -> expensive node evaluated once
```

---

## next three concrete tasks

1. **Parse function and template bodies.** This unlocks the largest block of
   the rejected corpus — the effect-in-view family (`R-001`, `R-033`, `R-036`,
   `R-037`) and body-level policy violations (`R-004`) — and E1's lowering needs
   it anyway. `R-037` matters most: an effect smuggled through a generic
   callback. RQ-2 proved the backend propagates effects that way; the `pw`
   checker must be shown to do the same, or it is worth approximately nothing.
2. **E2A-R** — the runtime half of structured concurrency: owner scopes,
   cancellation propagation, cleanup ordering, leak detection. E2A does not close
   until both halves pass, and the static rules must not be described as covering
   the runtime ones.
3. **Promote `spikes/koka-js-interop/node/kki.mjs` to `tools/kki-effects/`**
   with golden fixtures per pinned Koka version — for the `.kki` format *and*,
   per ADR-0011, for the value representation the decoder depends on.

Linux CI is deferred by operator decision to before E3 (risk R11).

Full ordered list with acceptance criteria: `docs/NEXT.md`.

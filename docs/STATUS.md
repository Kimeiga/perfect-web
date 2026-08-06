# STATUS

<!-- Charter §3.4 requires exactly these sections. Keep them. -->

**charter version:** **v2** (`PROJECT_CHARTER.md`, 3,206 lines). Adopted
2026-08-05 mid-session; v1 archived at `docs/research/charter-v1-superseded.md`.
See `docs/ASSUMPTIONS.md` A-008.

**numbering:** engineering `E0`–`E15`, public proofs `P0`–`P9`, risk-retirement
experiments `RQ-*`. Never a bare `M`. See `docs/MILESTONES.md`.

**current milestone:** **E0 — COMPLETE.** Gate passed against charter v2
(six spikes) on 2026-08-05, with one documented shortfall: Linux CI.
Full assessment: `docs/milestones/M0.md` (read `M0` there as `E0`).

**next milestone:** **E2 — source language front end. In progress.** Four of its
six gate items pass. E1 is closed on RQ-2's measured Outcome 1; E1A is
algorithmically implemented and now partly reachable from source. The two open
E2 gate items are lowering to Koka (item 4) and the rejected-corpus count
(item 2), which E1 and E5 own.

**risk-retirement queue** (`docs/RISK_QUEUE.md`):

| | | |
|---|---|---|
| RQ-1 | Marko resumption, Chrome + Safari | **done** — passed its pre-registered rule |
| RQ-2 | Koka higher-order effect propagation | **done** — Outcome 1, clean pass |
| RQ-3 | `pw` exhaustiveness + typed ABI | **partial** — checker done; no parser until E2 |
| RQ-4 | structured concurrency | **partial** — E2A-S done **and running on source**; E2A-R not started |
| RQ-5 | artifact capability audit | **partial** — rule done; not wired to a build |
| RQ-6 | effect-family rejection coverage | **partial** — layout/DOM families done |
| RQ-7 | E→P evidence ledger | **done** |

**E2 has landed a front end.** `just ci` now runs `pw check` over all 68 corpus
files and they all parse, with precise spans and error recovery. The
specification is executable in CI rather than merely well-formed.

**Seven rejected corpus files now fail to compile**, each caught by the rule its
`@rule` header declares, with a primary span, an origin span, a note and a legal
alternative. `just rejections` shows the current state. Four are declaration
rules; **three are caught by algorithms running on HIR lowered from source** —
R-007 by the exhaustiveness checker, which names every missing variant, and
R-013 and R-039 by the E2A-S scope graph.

The other 37 need effect checking (E1) or privacy/placement (E5). Coverage is
ratcheted by a test so it cannot silently regress.

**The front end is complete through HIR.** Rowan is adopted (ADR-0012); the
body grammar parses all 68 corpus files and they round-trip byte-for-byte;
lowering (ADR-0014) turns them into id-indexed arenas with a span on every node,
and `pw-core`'s checkers consume those ids without ever seeing a syntax node —
enforced by a test, not by convention.

**`pw fmt` ships** (ADR-0013 amendment). Idempotent, token- and
comment-preserving on all 68 files, `--check` wired into `just ci`. It is a
canonical *spacing*, not yet canonical line breaking, and the ADR says so.

**`pw` code executes.** `pw emit-koka` lowers the pure subset (ADR-0015) and
`just spike-pw-to-koka` compiles and runs it under the pinned Koka 3.2.3,
matching hand-computed values. Two negative controls run with it, including one
proving the generated `total` annotation is load-bearing rather than
decorative. **E2 gate: five of six items pass.** The open one is the
rejected-corpus count, which E1 and E5 own.

**public claims:** governed by `docs/EVIDENCE_LEDGER.md`. P0 still cannot be
published — several claims it needs are unstarted, and of 44 rejected corpus
files **5 are rejected by the compiler**, not the ≥40 the charter asks for.

**last passing commit:** `7630593` — Linux CI and supply-chain scanning.
`just ci` passes at that commit on macOS 26.5.2 / arm64, with 207 tests.
`just audit` is clean; `just spike-pw-to-koka` executes generated Koka.

---

## completed gate items

All seven charter §14 M0 gate items:

1. **`just doctor` works on the Mac** — exits 0, read-only, warns on the 16 GiB host deviation.
2. **All six spikes run from documented commands** — `just spikes`, six evidence files in `docs/evidence/M0/`. Charter v2 allows recording a blocker instead; none was needed.
3. **Versions and licenses pinned** — `tools/versions.lock`, `rust-toolchain.toml`, `pnpm-lock.yaml`, SHA-256-verified release tarballs, license column in the technology matrix.
4. **≥10 accepted / ≥20 rejected examples** — now **24 and 44**, covering **24/24** and **44/44** charter §16 categories including v2's layout/DOM families. Enforced by `tools/corpus-check` in `just ci`.
5. **Reuse/fork/tape/build matrix complete** — `docs/research/technology-matrix.md`, with *measured* vs *read* clearly distinguished.
6. **Known failures documented honestly** — `docs/KNOWN_LIMITATIONS.md`.
7. **One clean `just ci`** — passes on macOS.

---

## failing gate items

**None outstanding.** One partial:

- **Gate item 7 is still macOS-only, but no longer unaddressed.**
  `.github/workflows/ci.yml` now runs `just ci` on ubuntu-24.04 x64 and arm64,
  `scripts/bootstrap.sh` has Linux branches with checksums computed from the
  downloaded artifacts, `just case-check` guards charter §13.5, and `just audit`
  runs `cargo deny` plus a Node advisory gate for §3.6 — all four green locally.
  **The workflow has never executed**: pushing needs authorization this session
  does not have. Risk R11 is *reduced, not retired*; see
  `docs/KNOWN_LIMITATIONS.md` for which rows are verified and which are only
  written.

---

## exact commands to reproduce

```bash
just doctor        # read-only environment check; exits 0 when M0 tools are present
just bootstrap     # fetch pinned Koka 3.2.3 + Wasmtime 47.0.3 into .toolchain/, pnpm install
just ci            # fmt-check + clippy -D warnings + 18 unit tests + corpus check  -> "ci: OK"
just spikes        # all six spikes; rewrites docs/evidence/M0/*.txt

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

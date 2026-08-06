# Known limitations

Charter §3.3: *"Never hide uncertainty. When an upstream project is experimental,
an API changed, or an assumption failed, say so in the repository documentation.
Do not invent APIs or pretend a compatibility layer is permanent."*

Everything here is a thing that does **not** work, is **not** proven, or is
proven **not** to work. As of end of Milestone 0.

---

## The big one

**There is no compiler.** No `.pw` file compiles. The 45 files in `examples/` are
specification documents; `tools/corpus-check` validates their *shape and
coverage*, not their meaning. Every semantic claim in `docs/SEMANTICS.md` marked
"specified" is currently enforced by nothing but the corpus text.

---

## Proven not to work as hoped

These were tested and the answer was no. Each has an architectural consequence.

**Koka does not enforce exhaustiveness as an independent rule.**
A non-exhaustive match compiles cleanly when the function declares `exn`, and
fails at runtime with `pattern match failure`. It is rejected only in functions
whose effect row excludes `exn`.
→ `pw` must implement exhaustiveness itself (Milestone 9A). Milestone 1's
compile-fail cases must use **total** effect rows or they pass vacuously.
Evidence: `docs/evidence/M0/spike-koka-js-interop.txt` §2.

**Koka erases single-field value structs.**
`Money_usd(350)` returns `350`; `Store_id("store_47")` returns `"store_47"`.
Nominal typing exists only inside Koka's checker.
→ Charter §7.1's "nominal opaque domain types" and `Money<USD>` get **zero**
runtime protection at the JS boundary. The `pw` compiler must carry nominal
identity in its own manifest.

**`Nothing` and `Nil` are both `null` in Koka's JS output.**
`is_nothing(Nil)` and `is_nil(Nothing)` are both `true`. A bare `null` at the
boundary is genuinely ambiguous.
→ Charter §7.1's "no ambient `null`" holds inside Koka but not at the seam.
Decoders must be told the static type; they cannot discover it.

**Koka's generated `is_*` predicates are not validators.**
`is_just(42) === true`; `is_qok(null)` throws a `TypeError`.
→ Decoders must confirm structural shape before using any predicate.

**A `std` Rust guest carries ambient WASI authority.**
Fifteen imported WASI instances for a WIT world declaring one: environment, exit,
stdio, terminal, clocks, io.
→ Generated components must be `no_std`-equivalent, and the build must assert the
import list against the declared world rather than trusting it.

**WASI 0.3 is not reachable through the stable Rust toolchain.**
`wasm32-wasip2` emits imports at `@0.2.9`.
→ ADR-0008. Consequence: WASI 0.3's async story is unavailable, which matters for
charter §7.6 at the component boundary.

**`forcedStyleAndLayoutDuration` is not exposed by Chrome 150.**
The `long-animation-frame` entry type works and reports `duration` and
`blockingDuration`, but the forced-layout-specific field the charter names
(§14 M0 task 13) is `undefined`.
→ Layout instrumentation uses long-frame **count** plus `blockingDuration` as the
proxy. The harness probes for the field and will use it automatically if a later
Chrome exposes it.

**CSS containment can be slower where it does not belong.**
`contain: layout style paint` + `content-visibility: auto` made building and
laying out a 3,000-row subtree **4.18× faster** — but made a forced layout after
an unrelated mutation on the host element marginally **slower**.
→ Charter §7.5A's "only when subtree independence is semantically valid" is a
performance constraint as well as a correctness one. The compiler must not emit
`contain` speculatively.

**Bonsai's incrementality stops at the virtual DOM.**
`virtual_dom/node.mli` exposes `Patch.create ~previous ~current`, which compares
two complete trees — exactly what charter §8.4 forbids.
→ Confirms the charter's own warning not to assume Bonsai's vdom loop provides
what this project needs. Borrow the DAG/cutoff/stabilize semantics, not the
renderer.

**"Zero JS" is exactly true only for a fully static route.**
`/static` emits zero `<script>` tags. The **streamed** route downloads nothing but
carries **849 bytes of inline script** to apply out-of-order patches.
→ Not a defect — it is the same mechanism charter §8.6 proposes building — but it
must be counted, and Milestone 3's budgets must state downloaded and inline bytes
separately.

---

## Not attempted at all

No evidence exists in either direction. Do not assume these work.

- **Structured concurrency (charter §7.6).** The Koka spike tested no async,
  no cancellation, no task scopes. This is the largest untested area of the model.
- **Affine resources / linearity (§7.7).** Charter §14 M1 task 7 explicitly warns
  against claiming Koka statically proves linear usage. We have not checked.
- **Row-polymorphic effects (§7.2).** Whether generic library functions preserve
  callback effects is untested.
- **Koka → Wasm Component.** The wasmtime spike's guest is Rust. The
  Koka-to-component path is completely unvalidated.
- **Anything in a browser except headless Chromium.** No Playwright, no Safari,
  no Firefox, no JS-disabled test, no accessibility audit, no INP/LCP
  measurement. The Marko spike's interactivity claim rests on payload sizes and
  the emitted resume manifest — **nobody has clicked the button.** The layout
  spike used headless Chrome only; **Safari's layout behaviour is untested**, and
  Safari has no Long Animation Frame API at all.
- **Layout-phase semantics in the corpus.** Charter v2 §7.5A adds eight effect
  families and a prohibition on direct geometry reads, but `examples/` contains
  **no** accepted or rejected example for any of them. Charter §16 requires
  examples before features; this is now the largest corpus gap.
- **A Bonsai web application.** The library installs and its dependency graph and
  rendering API were read from source, but no `Bonsai_web` app was compiled to
  JavaScript and run. Incremental behaviour was demonstrated with Incremental
  directly. Bonsai's lifecycle/scoping model and expect-test workflow were read,
  not exercised.
- **SQLite, the outbox, materialization, the resource runtime.** Milestones 4–6.
- **Multi-node, network shaping, HTTP/3.** Milestones 11–12.
- **Semantic diff, LSP, the AI benchmark.** Milestone 14.

---

## Infrastructure gaps

**Linux CI is not wired up.** `.github/workflows/` exists but is empty.
Charter §13.5 and §20 require Linux CI to catch macOS case-insensitivity
assumptions, and §3.6 requires license and vulnerability checks in CI. Neither
runs. `just ci` is macOS-local only. **This is the most concrete Milestone 0
shortfall** — recorded in the M0 gate assessment as a partial pass.

**`just bootstrap` is macOS/arm64 only.** It refuses to run elsewhere with a
clear message rather than silently doing the wrong thing, but there is no Linux
path yet.

**No automated license/vulnerability scanning.** `cargo-deny` / `pnpm audit` are
not configured. Licenses are recorded manually in the technology matrix.

**`just` recipes for later milestones fail loudly by design.**
`test-integration`, `test-e2e`, `test-security`, `bench`, `demo`, `lab-*` print
which milestone will provide them and exit 1. This is deliberate — a silently
passing stub is worse — but it means `just test` is not yet the full pyramid.

---

## Host deviations

**The machine is not the one the charter assumes.** M2 Pro / 16 GiB, not M3 Max /
64 GB. Charter §14 M11's VM table allocates 14 GB of guests, which does not fit.
Revised sizing is in `docs/ASSUMPTIONS.md` A-001, and Stage 11A (native processes,
no VMs) becomes the primary functional path.
**All benchmark numbers are incomparable to any figure produced on an M3 Max.**

**Node is 22.21.1 (maintenance LTS), not 24 (active LTS).** Satisfies every
declared engine range. Assumption A-002.

---

## Measurement caveats

- Every number in `docs/evidence/M0/` is a **single run on one machine over
  localhost**, with no network shaping and no sample distribution. Charter §18.5
  requires sample counts, medians and distributions before anything is published
  as a benchmark. These are spike evidence, not benchmark results.
- `docs/BENCHMARKS.md` contains no benchmarks. Benchmarking starts at Milestone 3.
- The `no_std` component's bump allocator never frees. Fine for one short call;
  it would be wrong under sustained load.
- The wasmtime spike configures a memory limit but only exercises **fuel**.
  Epoch interruption is untested.
- Column numbers in diagnostics come from `annotate-snippets`' byte→display
  mapping, untested against non-ASCII or wide characters.

---

## Documentation caveats

`docs/research/technology-matrix.md` marks each row **measured** or *read*. Only
seven rows were actually exercised here. The rest come from primary documentation
and must not be cited as if we had run them.

## Linux CI exists but has never run (2026-08-06)

`.github/workflows/ci.yml` is written and its YAML parses, but **no push has
happened**, so it has never executed. Charter §3.7 forbids pushing without
explicit human authorization, and that authorization has not been given.

"The workflow exists" is not "CI is green on Linux". Risk R11 — that something
platform-specific has gone unnoticed on a macOS-only history — is **reduced but
not retired**. What has actually been verified locally, on macOS:

| part | verified |
|---|---|
| `scripts/bootstrap.sh` Linux branch | **partially** — the four Linux artifact URLs resolve and their SHA-256s were computed from the downloaded files; the macOS path still works after the refactor. The Linux path itself has not been executed. |
| case-collision check | **yes** — `just case-check` passes on 236 tracked paths, and a synthetic colliding pair is rejected. macOS cannot create a real collision, which is why the check reads the git index rather than the filesystem. |
| `cargo deny check` | **yes** — advisories, bans, licenses and sources all clean |
| Node advisory gate | **yes** — `scripts/audit-node.sh` passes, and rejects a synthetic unlisted advisory |

The first push is what converts this row from "written" to "measured".

## An esbuild advisory is accepted, not fixed (2026-08-06)

`GHSA-g7r4-m6w7-qqqr` (low) affects esbuild `<0.28.1`. It cannot be fixed
without moving the `@marko/run@0.11.8` pin that E0's measurements depend on:
the chain is `@marko/run` → `esbuild-plugin-browserslist@2.0.0`, whose peer
range `~0.27.0` does not admit 0.28.x. A pnpm override was attempted and does
not take, for that reason.

The exposure is esbuild's development server. Nothing here runs `vite dev` —
the spike builds and serves the built output — and no production artifact
contains esbuild. Full reasoning and the revisit condition are in
`tools/node-audit-allow.txt`, which is the file `just audit` reads; anything
not listed there fails the build.

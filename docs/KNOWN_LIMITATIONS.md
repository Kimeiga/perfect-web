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
- **Anything in a browser.** No Playwright, no Safari, no JS-disabled test, no
  accessibility audit, no INP/LCP measurement. The Marko spike's interactivity
  claim rests on payload sizes and the emitted resume manifest — **nobody has
  clicked the button.**
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

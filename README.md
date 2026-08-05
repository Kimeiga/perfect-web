# perfect-web

A clean-slate research platform for web applications that tries to make as much
invalid application behavior as practical **impossible to express or impossible
to compile** — one language with algebraic data types, typed effects and
capabilities, compiler-known placement across build/browser/edge/origin,
privacy-aware data flow, and streaming resumable rendering without whole-tree
hydration.

The governing principle:

> Application developers declare domain meaning, permissions, freshness,
> consistency, state transitions, and UI. The compiler and platform own
> placement, scheduling, rendering, caching, serialization, invalidation,
> synchronization, cleanup, and low-level optimization.

**[`PROJECT_CHARTER.md`](PROJECT_CHARTER.md) is the authoritative constitution.**
Read it before changing anything. [`AGENTS.md`](AGENTS.md) summarizes the
operating rules.

---

## Status: Milestone 0 complete

**There is no compiler yet.** Milestone 0 deliberately builds none. Its job was
to find out whether the integration boundaries the plan depends on actually
exist, *before* committing an architecture to them.

Current state: **[`docs/STATUS.md`](docs/STATUS.md)** ·
Gate assessment: **[`docs/milestones/M0.md`](docs/milestones/M0.md)** ·
What does not work: **[`docs/KNOWN_LIMITATIONS.md`](docs/KNOWN_LIMITATIONS.md)**

### What Milestone 0 produced

- **45 corpus files** (14 accepted, 31 rejected) covering **every** category in
  charter §16 — an executable specification written before the implementation.
- **Four feasibility spikes**, each with measured evidence in `docs/evidence/M0/`.
- **10 ADRs**, pinned toolchains, and a technology matrix that distinguishes what
  was *measured here* from what was only *read*.

### What the spikes measured

| spike | headline result |
|---|---|
| `koka-js-interop` | Inferred **effect rows are machine-readable without a fork**, from Koka's `.kki` file: `calculate-subtotal !{}` vs `load-store !{database-read, trace-effect}`. |
| `marko-stream-resume` | Static route: **588 B HTML, zero `<script>` tags, zero JS.** Streaming is real and out-of-order. **9.6× more HTML → 1.03× the client JS.** |
| `wasmtime-component` | Withholding a capability **refuses instantiation** with a diagnostic naming it. A `no_std` component imports exactly what its WIT world declares; a `std` one imports **15**. |
| `compiler-diagnostic` | Charter §16.3's target diagnostic reproduced from real spans — naming the rule, the label's **origin**, the **boundary**, and legal alternatives. |

### Four assumptions that turned out to be wrong

Recorded because they change later milestones:

1. **Koka does not enforce exhaustiveness** as an independent rule — only for
   functions whose effect row excludes `exn`. `pw` must own it.
2. **Koka erases single-field value structs** — `Money_usd(350)` *is* `350`, so
   nominal domain types get zero protection at the JS boundary.
3. **`Nothing` and `Nil` are both `null`** and runtime-indistinguishable.
4. **WASI 0.3 is not reachable** through Rust's stable toolchain; imports resolve
   at `@0.2.9`.

---

## Getting started

Requires macOS on Apple Silicon (Linux support is the next task — see
`docs/NEXT.md`).

```bash
just doctor        # read-only: what's present, what's missing, how to get it
just bootstrap     # fetch pinned Koka 3.2.3 + Wasmtime 47.0.3, pnpm install
just ci            # fmt + clippy -D warnings + unit tests + corpus check
just spikes        # run all four spikes, rewriting docs/evidence/M0/
```

`just doctor` never installs anything and never modifies the system.
`just bootstrap` writes only into `.toolchain/` and `node_modules/`; deleting
those fully reverses it.

## Layout

```text
PROJECT_CHARTER.md   the constitution
AGENTS.md CLAUDE.md  operating rules
docs/                STATUS, NEXT, DECISIONS, ASSUMPTIONS, KNOWN_LIMITATIONS,
                     RISK_REGISTER, ARCHITECTURE, SEMANTICS, BENCHMARKS,
                     DECISIONS/, milestones/, research/, evidence/, vision/
examples/            the accepted/rejected specification corpus
spikes/              four Milestone 0 feasibility spikes
tools/               corpus-check, versions.lock
scripts/             doctor, bootstrap, record-environment
```

`compiler/`, `runtime/`, `stdlib/`, `benchmarks/` and `lab/` are created when a
milestone needs them — charter §12: *"Do not create empty directories merely to
look complete."*

## License

Dual licensed under [MIT](LICENSE-MIT) or [Apache-2.0](LICENSE-APACHE), at your
option.

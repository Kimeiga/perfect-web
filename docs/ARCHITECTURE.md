# Architecture

What exists today, what it will become, and which parts are deliberately
temporary. Charter §6 defines the target; this file records the current state and
the path.

**Status: Milestone 0.** No compiler exists yet. What exists is four validated
integration boundaries, an executable specification corpus, and the toolchain
pins that make them reproducible.

---

## Today (end of Milestone 0)

```text
examples/{accepted,rejected}/*.pw        45 specification files (charter §16)
        │                                 no compiler reads them yet
        ▼
tools/corpus-check                        validates shape + full §16 coverage
                                          (runs in `just ci`)

spikes/                                   four validated boundaries
├── compiler-diagnostic   Rust   parser -> spans -> multi-span diagnostic
├── koka-js-interop       Koka   ADTs/effects -> .mjs -> Node, + .kki effect rows
├── marko-stream-resume   Marko  streaming SSR + resumption, byte-measured
└── wasmtime-component    Rust   WIT world -> component -> capability enforcement
```

Nothing above is connected to anything else. That is the point of Milestone 0:
prove each boundary exists *before* committing an architecture to it.

---

## Target for Milestones 1–8 (the "taped" architecture)

Charter §6, first diagram. Temporary layers are marked.

```text
Application source (.pw)
        │
        ▼
Custom compiler front end (Rust)              ADR-0003
  ├── parser + lossless syntax tree           M2
  ├── names + modules                         M2
  ├── domain/value type declarations          M2
  ├── effect/capability declarations          M4
  ├── privacy labels                          M5
  ├── placement constraints                   M5
  ├── query/command/subscription graph        M4
  ├── HTML/template graph                     M3
  └── CSS/style metadata                      M3
        │
        ├── generated Koka modules        ── TEMPORARY (ADR-0001, delete at M9)
        ├── generated Marko templates     ── TEMPORARY (ADR-0002, delete at M7)
        ├── generated JS bridge           ── TEMPORARY
        ├── resource manifests                M4
        ├── semantic reports                  M2 (`pw explain`)
        └── source maps                       M2
                  │
                  ▼
       Node/Marko development host        ── TEMPORARY (charter §10.1)
                  │
                  ▼
       Rust capability host + Wasmtime        ADR-0006, M8
          ├── database interfaces (SQLite)    ADR-0005
          ├── secret/session interfaces
          ├── cache/materialization           M6
          ├── tracing
          └── WIT-defined capabilities        ADR-0008 (WASI 0.2)
```

### Every temporary layer has a deletion condition

Charter §4 requires this. Nothing is allowed to become permanent by inertia.

| layer | replaced by | deletion condition | ADR |
|---|---|---|---|
| generated Koka | own type/effect checker | M9 accepts the accepted corpus, rejects the rejected corpus, differential agreement on the shared subset | 0001 |
| generated Marko | own document-parts renderer | M7 passes the renderer-independent golden suite | 0002 |
| JS DOM shim | own parts runtime, then possibly native primitives | M7; native only after M13 profiling | — |
| Node host | Rust + Wasmtime host | M8: Node no longer owns privileged business I/O | 0006 |
| SQLite | PostgreSQL (alongside, not instead) | M11 multi-node topology | 0005 |
| WASI 0.2 | WASI 0.3 | Rust ships a stable `wasm32-wasip3` and the four capability checks still pass | 0008 |

---

## Target after Milestones 9–13 (the "own" architecture)

```text
Application source (.pw)
        │
        ▼
Own compiler + semantic IR                    M9, M10
        ├── HTML + typed CSS + parts manifest M7
        ├── browser JS or Wasm handlers       M10
        ├── edge/origin Wasm Components       M8, M10
        ├── resource dependency graph         M4, M6
        ├── cache/materialization plan        M6
        ├── WIT worlds                        M8
        ├── semantic diff                     M14
        └── debug/trace metadata              M14
                  │
                  ▼
Browser ───── edge ───── origin ───── database
   one compiler-known distributed application
```

---

## What Milestone 0 learned that changes the plan

Four measured findings with architectural consequences. Each is evidence-backed;
see the named spike README.

**1. The `pw` compiler must own its own type manifest.**
Koka's JS output cannot carry nominal identity. Single-field `value struct`s are
erased (`Money_usd(350)` **is** `350`), and `Nothing`/`Nil` are both `null` and
runtime-indistinguishable. So `Money<USD>`, `StoreId`, and `Option<T>` get zero
protection at the Koka→JS boundary. Any lowering to Koka must be accompanied by a
`pw`-generated manifest that carries the information Koka drops.
→ `spikes/koka-js-interop` F-4, F-7.

**2. Exhaustiveness is our job, not Koka's.**
Koka enforces it only for functions whose effect row excludes `exn`. Milestone 1
must therefore write its exhaustiveness compile-fail cases with **total** effect
rows, or they pass vacuously; and Milestone 9A must implement the rule properly.
→ `spikes/koka-js-interop` F-8.

**3. Generated components must be `no_std`-equivalent, and imports must be
asserted against the declared world.**
A `std` Rust guest imports 15 WASI instances when its WIT world declares one.
That ambient authority comes from the language runtime, not from anything the
application asked for. The `no_std` guest imports exactly one and is 8.3× smaller.
The build must *check* the import list, not trust it.
→ `spikes/wasmtime-component` F-2, F-3.

**4. Marko's emitted HTML already is a document-parts representation.**
`<!--M_*2 a-->` anchors plus an inline resume manifest, with client payload that
does not scale with document size (9.6× the HTML → 1.002× the JS). Milestone 7
therefore has a working oracle to diff against rather than a blank page.
→ `spikes/marko-stream-resume` F-4, F-5.

---

## Repository layout

Charter §12, with the parts that exist today marked. Directories are created when
a milestone needs them — §12: *"Do not create empty directories merely to look
complete."*

```text
perfect-web/
├── PROJECT_CHARTER.md          ✓  the constitution
├── AGENTS.md  CLAUDE.md        ✓  operating rules + pointer
├── README.md                   ✓
├── LICENSE-MIT LICENSE-APACHE  ✓
├── Cargo.toml                  ✓  workspace (wasm crates excluded)
├── rust-toolchain.toml         ✓  1.97.1 + wasm32-wasip2
├── package.json                ✓  pnpm 10.15.1, type: module
├── pnpm-workspace.yaml         ✓
├── justfile                    ✓  charter §13.4 command surface
├── docs/                       ✓  STATUS, NEXT, DECISIONS, ASSUMPTIONS,
│                                  KNOWN_LIMITATIONS, RISK_REGISTER,
│                                  ARCHITECTURE, SEMANTICS, BENCHMARKS,
│                                  DECISIONS/, milestones/, research/,
│                                  environment/, vision/, evidence/
├── examples/{accepted,rejected}✓  45 corpus files
├── spikes/                     ✓  four feasibility spikes
├── tools/                      ✓  corpus-check, versions.lock
├── scripts/                    ✓  doctor, bootstrap, record-environment
├── compiler/                   ─  Milestone 2
├── runtime/                    ─  Milestone 3
├── stdlib/                     ─  Milestone 1
├── benchmarks/                 ─  Milestone 3
├── lab/                        ─  Milestone 11
└── .github/workflows/          ─  see KNOWN_LIMITATIONS (Linux CI not yet wired)
```

---

## Invariants

These hold from Milestone 0 onward and are checked, not merely intended.

1. **Generated code is never authoring source.** It lives in build directories
   and never appears in a user-facing diagnostic. Asserted today by
   `explain_reports_derived_placement_without_naming_generated_files`.
2. **Every temporary dependency sits behind a boundary with a deletion
   condition**, recorded in an ADR and in the table above.
3. **Tests define behavior independently of the temporary dependency.** The
   Marko byte budgets and streaming timings are stated as properties, not as
   "whatever Marko does".
4. **Versions are pinned to what was actually tested**, in `tools/versions.lock`,
   with `just doctor` warning on drift.
5. **No credentials, tokens, private certificates, or user data** in the
   repository. `.gitignore` excludes `lab/certs/*`.

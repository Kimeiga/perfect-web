# Assumptions

Every assumption this project is running on that has **not** been proven yet.
PROJECT_CHARTER.md §3.2 permits making reasonable assumptions instead of asking
broad questions — but §3.3 forbids hiding them.

Each entry states how it gets validated and what happens if it turns out false.
An assumption is removed from this file only when it is either proven (and moved
into an ADR or `docs/SEMANTICS.md`) or disproven (and moved into
`docs/KNOWN_LIMITATIONS.md` with the consequence).

Status values: `open` · `validated` · `invalidated` · `superseded`

---

## A-001 — Host hardware differs from the charter's stated target

**Status:** `validated` (measured 2026-08-05) — the deviation is real and permanent.

The charter (§1, §13, §14 M11) specifies *"an Apple Silicon MacBook Pro with an
M3 Max and 64 GB RAM"*. The actual host is:

```text
model   Mac14,10 (MacBook Pro 14", 2023)
cpu     Apple M2 Pro, 12 cores
memory  16 GiB   (17179869184 bytes)
os      macOS 26.5.2 (25F84), arm64
```

**Consequence — Milestone 11 VM sizing is not executable as written.** The
charter's table allocates edge 3 GB + origin 6 GB + database 4 GB + router 1 GB
= **14 GB of guests on a 16 GiB host**, which leaves nothing for macOS, the
browser, and the compiler. §13.5 already warns "Do not allocate all 64 GB to
VMs"; on this host the constraint is much tighter.

**Revised plan for Milestone 11** (to be confirmed by an ADR when M11 starts):

| VM | charter vCPU/RAM | revised vCPU/RAM |
|---|---|---|
| edge | 2 / 3 GB | 2 / 1.5 GB |
| origin | 4 / 6 GB | 4 / 2.5 GB |
| database | 2 / 4 GB | 2 / 1.5 GB |
| router | 1 / 1 GB | 1 / 0.5 GB |

Total revised guest allocation ≈ 6 GB, leaving ≈ 10 GiB for host, browser and
toolchain. Stage 11A (native-process topology, no VMs) becomes correspondingly
more important as the primary functional validation path, with 11B reserved for
network-impairment tests that genuinely need separate network namespaces.

**Also affected:** benchmark absolute numbers are not comparable to any figure
produced on an M3 Max. Every benchmark record must carry the host string
(charter §18.5 already requires this).

**Validation:** `just doctor` reports memory and warns below 32 GiB;
`docs/environment/macbook.md` records the full detection output.

---

## A-002 — Node 22 (maintenance LTS) is acceptable instead of Node 24 (active LTS)

**Status:** `open`

The charter §13.2 asks for "a pinned current Node LTS". The current *active* LTS
is **v24.19.0 "Krypton"** (verified on `nodejs.org/dist/index.json`, 2026-08-05).
The host has **v22.21.1**, which is a maintenance-track LTS.

Assumption: v22.21.1 is sufficient for Milestone 0 and Milestone 3, because the
two packages that constrain us declare:

```text
vite@8.2.0    engines.node = ^20.19.0 || >=22.12.0    -> satisfied
marko@6.3.32  engines.node = >=22                     -> satisfied
```

**Why not upgrade now:** installing a second Node would add an unpinned variable
to Milestone 0 without changing any gate outcome. The charter's own rule is to
pin *what was actually tested*.

**Invalidation condition:** any Milestone 3 dependency requiring Node ≥ 24, or a
Node 22 end-of-maintenance date arriving before Milestone 3 completes.
**Action if invalidated:** pin Node 24 via `mise` or Homebrew, re-run
`just env-record`, re-run the Marko spike, and record the delta.

---

## A-003 — Koka is usable as a *semantic oracle* through its JS backend

**Status:** see `docs/milestones/E0.md` (measured by `spikes/koka-js-interop`)

Assumed: Koka 3.2.3's JavaScript backend emits modules that Node can load, and
`maybe`/`result` values can cross the boundary with explicit decoding. The
charter's whole Milestone 1 plan depends on this.

**Fallback if false** (charter §20): keep Koka only as an isolated semantic
oracle, do not route the Marko/browser work through it, and pull the own
type/effect checker (Milestone 9) earlier.

---

## A-004 — Marko 6 can emit a genuinely zero-JS static route

**Status:** see `docs/milestones/E0.md` (measured by `spikes/marko-stream-resume`)

The charter's §18.4 budget asserts "static page: 0 application JS and 0 runtime
JS" as a *target to test, not a fact to fake*. Milestone 0 measures what Marko 6
actually emits rather than assuming the marketing claim.

---

## A-005 — WASI 0.3 may not be reliable; 0.2 is the safe path

**Status:** see `docs/milestones/E0.md` (measured by `spikes/wasmtime-component`)

Charter §10.3 instructs preferring the newest stable WASI the toolchain supports
*reliably*, and to fall back to 0.2 behind an adapter if 0.3 tooling is
incomplete. The assumption entering Milestone 0 is that Rust's `wasm32-wasip2`
target plus Wasmtime 47 is the dependable path today.

---

## A-006 — `docs/`-scoped locations satisfy the operator's requested file names

**Status:** `validated` by construction

The session instruction asks for `STATUS.md`, `DECISIONS.md`, and
`ASSUMPTIONS.md`. The charter (§3.4) mandates `docs/STATUS.md` and
`docs/DECISIONS/ADR-*.md`. Because the charter is the authoritative constitution
and duplicated files drift, all three live under `docs/`:

- `docs/STATUS.md` — charter-mandated, single source of truth.
- `docs/DECISIONS.md` — index and short log over `docs/DECISIONS/ADR-*.md`.
- `docs/ASSUMPTIONS.md` — this file. Not named in the charter; added because
  §3.2 and §3.3 require assumptions to be explicit and it had no home.

**Invalidation condition:** the operator wants them at the repository root.
That is a one-line move plus a pointer update.

---

## A-008 — charter v2 is the governing charter

**Status:** `RESOLVED 2026-08-05` — the operator chose **adopt v2 and reopen
Milestone 0**. `PROJECT_CHARTER.md` is now v2 (3,206 lines, sha256
`5711d0d6…f686e6`); v1 is archived at
`docs/research/charter-v1-superseded.md`. Both new spikes were built and run, and
the Milestone 0 gate was re-evaluated against the six-spike requirement.

`perfect-web-master-agent-prompt-v2.md` was added to the repository at 17:39 on
2026-08-05, after `PROJECT_CHARTER.md` had been created from the v1 prompt and
while Milestone 0 was being executed. All Milestone 0 work was done against v1.

**v2 is a strict superset** (3,206 lines vs 3,080). Verified differences:

| area | v1 | v2 |
|---|---|---|
| mission items | 16 | 17 — new item 12, *frame-phase and layout safety* |
| §7.5A | absent | **new**: `dom.mutate`, `style.mutate<LayoutAffect>`, `layout.measure`, `observe.resize`, `observe.intersection`, `animation.composite`, `paint.custom`, `post_paint`; a default frame transaction; a ban on ordinary code calling `offsetWidth`/`getBoundingClientRect`/etc. directly, replaced by a scheduled `LayoutSnapshot<T>` |
| design donors | — | **new**: Jane Street Incremental, Bonsai/Bonsai_web |
| anti-patterns | — | **new**: *"Fine-grained updates are mistaken for layout safety"* |
| **Milestone 0 spikes** | **4** | **6** — adds `bonsai-incremental-model`, `layout-phase-scheduler` |
| **Milestone 0 gate** | "all four spikes run from documented commands" | "all **six** spikes run…, **or** a primary-source-backed blocker is recorded for any upstream toolchain that cannot currently run" |
| E0 research list | — | adds browser rendering phases, forced synchronous layout, CSS containment, `content-visibility`, ResizeObserver, Long Animation Frame attribution |

**Milestone 0's own task list and gate are otherwise byte-identical between the
two versions**, so every artifact produced so far remains required under v2.
Nothing done is invalidated; two spikes are simply missing.

**Why the charter was not overwritten:** replacing the project constitution is a
consequential, human decision. Charter §3.2 permits resolving design questions
experimentally, but this is not a design question — it is which document is
authoritative.

**What the resolution required, all done:**

- `PROJECT_CHARTER.md` replaced by v2; v1 archived, not deleted.
- `docs/SEMANTICS.md` gained §5A (layout effects and frame phases).
- `docs/research/technology-matrix.md` gained Jane Street Incremental and
  Bonsai/Bonsai_web rows.
- `spikes/layout-phase-scheduler` and `spikes/bonsai-incremental-model` built,
  run, and evidenced.
- `docs/milestones/E0.md` re-evaluated against the six-spike gate.

The OCaml toolchain **did** install on Apple Silicon, so the charter's
"record the blocker" escape clause was not needed.

---

## A-007 — `.pw` is a safe provisional source extension

**Status:** `open` — decided provisionally in ADR-0010, revisited in Milestone 2.

`.pw` is used by no widely deployed language toolchain the author is aware of.
This has **not** been exhaustively verified against editor/language registries.
Milestone 2 task 1 owns the real decision; changing it before Milestone 2 costs
a rename of corpus files only.

---

## A-009 — one `pw check` invocation is one program

**Status:** `open` — narrowed by name resolution, which does not exist yet.

Body-level checks need a type environment: `R-007` matches on `OrderState`,
which is declared in `A-002`. `compiler/pw-core/src/check.rs` therefore builds
one `Program` from **every file passed to the invocation** and checks each file
against it.

**What this assumes.** That the files given together belong together. It is how
a real compiler treats a package, and the corpus is written as excerpts of one
application, so it holds for every use the project has today.

**Where it is wrong.** Import-based visibility is not enforced. A file can
currently match on a type it never imported, and `R-007` in fact does. Two
files in different programs declaring the same type name would collide.

**Why it is safe to hold now.** The failure mode is *permissiveness*, not false
reports: the environment can only make a type visible that should not be, and
the only consequence is that a match gets checked which would otherwise be
skipped. It cannot invent a variant, so it cannot invent a diagnostic. The
narrower direction — refusing to check anything until name resolution exists —
would leave the tested exhaustiveness algorithm unreachable from source.

**Retire when** name resolution lands and the environment is built from a
module's imports rather than from the invocation's file list. The test
`the_environment_spans_every_file_checked_together` pins the current behaviour
in both directions, so the change will be visible.

**Narrowed by architect ruling, 2026-08-06.** The correct reading is *"`pw
check` constructs one workspace module graph"*, **not** *"every declaration in
every supplied file is ambiently visible everywhere"*. There is to be no ambient
union of user declarations, and no opaque-symbol fallback: external
implementation is allowed through an explicit interface module, a missing
declaration is not. E2B owns the change; `examples/domain.pw` was written so the
corpus can survive it.

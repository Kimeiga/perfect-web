# STATUS

<!-- Charter §3.4 requires exactly these sections. Keep them. -->

> ### ⚠ UNRESOLVED: which charter governs
>
> A second charter, `perfect-web-master-agent-prompt-v2.md`, was added to the
> repository at 17:39 on 2026-08-05, **after** `PROJECT_CHARTER.md` was created
> from v1. It is a strict superset (3,206 lines vs 3,080) and it **changes the
> Milestone 0 gate**:
>
> - Milestone 0 grows from **four spikes to six**, adding
>   `spikes/bonsai-incremental-model` and `spikes/layout-phase-scheduler`.
> - The gate becomes *"all **six** spikes run from documented commands, **or** a
>   primary-source-backed blocker is recorded"*.
> - New mission item 12: **frame-phase and layout safety**; new §7.5A defining
>   `dom.mutate`, `style.mutate<LayoutAffect>`, `layout.measure`,
>   `observe.resize`, `observe.intersection`, `animation.composite`,
>   `paint.custom`, `post_paint`.
> - New design donors: Jane Street Incremental, Bonsai/Bonsai_web.
> - New anti-pattern section: *"Fine-grained updates are mistaken for layout safety."*
>
> **Milestone 0 was executed against v1 and its gate passes against v1.**
> Under v2, Milestone 0 is **NOT complete** — two spikes are outstanding.
>
> `PROJECT_CHARTER.md` has deliberately **not** been overwritten: replacing the
> project constitution is a human decision, not one to make silently mid-session.
> **This must be resolved before Milestone 1 begins.**

**current milestone:** **Milestone 0 — COMPLETE against charter v1** (gate passed
2026-08-05 with one documented shortfall, Linux CI).
**INCOMPLETE against v2** — two spikes outstanding.
Full assessment: `docs/milestones/M0.md`.

**next milestone:** Milestone 1 — Koka semantic kernel. **Not started, and must
not start** until the charter question above is resolved.

**last passing commit:** see `git log -1` — the Milestone 0 bootstrap commit.
`just ci` passes at that commit on macOS 26.5.2 / arm64.

---

## completed gate items

All seven charter §14 M0 gate items:

1. **`just doctor` works on the Mac** — exits 0, read-only, warns on the 16 GiB host deviation.
2. **All four spikes run from documented commands** — `just spikes`, evidence in `docs/evidence/M0/`.
3. **Versions and licenses pinned** — `tools/versions.lock`, `rust-toolchain.toml`, `pnpm-lock.yaml`, SHA-256-verified release tarballs, license column in the technology matrix.
4. **≥10 accepted / ≥20 rejected examples** — **14 and 31**, covering **14/14** and **31/31** charter §16 categories. Enforced by `tools/corpus-check` in `just ci`.
5. **Reuse/fork/tape/build matrix complete** — `docs/research/technology-matrix.md`, with *measured* vs *read* clearly distinguished.
6. **Known failures documented honestly** — `docs/KNOWN_LIMITATIONS.md`.
7. **One clean `just ci`** — passes on macOS.

---

## failing gate items

**None outstanding.** One partial:

- **Gate item 7 is macOS-only.** Linux CI is not wired up (`.github/workflows/`
  is empty), so charter §13.5 case-sensitivity checks and §3.6 license/vulnerability
  scanning do not run. Does not block Milestone 1 (nothing in it is
  platform-sensitive); **must** be closed before Milestone 3. Risk R11.

---

## exact commands to reproduce

```bash
just doctor        # read-only environment check; exits 0 when M0 tools are present
just bootstrap     # fetch pinned Koka 3.2.3 + Wasmtime 47.0.3 into .toolchain/, pnpm install
just ci            # fmt-check + clippy -D warnings + 18 unit tests + corpus check  -> "ci: OK"
just spikes        # all four spikes; rewrites docs/evidence/M0/*.txt

# individually
just spike-compiler-diagnostic
just spike-koka
just spike-wasmtime
just spike-marko

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
```

---

## next three concrete tasks

1. **Wire up Linux CI** (`.github/workflows/ci.yml`): run `just ci` on
   ubuntu-latest arm64+x64, add case-sensitivity checks and `cargo-deny` /
   `pnpm audit`. Closes the gate item 7 shortfall and risk R11.
2. **Begin Milestone 1 task 1** — Koka effect libraries under `stdlib/koka/`
   for `database_read`, `database_write`, `network`, `clock`, `random`, `trace`,
   `session`, `secret`, `storage`, `task_scope`, `resource_scope`, `query`,
   `command`, `subscription`.
3. **Promote `spikes/koka-js-interop/node/kki.mjs` to `tools/kki-effects/`**
   with tests. It is the mechanism Milestone 1's gate depends on for
   *"every effectful example has a visible inferred effect"*.

Full ordered list with acceptance criteria: `docs/NEXT.md`.

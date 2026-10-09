# ADR-0281: a merge is held to CI's verification run

Status: accepted under the owner's delegation of 2026-10-02, on the owner's
decision of 2026-10-08, 22:45 (relayed). Date: 2026-10-08. Milestone: E14.
Amends the integrator's practice under ADR-0245.

## Context

- **Merging waited on local chains of mutation scripts.** Before merging,
  the integrator ran whole each script whose code or tests the change
  touched, one after another on the development machine:
  - ADR-0277's 17 scripts took 4.5 hours;
  - ADR-0280's 24 were stopped after five of them, on 2026-10-08, to free
    the machine. Its load was 7 to 10 on 12 cores, with a worker's builds
    beside the chain, and its memory nearly full.
- **CI already runs the same recipes** (ADR-0245, ADR-0249, ADR-0278).
  Verification runs them in 17 parallel shards on GitHub in under an hour,
  each recipe's output an artifact stamped with its run.
- **The owner's decision** (relayed 2026-10-08): CI's verification run is
  the evidence of record for a merge. Locally, the fast gates and the
  touched area's quick checks only.
- **CI's plan missed browser specs.** `ci_plan.py` reached a recipe from a
  change to its mutation script, to a line near a mutant's anchor, or to a
  Rust test file a script runs. A change to a browser spec reached nothing:
  ADR-0280 changed `pages.spec.mjs` and `accessibility.spec.mjs`, and
  `e14-accessibility` and `e14-titles` would not have run.

## Decision

1. **A branch is merged once its tip's runs pass.**
   - CI's `ci` and `verify` runs at its tip are green, apart from failures
     already open in NEXT with their runs. The merge's message names each
     such failure.
   - The integrator's own work is pushed first to a branch `track/<name>`,
     so that verification plans what it changed since it left `master`, as
     a track's push does (ADR-0253).
   - The merge's message names its verification run, and what it found.
2. **Locally, before a push:**
   - `just ci`, the fast gates;
   - the new or changed tests themselves;
   - the change's own mutation script, once. That run is a first look,
     which finds a survivor sooner than a run on CI would. It is not the
     record.

   Every other script the change touches runs on CI only.
3. **CI plans what a change touches** (`ci_plan.py`). A browser spec
   counts as a test, as a Rust test file does. A change to one plans:
   - each mutation script that runs it;
   - each recipe that names it in its own lines.
4. **The record.** `just evidence-fetch <run>` copies a run's evidence into
   `docs/evidence/`, each file stamped with its run and its commit (ADR-0245).
   - That is a file a recorded command produced, as CLAUDE.md asks.
   - The fetch is a download, so it waits for the owner's yes in the
     integrator's session. Approval relayed through another session does
     not grant it.
   - Until the owner says yes, a merge cites its run, and a gate's claim
     waits for its fetched file.
5. **What CI cannot run stays local, and says so.** These are recorded on
   the development machine, each recipe naming its machine:
   - measurements of the machine: `LOCAL_ONLY`'s timings, loads and memory
     curves, and the long-frame instrument;
   - behaviour that is macOS's own;
   - anything that needs the owner's local data. Kiokun's whole dictionary
     is read through `KIOKUN_DATA`, and on CI its recipes run on a
     committed sample, saying they skipped the rest.

## Acceptance

- **`scripts/tests/test_ci_scripts.py`, 23 tests.** Among them, one new on
  this ruling: a changed browser spec reaches the scripts that run it
  (`accessibility_mutations.py`), and the recipes that do
  (`e14-accessibility` and `e14-titles`). A spec that nothing runs reaches
  nothing.
- **The first merges under it**: ADR-0280's and the soundness ruling's,
  each naming its verification run.

## Not claimed

- **CI's runners are not the development machine.** They run Linux, where
  this machine runs macOS, and their WebKit is Playwright's Linux build.
  What only macOS shows is local, under decision 5.
- **A flaky test can pass a run.** NEXT names each flake that is still
  open, and a merge names any it met.
- **Until the owner says yes to `evidence-fetch`**, `docs/evidence/` holds
  only what was recorded here.

## Alternatives

- **Keep the local chains.** That costs hours a merge, and a machine too
  loaded to build on.
- **Run the local chains in parallel worktrees.** Each worktree needs a
  target directory of its own, and the disk and the memory were already
  short.
- **Merge on `just ci` alone.** No mutation control would run before a
  merge, so a test that kills nothing could land.

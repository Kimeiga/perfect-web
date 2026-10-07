# ADR-0245: the heavy verification runs on GitHub Actions

Status: accepted under the owner's delegation of 2026-10-02, on the owner's
direction to move the heavy verification off the laptop (relayed
2026-10-07). Date: 2026-10-07. Milestone: E14.

## Context

- **A commit's verification was serial, and on one machine.** After `just
  ci`, the chain ran each recipe the commit touched, each re-running its
  tests and its mutation controls one mutant at a time: ADR-0239's chain
  ran 39 recipes, for hours. An ADR's own controls and the scripts near its
  change took hours more: ADR-0243's were 27 mutants, then 99. While they
  ran, the worktree was theirs, and ADR-0244's 21 scripts were not run.
- **Master's CI went unread for two days** (ADR-0244).
- **Charter §13.5 asks for Linux CI**, "to catch casing errors". The
  recipes had only run on macOS.
- **The repository is public**, and GitHub Actions is free there on
  standard runners: "GitHub Actions usage is free for self-hosted runners
  and for public repositories that use standard GitHub-hosted runners"
  ([billing](https://docs.github.com/en/billing/managing-billing-for-your-products/managing-billing-for-github-actions/about-billing-for-github-actions)).
  The Free plan runs 20 jobs at once, each for at most 6 hours, a matrix of
  at most 256 ([limits](https://docs.github.com/en/actions/reference/limits)).
- **The whole set is large**: 208 evidence recipes plant about 1,900
  mutants, each a build and a test run.

## Decision

1. **A `verify` workflow** (`.github/workflows/verify.yml`): a plan, the
   recipe shards, the browser suite in each engine, and a summary.
   - **A push to master** runs the recipes its commits touch, the chain's
     choice, in up to 16 shards;
   - **a nightly run** runs every one, in 48;
   - **on demand**, named recipes, or every one.
2. **The plan** (`scripts/ci_plan.py`, `just verify-plan`) takes `just`'s
   own recipes, every `e<N>-…` one but the measurements of a machine
   (`LOCAL_ONLY`: a time, a load, a memory curve). A range of commits
   touches a recipe that runs a mutation script it changed, whose mutant
   sits within 30 lines of a changed line, or whose tests it changed, and a
   recipe whose own lines it changed. Recipes are dealt to the least loaded
   shard, the costliest first, by the mutants each plants.
3. **A shard** bootstraps the pinned toolchains, installs the browsers,
   builds, and runs its recipes one after another (`scripts/ci_recipes.py`).
   Each recipe's exit status, time, output, and the evidence files it wrote
   (those whose bytes changed while it ran) are the shard's artifact.
4. **The summary** (`scripts/ci_summary.py`) fails the run where a recipe
   failed, a mutant survived, or a shard reported nothing.
5. **Evidence is fetched by a recorded command**: `just evidence-fetch
   <run>` (`scripts/evidence_fetch.py`) copies a successful run's evidence
   into `docs/evidence/`. A file with a `commit:` line gains a `ci:` line
   after it naming the run and its runner, so it says which command produced
   it, at which commit, and where. A file naming another commit is refused,
   and so is a run of another commit than `HEAD`. The rule that evidence is
   "a file under `docs/evidence/` that a recorded command produced" holds as
   written: the recipe produced it, on the run, and the recorded fetch
   brings it here.
6. **Locally, before each commit**: the precommit gates, the workspace, the
   ADR's own mutation controls and the browser specs it touches. The chain
   runs on the push.

## Acceptance

- **`scripts/tests/test_ci_scripts.py`, 14 tests**:
  - a plan deals the costliest first, the same every time, and leaves out
    the measurements;
  - a change reaches the script it is, a test file a script runs, and a
    mutant near it, and not one far from every anchor;
  - a recipe wrote what changed while it ran;
  - a run fails for a failed recipe, a survivor or a missing shard;
  - the fetch names the run, replaces its own line, refuses another commit,
    and copies a file with no commit as it is.

  `just evidence-gates` runs them, with the gates' own.
- **The first runs**: RUNS.

## Not claimed

- **The measurements** stay on the development machine, which each names in
  its `host:` line.
- **A run commits nothing.** Its evidence is fetched here and committed
  after reading the run.
- **A push's plan covers its own commits.** A run is not cancelled for a
  later push, and the nightly run covers every recipe.

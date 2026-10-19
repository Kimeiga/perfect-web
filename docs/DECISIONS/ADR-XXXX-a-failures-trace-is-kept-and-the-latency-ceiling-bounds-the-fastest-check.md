# ADR-XXXX: a failure's trace is kept, and the latency ceiling bounds the fastest check

Status: proposed by track `ci-diagnostics` (2026-10-19) for the integrator;
local gates and the track's recipe recorded below; not yet in a train.
Date: 2026-10-19. Milestone: E14; the evidence's own harness (ADR-0245,
ADR-0281, ADR-0344, ADR-0349).
Track `ci-diagnostics`, on `8c5618c8` (`track/train-10`'s head), not on
`master` `32757966` (the first correction).
Amends ADR-0349's `scripts/run_suite.sh` and ADR-0344's section 4
(`check_latency`).

## Corrections first

- **The brief's base held neither thing it asked to fix.** Master
  `32757966` has no `scripts/run_suite.sh` and no
  `scripts/tests/test_run_suite.py` (ADR-0349), and its `check_latency.rs`
  is still the wall-clock 2 s version: both the helper and the
  CPU-time-per-thousand-lines ceiling (ADR-0344, `2f343bd1`) are train-10's.
  The branch was moved to train-10's head before any commit of its own,
  as the other tracks for train-11 are (`browser-host-map-train11` was "on
  `587657ff`"). Reversible: `git reset --hard 32757966` drops nothing of
  this track's.
- **`test-results/` was never the failing run's, only the last run's.**
  `verify.yml` said so ("each run of a suite clears the directory, so this
  is the last run's") and uploaded it once, at the shard's end. In
  train-10's second verify (39131257549) that was a mutation control's
  intended failures, as the integrator read it: `e10-browser`'s second
  Chromium run (a `scheduled-orders.spec.mjs:74` timeout) had failed, its
  third and every Playwright run after it in the shard had cleared the
  directory, and the trace of the one failure the train had to chase was
  not in `traces-23` (this track downloaded nothing of it).
- **No hardware counter is to be had here.** The brief allowed "a hardware
  instruction count if one is available in the test environment". In this
  WSL2 machine `perf stat -e instructions:u,cycles:u` says "not
  supported" (the virtual machine has no PMU: `arch_perfmon` is not among
  the CPU's flags, `perf_event_paranoid` is 2), and a GitHub-hosted runner
  is a virtual machine too. The statistic is the fastest of several CPU
  times (section 2).
- **At the PC's own loads the old ceiling holds for train-10's checker.**
  Measured below: 143 to 330 ms CPU a thousand lines, alone, beside a
  build, or beside 64 busy loops, against 800. The failures the coordinator saw (three of five
  local `just ci` runs at load 40 to 80) need the complete-validation
  checker of track `check-is-build` (2.0 to 2.9 times the work: 508 to 809
  ms a thousand lines at loads of 21 to 55, its own notes), which is not
  on this base. So the proof of section 2 runs the checker also made two
  and three times over, as that one will be.

## 1. A failing run's output is kept

- **Cause.** Playwright removes each project's `outputDir`,
  `test-results/`, as a run starts (`createRemoveOutputDirsTask`, which
  only the watch mode skips; `playwright@1.58.0`, `lib/runner/tasks.js`).
  A recipe that runs a suite several times (`e10-browser`, three), and every
  Playwright run after it in a CI shard, a mutation control's among them,
  left the last run's output there, and the shard uploaded that once.
- **Fix.**
  - `scripts/run_suite.sh` copies a run's `test-results/` as the run ends,
    when it failed, into `<suite dir>/kept-traces/<recipe>/<time>-<pid>/
    run-<i>/` (`scripts/keep_traces.py`), before the next run clears it,
    and says where, before it says the run's exit. The recipe is
    `$PW_RECIPE`, which `ci_recipes.py` now sets for each recipe it runs
    (`local` where unset); the time and the helper's pid keep one call's
    runs apart from another's in the same recipe. Its status is the
    suite's whether or not the keeping worked.
  - **A run inside a mutation script is not kept, except its baseline,
    and that only when it came out red.** A mutant's suite fails by design,
    once per mutant, many in a shard (`e14-refusal` alone plants 12 browser
    mutants, each failing tests); its trace says nothing the kill does not,
    and they are the swamp the brief names. A red baseline is a
    real failure that stops the script, and it was lost with the shard's
    later runs as `e10-browser`'s was. `mutation_baseline.explain()`, which
    all 277 call sites call where a baseline is red, now also keeps the
    output of the script's last command where that was a browser run that
    failed, in `<suite dir>/kept-traces/<recipe>/<time>/<script>-baseline/`.
    "The last command" excludes a mutant's: of the 277 calls, 272 follow the
    baseline's own red print, and the five others (a refused producer, the
    restored source's own check) find a cargo command or a real failure
    last; a test names the five.
  - **Bounded.** `kept-traces/` holds at most 32 MiB (`PW_KEPT_TRACES_KIB`)
    in all: ten times the 3 MB of train-10's whole `traces-23`, which held
    a run's failed tests. An entry that does not fit is not copied, and the
    run's directory names
    it and its size (`NOT-KEPT.txt`), so a run in which hundreds of tests
    failed keeps its first and says what it left. The upload steps of
    `verify.yml` (`traces-<shard>`, `browser-<engine>`) name every
    `kept-traces/`, beside the last run's `test-results/`, as before.
- **Claimed.** `test_run_suite.py`: a run that fails, then one that passes,
  keeps the first's trace though the second cleared `test-results/` (the
  stand-in clears it, as Playwright does), a passing run's is not kept, each
  failed run of three is kept apart, two calls in a recipe keep apart, a
  recipe's name (or `local`) is the directory and cannot leave it, the
  benchmark harness keeps under its own, the budget holds and names what it
  left, and a failure to keep leaves the suite's status. `test_keep_traces.py`:
  the copy, the budget (the root's whole, not the run's), a name that cannot
  leave the root, the command line, `ci_recipes.py`'s `PW_RECIPE`, and both
  upload steps of `verify.yml` naming `kept-traces/`.
  `test_mutation_baseline.py`: a red baseline's failed browser run is kept,
  a passed one, a mutant's (anything but a browser run last) and an earlier
  recipe's are not. 25 mutation controls
  (`scripts/ci_diagnostics_mutations.py`, `e14-ci-diagnostics`).
- **Not claimed.** That a trace explains a failure: it is kept, not read.
  That every Playwright run is kept: only a failed run of the helper, and a
  red baseline. A mutation script that runs Playwright and fails in a way
  that is not a red baseline (a mutant's test failing for the wrong reason)
  keeps nothing, as before. The browser jobs run the suite once, so
  their `test-results/` was never lost; they upload `kept-traces/` for
  symmetry only. The 32 MiB is a choice, not a measurement of the artifact
  store's quota.

## What the planner plans when `run_suite.sh` or `verify.yml` changes

Measured with `scripts/ci_plan.py` on this track's first two commits
(`a5649409`, `061fdba8`), each changed file alone:

- **`scripts/run_suite.sh` alone plans 91 recipes** of the 362 CI can run:
  every recipe whose lines name the helper's path (`reads_what_changed`'s
  rule for "a shell script it runs", ADR-0281), `e10-browser` and every other
  browser recipe among them. The helper's behaviour differs only on a run
  that failed, but the planner cannot see that, so any change to it
  re-verifies every browser recipe.
- **`.github/workflows/verify.yml` alone plans one: `e14-ci-diagnostics`**,
  by this track's two upload mutants. A workflow is no recipe's input; a
  recipe is planned by it only where one of its mutants is anchored within
  30 lines of a changed line (a change near the plan job's `--stale` case
  would plan `e14-evidence-integrity`, as before).
- `scripts/mutation_baseline.py` plans three, by mutants anchored near the
  hunk (`e14-ci-diagnostics`, `e14-derived-outside`,
  `e14-evidence-integrity`); `ci_recipes.py`, `keep_traces.py`, the
  `.gitignore`s and the recipe file plan this track's recipe alone.
- **Together, against train-10's head (`8c5618c8`): 92 recipes in 33
  shards** (`ci_plan.py --shards 9 --changed 8c5618c8 HEAD --current`, which
  leaves none out). **Against `master`, which is where a track's push
  measures from (`git merge-base origin/master`): 178 in 39**, since
  train-10's own changes count there, `--current` leaving out `e14-tables`.
  So a push of this branch starts a verify of 178 recipes, hours of runners
  repeating train-10's dispatches. The integrator may cancel it (train-6's
  and train-10's precedent) and dispatch what the change reaches:
  `e14-ci-diagnostics e14-evidence-integrity e14-evidence-integrity-2
  e14-derived-outside e10-browser`.

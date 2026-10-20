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
- **At the PC's own loads the old ceiling held for train-10's checker.**
  Measured below: 143 to 330 ms CPU a thousand lines, alone, beside a
  build, or beside 64 busy loops (loads of 25 to 60), and 333 to 644 at
  loads of 49 to 116 (the table), against 800. The failures the coordinator
  saw (three of five local `just ci` runs at load 40 to 80) need the
  complete-validation
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

## 2. `check_latency` bounds the fastest check

- **Cause.** ADR-0344's finding 4 moved the ceiling from the wall clock to
  the checking thread's CPU time, and noted that CPU time "is not free of
  load either" (about 1.5 times). This track measured how not, one
  6,568-line check at a time (a probe of the same call, thread CPU time and
  the CPU it ran on, 15 to 20 checks in a row): 0.94 s alone; 1.7 to 2.2 s
  beside a looping workspace build; 1.7 to 2.0 s beside 64 busy loops, and
  drifting by up to 2 times within 30 seconds as other workers' builds
  start and stop. The PC is an i9-13900K, SMT and efficiency cores under
  Hyper-V; a thread's CPU time counts the sibling's contention, the slower
  core and the host's, as well as its own work. The test took the median of
  seven consecutive checks, ten to thirty seconds of it, so the drift of
  those seconds was taken whole. For train-10's checker that is far from
  800 ms a thousand lines (143 to 644 measured, by load); for the
  complete-validation checker of track `check-is-build` (2.0 to 2.9 times
  the work, its notes) it was 508 to 809 at loads of 21 to 55, and the
  integrator reported three of five local `just ci` runs failing at 40 to
  80.
- **Fix.** The ceiling is unchanged: 800 ms of the thread's CPU time a
  thousand lines (a test pins the number, so a raise is a visible change to
  a test). It bounds the fastest of at least seven checks, not their median:
  a machine's load only adds to a check's time, so the fastest is the
  least moved, and a checker that is slow itself is as slow in its fastest
  check as in its slowest. Where the fastest so far is over the ceiling,
  more checks are taken, until one is within it, or 40 have been taken, or
  their wall clocks add up to four minutes (`sample`): a quiet machine takes
  exactly seven, as before, and a machine that quiets down is waited for.
  The slow path's ratio to the clean (5) is of the fastest of both; the
  control (a check that happened, more than 200 microseconds of CPU time)
  is of the fastest. The medians are still printed, with the fastest and
  the number of checks, so that CI's logs go on saying what the runners
  measure; a failure says the load average.
- **Hardware counts: not available** (the corrections): the statistic is
  time. **Not the other fixes:** a calibration loop on the same thread (a
  new unit, calibrated on a machine not at hand: the ARM runner), pinning
  to a performance core (privileged and not portable), waiting for the load
  average to fall (it lags a minute, and a busy loop never falls), skipping
  the test under load (a gate that passes where it cannot measure), and
  raising the ceiling (ruled out).
- **Claimed.** Nine tests that run no check (`the_ceiling_is_800_ms_...`
  through `a_clock_that_reads_nothing_...`), on checks that cost what a
  script says: the ceiling is 800 and is per thousand lines; six checks of
  seven stretched past it and one not pass, by the fastest, though their
  median fails; the fastest is of CPU time, not wall; a quiet machine is
  asked seven times and no more; a machine that quiets down after ten
  stretched checks is waited for, to the first that fits; the waiting ends
  with the window or the most checks; **the checker made ten times slower
  fails however many checks are waited for**; a clock that reads nothing,
  or once among real readings, is refused by the control; the slow path is
  judged at the fastest of both. 13 mutation controls on them
  (`ci_diagnostics_mutations.py`), killed; and `evidence_integrity_mutations.py`'s
  two on the measurement itself still killed by the real tests, run whole
  (42 of 42): the checker run ten times over fails 1 of its 11 tests (the
  cold test), the thread's clock reading nothing 2 of 11.
  **The proof, at load.** The first test as committed, and the old
  one (`8c5618c8`'s, the median of seven) beside it, alternately, as test
  binaries built from a scratch copy of the tree (`/tmp/cid-proof`), the
  checker as it is, made two and three times over (the complete-validation
  checker costs 2.0 to 2.9 times as much), and ten times over (the
  control). On the PC (32 threads) beside the other workers' builds and
  tests and a looping `cargo build --workspace`; the one-minute load average
  at the start of each run in the table. ms are of CPU time a thousand
  lines; the ceiling is 800.

  | checker | test | runs passed | median of seven | fastest of seven or more | load |
  |---|---|---|---|---|---|
  | as is | old | 10 of 10 | 333 to 562 | | 49 to 92 |
  | as is | **new** | **10 of 10** | 323 to 644 | 240 to 461 | 57 to 116 |
  | 2 times over | old | 3 of 5 | 678 to 976 | | 45 to 76 |
  | 2 times over | **new** | **5 of 5** | 559 to 781 | 476 to 651 | 47 to 71 |
  | 3 times over | old | 0 of 4 | 931 to 1,329 | | 47 to 52 |
  | 3 times over | **new** | **4 of 4** | 730 to 1,181 | 642 to 702 | 43 to 56 |
  | 10 times over | **new** | **0 of 10 pass** | 2,742 to 3,610 | 2,389 to 3,442 | 66 |

  So: **the new test is green in 10 of 10 runs at load, and the slowed
  control fails in 10 of 10** (three to four times the ceiling at its
  fastest, so no number of checks lets it through). The old statistic
  failed the checker two times over in 2 of 5 runs and three times over in
  4 of 4, which the fastest of seven or more passes; one run of the latter
  took 12 checks, waiting for a quieter moment (its first seven's median
  was 1,181). At loads of 77 to 89, an earlier build of the same test (90 s
  window), the checker three times over failed both (the fastest 985, the
  median 1,209 and 1,329): see the next bullet.
- **Not claimed.** That the ceiling is proof against any load: a machine
  loaded through the whole of a measurement (four minutes) with every check
  stretched past the ceiling still fails it. Beside a load of 77 to 120 the
  PC's CPU time for the same check is up to 3 times its quiet time (the
  hybrid cores, SMT and the host, which a guest cannot see: WSL2 reports 16
  cores of two threads), and the checker three times over fails there
  whatever the statistic. That is the ceiling's margin to carry
  (train-10's checker: 800 against 143 to 644 on the PC, 321 on ARM; the
  complete-validation checker is 2.0 to 2.9 times that: ARM would measure
  640 to 930 against the 800, which is the integrator's to rule on, and
  no measurement here changes). A calibration loop would hold the verdict
  at any load, and needs a constant per machine class that only the ARM
  runner can give. That CPU time is
  the work. That `e9-latency`'s wording ("median of 7 runs") changed: it
  prints the medians still, and the fastest beside them. The real
  measurement's assertion lines are tested through `fast_enough` and
  `happened`, not mutated in place.

## End to end, with a real Playwright

`scripts/run_suite.sh --runs 3` on a toy spec that fails its first and
third runs and passes its second (Playwright 1.58.0, Chromium, `trace:
"retain-on-failure"`): run 1's `trace.zip` (5.8 KiB) was in
`kept-traces/e10-browser/<time>-<pid>/run-1/` after run 2 had cleared
`test-results/` and run 3 had left its own there; run 3's was kept as well;
the helper's lines named both before their exits. (A toy tree under `/tmp`:
the repository's own suite needs its builds, and is the verify run's.)

## Alternatives

- **A `test-results/` of its own for each run** (`outputDir` in
  `playwright.config.mjs`, by a timestamp): keeps every run's failures, a
  mutation control's among them (the swamp), and bounds nothing.
- **Upload after every recipe**: an artifact per recipe, hundreds a run,
  against the artifact store's quota, to keep what a few recipes fail.
- **The helper alone, not the red baseline**: a baseline that came out red
  loses its trace to the shard's later runs as `e10-browser`'s second run
  did; 59 scripts run Playwright, all through `explain()`.
- **Keeping a mutant's failing run too, under a label**: hundreds of traces
  a shard that the kill already explains.
- **A hardware instruction count, a calibration loop, pinning to a core,
  skipping under load, a higher ceiling**: section 2.

## Next

- Merge with train-11 (the integrator's); `check_latency.rs` differs from
  `track/check-is-build-train11`'s by one line, `check_program_sources`:
  take both.
- Rule on the ceiling for the complete-validation checker (800 ms a thousand
  lines; ARM would measure 640 to 930), or on a calibration loop with
  per-architecture constants, once `check-is-build` lands.
- A push of this branch plans 177 recipes in 39 shards (92 against
  train-10's head): cancel it and dispatch `e14-ci-diagnostics
  e14-evidence-integrity e14-evidence-integrity-2 e14-derived-outside
  e10-browser`, or merge first.

## Questions for the integrator

1. The base: this track is on train-10's head, not on `master`
   `32757966` (the first correction). Is that what you want, or should its
   commits wait for train-10 to land, or move onto train-11's tip?
2. Is a mutant's browser run never kept right (section 1), or should a
   failing mutant run in a recipe whose baseline passed (a mutant killed for
   the wrong reason) be kept under a label, within the same 32 MiB?
3. Should `e9-latency`'s wording ("Median of 7 runs") say that the ceiling
   bounds the fastest of them? (Its evidence is the Mac's, `LOCAL_ONLY`;
   unchanged here.)

## Acceptance

- **Tests.** `scripts/tests/test_keep_traces.py` (11), `test_run_suite.py`
  (16, 8 new), `test_mutation_baseline.py` (26, 8 new), `check_latency.rs`
  (11, 9 new, none timed); `just evidence-gates` passes whole.
- **Every mutation script the change reaches, run whole** (the tree at
  `e438f689`; the last two in clones of it, so as not to wait for the first
  three): `ci_diagnostics_mutations.py` 38 of 38 (25 + 13),
  `derived_outside_mutations.py` 10 of 10, `mutation_bound_mutations.py`
  16 of 16, `evidence_integrity_2_mutations.py` 35 of 35 (`run_suite.sh`'s
  anchors and `test_run_suite.py`), `evidence_integrity_mutations.py` 42 of
  42 (`mutation_baseline.py`'s and `check_latency.rs`'s). The memory bound
  stopped nothing; `mutation_anchors.py` finds 3,317 mutants in 274 scripts,
  each matching once, after each.
- **`just e14-ci-diagnostics`** records them (`docs/evidence/E14/ci-diagnostics.txt`,
  at `e438f689`); its evidence of record is the next train's run.
- **`just ci`** on `24c11117` (the code of `e438f689` and its recorded
  evidence), 00:46 to 02:08Z on the PC at loads of 40 to 90: `ci: OK`;
  evidence-gates 292 tests (2 skipped), case-check 2,876 paths, anchors
  3,317 mutants in 274 scripts, 346 test binaries with 3,074 tests passed,
  none failed, 25 ignored (train-10's 3,065 and this track's 9), mutant
  programs 301 judged in 12. Its own `check_latency` ran at a load of 55 to
  62: the fastest of seven 229.5 ms a thousand lines, the median 245.4.

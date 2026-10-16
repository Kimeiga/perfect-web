# ADR-0344: six ways evidence could be wrong, or a gate fail for the wrong reason

Status: accepted under the owner's delegation of 2026-10-02, on the
integrator's findings and rulings of 2026-10-15 (track
`evidence-integrity`, its four findings and two more relayed the same day).
Date: 2026-10-15. Milestone: E14; the evidence's own harness (ADR-0245,
ADR-0281, ADR-0292).

## Corrections first

- **Finding 2's cause is not that nothing staled the evidence.** The
  planner held `docs/evidence/E10/privacy-by-resolution.txt` not current
  all along (`ci_plan.current()` was right). What missed it: a push plans
  what its own range reaches, less what is current, so evidence gone stale
  before the range is planned by no push, and the nightly, which runs every
  recipe, keeps no evidence. 54 recipes CI runs had evidence stale by the
  planner's own rule at master 22a0508e; `labels-through-plain-values.txt`
  disagreed with its script too (3 of 3 mutants, of a script that plants 2
  since d1c451d2, ADR-0129, 2026-10-02).
- **Finding 3 is 37 headers, not two, and none was an accident.** The
  numbers were deprecated aliases (`diagnostics.rs`, `DEPRECATED_ALIASES`),
  kept "only while the corpus migrates", and every corpus test read a header
  through `canonical_code`. 37 of the 74 rejected fixtures named a number
  `pw check` never prints; the migration was never finished.
- **Finding 5: the recipes did stop at a failed build; the suite's own
  guard did not.** Every recipe line runs under the justfile's `bash -euo
  pipefail`, which a `{ ...; } > file` group keeps: a `;` after a failed
  `run.sh` ends the recipe (a recipe of the pattern, run by `just`, stops;
  so does one in an imported file). What let a suite run on a half-built
  `dist` was `playwright.config.mjs`: it checked `dist`'s runtime and
  sources only where `dist` held its runtime, which `run.sh` copies after
  step 3c, so a build stopped there or before was served, whoever ran the
  suite. And three recipes ran a suite with no build at all.
- **Finding 4: CPU time is not free of load either.** Beside 64 busy loops
  the checking thread took 1.2 s of CPU for what takes 0.79 s on a quieter
  machine: a busy core's sibling and a hybrid processor's slower cores
  stretch it about 1.5 times. It is still under the 2 s ceiling, where the
  wall clock (2.5 s) is not.

## 1. A mutation script's sources run as they are

- **Cause.** Python takes a cached `.pyc` for its source where the two agree
  in the source's size and its modification time in whole seconds
  (`importlib._bootstrap_external`'s `get_code`: `source_mtime =
  int(st['mtime'])`, `_validate_timestamp_pyc`, CPython 3.10 as installed).
  A mutant of a Python source the same size as its anchor, restored within
  the second it was written in, leaves its bytecode standing for the
  restored source: the next run, another mutant's, a baseline's or `just
  ci`'s, runs the mutant. Four scripts mutated Python: `dealt_by_time` (44
  mutants, 3 the same size), `derived_outside` (2), `kiokun_inventory` (26)
  and `mutation_bound` (16, 1 the same size); this track's is a fifth.
- **Fix.** `mutation_baseline.py`, which every script imports first in
  `main()`, sets `sys.dont_write_bytecode` and `PYTHONDONTWRITEBYTECODE=1`
  in `os.environ`, which every process the script starts inherits, before
  it imports anything of the repository. And it removes the bytecode cached
  for each Python source the script's `MUTANTS` change (`forget_bytecode`),
  each interpreter's in its `__pycache__` and where this one looks:
  `dont_write_bytecode` stops writing, not reading (`get_code` reads a
  `.pyc` whatever the setting says), and a `.pyc` an earlier, unguarded run
  left for a restored source would still be read. Removal is cheap, touches only
  caches git ignores, and makes the baseline's and each mutant's code their
  source's, whatever ran in the tree before. The change sits above `import
  mutation_bound`, apart from track parallel-mutants' lines below it, in a
  commit of its own.
- **Claimed.** On a toy tree's script, started as a script is, with one
  same-size mutant restored within its second (the seconds are set, not
  waited for): unguarded, the restored source runs the mutant ("1 2 2", the
  control); guarded, "1 2 1", and neither its suite nor its own process
  wrote bytecode; guarded after an unguarded run left the mutant's
  bytecode, its baseline reads the source (without the removal, "2 2 2").
- **Not claimed.** Not reproduced on a real script: their suites take
  seconds, so a restore within the mutant's second needs a suite that ends
  within it. A worktree that ran these scripts before this fix may hold a
  stale `.pyc` of a file no script it runs again mutates: removing
  `__pycache__` clears it. A child given an `env` of its own, without the
  variable, writes bytecode again; none of the five scripts' does.

## 2. Evidence gone stale is planned, and its counts are its script's

- **Cause.** The section above, corrections first. And nothing compared an
  evidence file's `k of n mutants killed` with the script that wrote it.
- **Fix.**
  - `ci_plan.py --stale` adds each recipe whose committed evidence is stale
    at the head, whatever the range: every file it writes names the commit
    it was recorded at (whole, or with changes beside it), or is missing,
    and it is not `current`. A file that names no commit (an artifact, an
    older recipe's text) is planned as before. `verify.yml` passes it at a
    train's push (`track/train-*`), whose run's evidence the merge takes;
    not at another track's, whose run checks its change and is not the
    record, nor at `master`'s, which plans from the run a train passed
    (`verified_base.py`). The next train's run records the 54 again.
  - `scripts/evidence_counts.py`, held by `just evidence-gates`: for each
    recipe that runs a mutation script, each `k of n mutants killed` in
    each evidence file it writes is read with the verdicts above it. `n` is
    as many mutants as the script defines and as many verdicts as the block
    holds; `k` as many as say KILLED; each verdict names one of the script's
    mutants, once (by its description, the longest of its words, beside its
    other words, a suite's tag in brackets and separators: `[core]`,
    `next-react: `, `V1  `); each script the recipe runs is counted. A
    disagreement fails unless a train's push records it again: CI runs the
    recipe (not `LOCAL_ONLY`), its files name their commit, and the planner
    holds the evidence stale in the tree here (not at the head, since `just
    ci` runs before a commit). Evidence of record comes only from a run
    (ADR-0281), so such a file is named and fails nothing.
- **Claimed.** Of 265 count lines in 262 evidence files, two recipes'
  disagree, both stale and planned (`privacy-by-resolution`, 4 counted of
  a script of 3, and a verdict naming the retired mutant;
  `labels-through-plain-values`, 3 of 2, the same); the rest agree with
  their scripts. `stale()` holds 54 of master's 280 CI recipes stale at
  22a0508e. On this branch, `ci_plan.py --changed origin/master HEAD
  --current --stale` plans 67 recipes in 9 shards: the 17 its own changes
  reach, and 50 more only because their evidence is stale, both above
  among them.
- **Not claimed.** A subset of a script's mutants that a recipe documents:
  every recipe runs its scripts whole (264 invocations, no arguments), so
  none is read; one would need a form the gate can read. A disagreement
  whose recipe fails on CI stays stale and is planned at every train until
  it passes.

## 3. A rejected fixture's `@rule` is the code `pw check` reports

- **Cause.** The section above, corrections first.
- **Fix.** `checking_source.rs`'s
  `every_rejected_fixtures_rule_is_a_code_its_check_reports`: each rejected
  fixture, checked as its own program by `check_sources` (what `pw check`
  runs; it runs the declaration rules too, ADR-0090), is held to its
  `@rule` as written: a registered code, not an alias, not retired, among
  the codes reported, and its `@invariant` that code's symbol. The 37
  headers name their codes (`DEPRECATED_ALIASES`, applied once);
  `examples/history` keeps the sources as they were.
- **Claimed.** 74 of 74 fixtures pass it. Its control
  (`a_fixtures_header_is_held_to_its_code_as_written`) refuses an alias, a
  code not reported, another code's invariant, an unknown code and a
  retired one, and passes the right header.
- **Not claimed.** The alias table stays: no fixture declares an alias now,
  and its own rule removes it before codes are promised stable, which is
  the integrator's call. A fixture's `@expect-error` lines are not compared
  with the messages here.

## 4. `check_latency` bounds the checking thread's CPU time

- **Cause.** Train-7's local `just ci` and four workers' failed it at 2.77
  to 3.7 s against its 2 s ceiling at load averages of 40 to 110 (this
  machine, a dozen workers on 32 threads), where GitHub's quiet runners
  pass: a wall clock counts the time the thread waits for a CPU.
- **Fix.** The ceiling (2 s) and the rejected/clean ratio (5) bound the
  checking thread's CPU time, `clock_gettime(CLOCK_THREAD_CPUTIME_ID)`
  ("CPU time consumed by this thread", clock_getres(2), Linux since 2.6.12)
  through `libc`, on Linux and macOS: `check_sources` runs on the thread
  that calls it, and nothing else of the binary runs while it is measured
  (its two tests take turns). The wall clock is still measured and printed
  beside it for the E9 evidence; the control (more than 200 microseconds)
  reads the thread's clock, so a clock that reads nothing fails it.
  `libc` 0.2.189 was in Cargo.lock already (wasmtime, tokio, rustix and
  others) and in the local registry: a dev-dependency fetched nothing
  (`cargo test --offline`), Cargo.lock gains one edge; its declarations
  were read in its own source, its license is MIT OR Apache-2.0.
- **Claimed.** Here at load 33 to 43: cold 787.6 ms wall, 787.6 ms CPU.
  Beside 64 added busy loops (the one-minute load 46 to 61 over the run):
  cold 2535 ms wall, which the old bound fails, and 1214 ms CPU, which the
  new one passes; rejected 3080 ms wall, 1209 ms CPU. The recipe records
  both, the second beside twice as many busy loops as the machine has
  CPUs. Checking made pathological, the program checked five times over,
  fails it.
- **Not claimed.** CPU time is not the work: it stretches about 1.5 times
  under load (corrections first), so the ceiling stays loose. A bound that
  reads the wall clock again is not a mutant here: it survives on a quiet
  machine, and is shown by the measurement above instead. Elsewhere than
  Linux and macOS the wall clock stands in.

## 5. A browser suite runs on what its recipe built, or not at all

- **Cause.** The section above, corrections first. The three recipes:
  `e14-resume-gate` and `e14-signals` (no `run.sh`, no server build) and
  `e14-stream-ack` (no `run.sh`), each on whatever a worktree or a shard
  had built.
- **Fix.** `playwright.config.mjs` refuses `dist` unless it is a whole
  build: `run.sh` removes it first and writes its `sources.json` last, so a
  build stopped part-way, or none, has none; and its runtime is checked
  whether or not it is there. The three recipes build first.
  `scripts/tests/test_builds_before_suites.py`, in `just evidence-gates`,
  runs every recipe that runs the suite serving `dist` (82) as written, by
  `just` under the justfile's settings, in a tree of its own where each
  program it calls is a stand-in that logs its call (the build scripts,
  `cargo`, `pnpm`, `npx`, `python3`, `node`, `rustc`, `psql`, `git`, the
  built `pw`): each builds `dist` and the server before its first suite,
  and each build before it, failed in turn (188), fails the recipe with no
  suite after it.
- **Claimed.** All 82 hold it. Its control, recipes of its own: a `;`
  stops; a build behind `|| true` is named (the recipe passed, a suite
  ran); a suite with no build is named. The config, loaded by node beside a
  `dist` as each kind of build leaves it (`@playwright/test` stood in for):
  whole is served; stopped at 3c, at 3d, or never built is refused; so are
  another runtime and changed sources.
- **Not claimed.** The other builds' outputs (`dist-feed`, `dist-keyed`,
  `dist-kiokun`) were already refused without their sources, so a spec
  that needs one is skipped, not served. The benchmark harness's suite
  (`e14-contract`) serves its own builds, and checks them (`bench.mjs`).
  A mutation script builds for itself: each of the 51 that run `run.sh` and
  Playwright reads its exit status at each call, or calls it only to
  restore the build in a `finally`, by a reading of their text, not a test.

## 6. `master` holds no unnumbered ADR

- **Cause.** A track names its ADR by the placeholder until the integrator
  numbers it at the merge and rewrites each mention; a mention the rewrite
  missed survives, naming a decision no reader can find:
  `compiler/pw-core/tests/statements_separated.rs:176` from 2026-10-09
  (train-9 numbers it).
- **Fix.** `scripts/unnumbered_adrs.py` reads each tracked text file for the
  placeholder outside an inline code span (every document that describes
  the numbering writes it in backticks), and each `docs/DECISIONS` file
  named by it. `test_unnumbered_adrs.py` holds the repository to it where
  `GITHUB_REF` is `master`'s, as `test_evidence_present.py` tells master,
  or a train's (`track/train-*`), whose ADRs are numbered before it is
  pushed (ADR-0281); a track's branch keeps the placeholder, so the test is
  skipped there, and locally (`python3 scripts/unnumbered_adrs.py` reads it
  anywhere).
- **Claimed.** Today it names one line, the one above. Its control: the
  placeholder in backticks, a numbered ADR and a file that is no text name
  nothing; a bare mention, an evidence header and an unnumbered ADR file
  are named.
- **Not claimed.** A mention written in backticks to name a decision is
  read as one about the placeholder. Until train-9 lands, master's CI
  fails the repository's test on that one line.

## Other findings

- **`docs/evidence/E2D/corpus-enforcement.txt` is two months stale**:
  recorded 2026-08-07, it says 46 of 46 fixtures enforce their invariant,
  of a corpus of 74, with the alias column finding 3 retires. Nothing holds
  it current: `just evidence-corpus` is no `e<N>-` recipe, and the test
  writes the file only when asked (`PW_WRITE_EVIDENCE`). Not re-recorded
  here (evidence recorded locally).
- **`docs/evidence/E9/check-latency.txt` says 37 ms** (2026-08-08, on the
  machine `docs/environment/macbook.md` describes): the check now takes
  0.74 to 0.84 s here. `e9-latency` is `LOCAL_ONLY`, so neither a push nor
  `--stale` plans it.
- **A recipe's browser suite failing passes the recipe**: 81 of the 83
  recipes that run one end it `|| true`, recording its counts, and the
  run's summary (`ci_summary.py`) fails on a recipe's status, a survivor or
  a missing shard, not on a failed browser count. A recipe's evidence can
  say "1 failed" in a green run. The browser jobs run the whole suite and
  fail on it; the recipes' convention is the integrator's.
- **A change to `playwright.config.mjs` plans no browser recipe.** The
  planner reads a changed spec as a test its recipes run (ADR-0281), not
  the config every spec runs under: replayed, a change to the config's
  `dist` check plans `e14-evidence-integrity` alone, by its mutant's anchor.
  The browser jobs run the whole suite at a push; the recipes' evidence
  stays as their last runs recorded it.
- **`e14-stream-ack` runs its suite with `npx`**, which `run.sh`'s comment
  says resolves Playwright from its own cache where `pnpm exec` does not;
  left as it is.
- **Track reviews' `e14-reviews`** (in flight) runs its suite `if` its
  build passes and `else` prints FAILED and goes on: the recipe passes on
  a failed build, which `test_builds_before_suites.py` names when it merges
  (its `else` needs an `exit 1`).

## Acceptance

- `scripts/tests/test_mutation_baseline.py`, `Bytecode` (3);
  `test_ci_scripts.py`, `StaleEvidence` (5); `test_evidence_counts.py` (10);
  `checking_source.rs` (2 more); `check_latency.rs` (its 2, on the thread's
  CPU time); `test_builds_before_suites.py` (5); `test_unnumbered_adrs.py`
  (4, one held on master's CI and a train's).
- `scripts/evidence_integrity_mutations.py`, 32 mutants: 3 of finding 1, 14
  of 2, 5 of 3, 2 of 4, 3 of 5, 5 of 6. Run whole here twice on
  2026-10-15: 32 of 32 killed at load averages of 20 to 40, and 32 of 32
  again after its count of a skipped test was corrected, at 1 to 11; the
  memory bound stopped nothing. Checking made pathological fails the CPU
  ceiling; each other mutant fails a test of its own finding.
- The other scripts this change reaches: `dealt_by_time_mutations.py` (one
  anchor moved to `$stale`; its suite gained `StaleEvidence`),
  `derived_outside_mutations.py` and `mutation_bound_mutations.py` (their
  suite gained `Bytecode`), each run whole here on 2026-10-15: 48 of 48,
  10 of 10 and 16 of 16 killed, the moved mutant among them ("a push plans
  with no regard to the evidence", 2 of 72 tests fail), the bound stopping
  nothing; and
  `kiokun_inventory_mutations.py` (a mutant in `ci_plan.py`), on the
  development machine only, as its recipe is.
- `just e14-evidence-integrity` records them; its evidence of record is the
  next train's run.

## Alternatives

- **Bytecode: a `PYTHONPYCACHEPREFIX` of an empty directory** for the
  script and its children reads no `.pyc` of any source and needs no
  removal, but no standard library's either: every child would compile what
  it imports.
- **Stale evidence planned at every push**, a track's too: the 54 would run
  at each worker's push until a train records them. A track's run is not
  the record.
- **The count gate failing on every disagreement**: `just ci` would fail on
  master until a run records the two again, and nothing but a run may.
- **A wall-clock bound scaled by the load**: the load average lags the run
  by a minute, and a number divided by another the test does not control is
  no bound.
- **A static reading of each recipe's shell for a masked build**: the
  stand-ins run the recipes as `just` and `bash` do, which a reading would
  only approximate.

## Next

- Merge with or after train-9, which numbers `statements_separated.rs`.
- The integrator's answers to the questions below.

## Questions for the integrator

1. Remove `DEPRECATED_ALIASES`, now that no fixture declares an alias?
2. Re-record E2D's enforcement table (`just evidence-corpus`) and hold it
   current with a test, as `evidence_is_current.rs` holds the WIT?
3. Re-record `e9-latency` on this machine, with its CPU column, naming
   `docs/environment/pc-wsl.md`?
4. `--stale` at a train's push only: is that the scope you want?
5. `unnumbered_adrs` held on trains as well as master (one line, `held`):
   keep, or master only as ruled?
6. Should a failed browser count fail a recipe, or the run's summary?
7. Should a change to `playwright.config.mjs` plan the recipes that run
   the browser suite, as a changed spec plans those that run it?

## The integrator's answers (2026-10-15)

The seven follow-ups are accepted: remove alias translation from code
comparisons, re-record E2D and hold it current, re-record E9 latency on a
quiet machine with host/load and both clocks, plan stale evidence at train
pushes, hold unnumbered ADRs on trains and master, fail a recipe when its
suite fails, and plan browser recipes for changes to their shared config.
The implementation of the comparison, suite-status and planner follow-ups
is the evidence-integrity-2 track. E9 latency waits for a quiet window.
Train-10 carries this change; its integrated verification is pending.


## A report cannot inspect its own partial bytes (2026-10-16)

Verify 38824307495 at 83cdac27 failed its evidence recipe because the count
gate read that recipe's incomplete report stamped with the current commit.
Local runs on dirty source marked the same report stale and hid the cycle.
The downloaded artifact stops at its count test. Three behavioral regression
tests reproduce direct publication destroying a previous report, exposing
a partial report on a first run, and retaining partial bytes after failure.

The recipe now records to a temporary file beside its final destination,
so the final rename stays on one filesystem. It leaves complete prior evidence
visible to readers, and atomically replaces its final report only after all
commands succeed. On failure it retains prior evidence, emits the failed
run to stderr for CI's log, removes the temporary file and preserves failure
status. Count-test tracebacks are retained. New controls hold both atomic
publication and visible failure diagnostics. Results remain pending until
the full commands finish; no GitHub pass is claimed by this correction.


The first whole corrective run killed 33 of 34 controls: the diagnostic
control survived because its stand-in wrote directly to stderr, bypassing
the report being tested. The stand-in now writes its failure into the
captured report, as the real mutation script does. This retains the
independent diagnostic control; the failed run is not a pass.


Integration review also found that the planner recognized only direct
redirection to a report. Publishing by rename made this report disappear
from stale-evidence planning and fetching. A direct probe returned no
evidence files. The planner's existing evidence-files authority must also
recognize this publication form; an actual-recipe regression and an
independent omission control hold it. No separate output registry is added.


The complete final corrective suite kills 35/35 controls with no bound
stops. All 227 Python evidence tests run (225 pass, 2 intentional skips),
and formatting, anchors, case and diff checks pass. Four behavioral tests
and three independent controls cover publication, visible failures and
continued planner discovery of the published report.
[Commands, hashes and raw results](../evidence/E14/evidence-integrity-atomic.txt)
retain the failed-before history explicitly. Clean committed real-recipe
execution and assembled CI/verification remain pending at this checkpoint.

# ADR-0349: a failed browser test fails its recipe, and five more rulings on the evidence

Status: accepted under the owner's delegation of 2026-10-02, on the
integrator's rulings of 2026-10-15 on track evidence-integrity's questions
(its ADR, `ADR-0349-evidence-integrity.md`, unnumbered as this one is), and
the coordinator's ruling of the same day on `mutant-programs` (track
argument-invariants' question). Date: 2026-10-15. Milestone: E14; the
evidence's own harness (ADR-0245, ADR-0281, ADR-0292). A separate ADR, not a
dated section of its predecessor's: these are decisions of their own, each
with its tests and mutants, and both ADRs go to the integrator unnumbered.

## Corrections first

- **Track evidence-integrity's "81 of 83" was 83 of 85.** Read again with
  every recipe's body (section 3), 85 recipes ran `playwright test` (92
  invocations), and 83 swallowed its status: the two its count missed,
  `e10-browser` and `e10-close-bench`, have parameters with defaults, which
  the planner's header pattern stopped at, so `ci_plan.bodies()` held no
  body for them. The two that did not swallow were `e14-contract` and
  `e14-stream-ack`. Two recipes more, `e14-harness` and
  `e14-pleris-controls`, swallowed the benchmark's controls (`bench.mjs
  controls`, which grade each stack in a browser) the same way.
- **And so track evidence-integrity's `test_builds_before_suites.py` held
  82 recipes, not every one that runs the suite**: `e10-browser` and
  `e10-close-bench` were not among them. They are now (84), and both build
  before their suites.
- **Nothing the swallowing hid fails today in its spec, on its engine.**
  Each failure below ran here once more, its spec whole on its engine and
  the test five times over: all passed but one, "a follow reaches another
  reader of the page without a reload", whose five copies fail together
  because they follow `u-grace` on one host at once (`--repeat-each` runs
  the serial file's copies in parallel), which says nothing of the
  original; its spec passed 22 of 22. Its own comment records failures in
  CI's runs 37893301747 and 37918808029, the second the one below. Each is
  intermittent, or was fixed since: no test is fixed here.

## 1. What the swallowing hid (ruling 6, first half)

`python3 scripts/recorded_browser_failures.py` reads every version of every
evidence file the history holds, 2239 here: each commit's own change on the
first-parent line, a merge's against its first parent, and each side
branch's commit, in topological order, section by section. It names each
failed, flaky, interrupted or not-run browser test, failure listing or
failed benchmark control, once, where it was first committed. Its printed
historical lines are quoted with `|`: committing the report must not make
its next scan count its own quotes as a newly failed browser suite. A test
records a failed suite, commits its report, and still finds only the
original suite; a mutation removes the quote and is killed.


| first committed | file (recipe, recorded at) | what it recorded | today, here |
|---|---|---|---|
| 2026-09-25 ebd4696a, local | `E10/handlers-browser-suite-687aaf4.txt` (`e10-browser`, 687aaf4a) | [webkit] store.spec.mjs "only cart-related part ids update", 1 failed (full run 2) | 16 of 16, and 5 of 5: its cause was found that day (`public-fragment` given a host of its own, `playwright.config.mjs`); **still committed** |
| 2026-09-26 eeca1642, local | `E10/handlers-browser-suite.txt` (`e10-browser`, 12b3cb68) | [webkit] keyed-list.spec.mjs "changing one item's field replaces no sibling", 1 failed, 5 did not run (full run 1) | 16 of 16, and 5 of 5; **still committed** |
| 2026-10-02 cc2088e5, local | `E14/harness-T12.txt` (`e14-harness T12`, b64a9951) | "T12 pleris: unsafe FAIL": the unsafe change was not caught | ok (caught at check); recorded ok on 2026-10-03 |
| 2026-10-03 1d8f8b30, local | `E14/browser-suite.txt` (`e10-browser`, da0fcffb) | [firefox] lazy-handler.spec.mjs "two fast presses are one fetch", 1 failed | 13 of 13, and 5 of 5 |
| 2026-10-05 91665c0d, local | `E14/stable-layout.txt` (`e14-stable-layout`, 1a00c28a) | [chromium] performance.spec.mjs "gate 10: a thousand-item menu keeps a bounded rendering cost", 1 failed (32.7 s) | passed (1.8 s) |
| 2026-10-09 64abfcff, CI run 37918808029 (train merge of track/arrival-clock) | `E14/feed.txt`, `E14/string-invariants.txt` (4236e324) | [webkit] feed.spec.mjs "Load more shows the next page, and a post after it is shown over it", 1 failed, 10 did not run | 22 of 22, and 5 of 5; it was open in NEXT then (ADR-0281's amendment of 2026-10-09) |
| 2026-10-09 64abfcff, the same run | `E14/computed-holes.txt` (4236e324) | that one, and [chromium] feed.spec.mjs "a follow reaches another reader of the page without a reload": 2 failed, 13 did not run | 22 of 22; the five copies fail together (corrections first) |
| 2026-10-10 59f3351e, CI run 37993980886 (merge of track/refusals) | `E14/navigate.txt` (`e14-navigate`, 6964ee77) | [firefox] navigate.spec.mjs "a press made before the navigation is answered first, its answer first": waitForURL timed out (30 s) | 9 passed, 1 skipped; the test 10 of 10 |

Beside them, by design: `e10-close-bench` (LOCAL_ONLY) recorded E7 gate 8
failing in 3 of 8 runs (a long frame of 59 to 64 ms) on 2026-10-02 on the
Mac, "recorded, not retried"; 8 of 8 passed here today. No `flaky` was ever
recorded (the configuration retries nothing), and no `interrupted`. Not a
browser's: `E10/handlers.txt` was committed three times (344f3315,
9e53158e, a3e309d3) ending at a failed `cargo test` line: the recipe had
stopped, as `bash -e` stops it, and its part-written file was committed.

The reproductions ran at PORT 12041 between 23:48 and 00:24 UTC
(2026-10-15/16), load averages 3 to 20; the gate runs of `performance.spec`
alone in Chromium with `PW_PERFORMANCE=1`. Not read: the nightly's artifacts,
whose evidence no fetch keeps and whose download the owner has not
approved; a recipe failure the nightly swallowed is in none of the above.

## 2. A failed browser test fails its recipe (ruling 6)

- **The helper.** `scripts/run_suite.sh [-C DIR] [--runs N] -- COMMAND`
  runs a suite (in `spikes/own-renderer` unless `-C` names another
  directory), prints what a reader of the evidence needs of each run, its
  escape sequences taken out (each failure, its `Error:`, `Expected` and
  `Received`, each test's line where the list reporter prints them, a
  spec's `EVIDENCE` lines, the counts), and exits with the status of the
  first run that failed, once every run has run. A run that exits 0 and
  passed no test fails too (every test skipped, or none selected, is no
  evidence of passing behaviour); a run that failed before printing any of
  those lines shows its last five. Counts of zero with leading zeros are
  refused too: `bash scripts/run_suite.sh --runs 00 -- false` originally
  exited 0 having run nothing; the invalid-call test and a mutation hold
  the positive count check.
- **The recipes.** All 92 invocations run through it, in their recipes'
  `{ ...; } > file` groups under the justfile's `bash -euo pipefail`: a
  failed suite stops its recipe there, its output already in the file. The
  loops that counted runs are `--runs`, so every run is recorded and the
  recipe fails after the last: `e10-browser`'s three full runs,
  `e14-stream-ack`'s twenty, `e10-close-bench`'s RUNS (its note moved
  before the runs, which a failure would otherwise cut off). The
  per-engine loops (`e14-resume-gate`, `e14-signals`,
  `e14-idempotent-commands`, `e14-optimistic`) stop at the first engine that
  fails. The benchmark's controls are no longer `|| true`: the first task
  whose controls fail fails its recipe. Formats changed a little: the
  filter is one, `e10-browser`'s runs are headed `== run i of 3`, and
  `e14-stream-ack`'s twenty a count each.
- **The gate.** `scripts/suite_status.py`, held by `test_suite_status.py`
  in `just evidence-gates`, reads each recipe's lines (a `\`-continued line
  as one) as shell words and operators and finds each `playwright test` and
  each `bench.mjs controls`. One is refused where a `playwright test` is not
  the command the helper runs after its `--`; where it, or a subshell,
  group, loop or `if` holding it, is followed by `||`, `&&`, `|`, `&`, or by
  `; true` or `; :`; where it is a condition (`if`, `elif`, `while`,
  `until`), negated, or inside a command substitution; where its recipe
  says `set +e`, is a shebang script, or runs its line with `just`'s `-`;
  or where the justfile's shell is not bash with `-e` and `pipefail`.
  Floor: as many recognised as there are occurrences of `playwright test`
  in the recipes' text, which a suite hidden in a quoted string fails.
  Today 92 of 92, and 2 `bench.mjs controls`; on the justfile of 83cdac27
  it refuses 90 (186 refusals, its old header reading 90).
- **The behaviour.** `test_suites_fail_their_recipes.py` runs each recipe
  that runs a browser suite (87) as `test_builds_before_suites.py` does: by
  `just`, under the justfile's settings, in a tree of its own where each
  program is a stand-in. Each suite call, failed in turn (139), must fail
  its recipe, and an evidence file the recipe wrote must say so. Its
  control: the helper's form passes; `|| true`, the old subshell piped to a
  filter, and a status read as a condition are named; a filter that drops
  the count is named as keeping no evidence of the failure.
- **Not claimed.** A known failure is a job's `--known` at the fetch, never
  a recipe's (ADR-0281): a recipe whose suite fails now fails its shard,
  and its evidence is not fetched until it passes. A recipe's other
  commands' statuses are what `bash -euo pipefail` makes them; the 15 `||
  true` left in the justfile are reports (`grep -c`, a `sysctl` absent on
  Linux, a closing `grep` of what a recipe printed) and a `kill` in a
  trap, none a suite's.

## 3. The planner reads every recipe (found on the way)

`ci_plan.HEADER` stopped at a parameter's default (`[^:=]*` before the
colon), so `e10-browser`, `e10-close-bench` and `e14-harness` had no body:
nothing they run planned them, their evidence files were never judged
current or stale, and a shard dealt one alone set up neither browsers nor
the build. The pattern reads each parameter and its default, quoted or
not. And `needs()` gives browsers to a recipe that runs `bench.mjs`, whose
controls, no longer swallowed, would fail where none were installed. With
its body read, `e10-browser` (CI's, not LOCAL_ONLY) is planned by a change
to `run.sh`, `keyed-store.sh` or `feed.sh`, and by any spec (section 4).

## 4. The suite's configuration plans what runs under it (ruling 7)

A change to `spikes/own-renderer/playwright.config.mjs` plans each recipe
that runs a suite from that directory, in its own lines (by the helper, or
after a `cd`) or by a mutation script that runs Playwright there and names
the directory: 87 recipes, where 83cdac27's planner planned none for a
change at its line 100. The benchmark harness's configuration is under
`benchmarks/`, whose changes already plan each recipe that names it. Two
holes of the same rule (ADR-0281: a changed spec plans what runs it):
- a module the specs import, `e2e/accessibility-rules.mjs` (three specs),
  planned nothing; it now reaches the specs that import it;
- `e10-browser`, which runs every spec, was planned by no changed spec; a
  changed spec or module of `e2e/` plans each recipe that runs the suite
  naming no spec.

## 5. Every program a mutation script mutates (the coordinator's ruling)

`scripts/mutant_programs.py` named four programs (store, feed, kiokun,
kiokun-site), and a mutant elsewhere was skipped unread: the benchmark's
store (`bench_contract_mutations.py`), a rejected fixture
(`evidence_integrity_mutations.py`'s R-004), and `examples/demo`, whose
mutants track argument-invariants checks in its own script. Now:
- a mutant's root is its source's directory, or the source alone where its
  directory declares a module twice (a corpus of fixtures, each its own
  program, E2B);
- a root's program is the standard packages, the root's sources, and every
  source they import, transitively, each import the nearest source that
  declares its module, by the steps between their directories (the same
  directory, `examples/domain.pw` above, `examples/lib/` beside); a tie, or
  no source, fails; a fixture's own directory is no source of its imports;
- a mutant is checked in every program that holds its source: a library's
  in each importer's, a standard package's in all;
- floor: every mutant of a `.pw` source is judged in at least one program,
  and a gate that judged none fails.
Here: 239 mutants judged (237 before), in 9 programs, 344 checks, none
failing; 67 s (58 s before). `examples/demo`'s program (35 files: its
sources, `domain.pw`, `lib/Recommender.pw`, the packages) checks clean,
ready for track argument-invariants' mutants when it lands.

## 6. No comparison translates an old number (ruling 1)

`canonical_code` translated each of the corpus's old numbers wherever a
fixture's `@rule` was compared: four tests of `checking_source.rs`,
`rules.rs`'s corpus test and `codes::rule_status`. It is gone with its
re-export: a header's code is read as written, so an old number is an
unregistered code (`rule_status` says `Unknown`). `DEPRECATED_ALIASES`
stays in `diagnostics.rs`, by its name and form, only where it is printed:
track guide's `scripts/guide_diagnostics.py` reads it by that name for the
reference page's "Old numbers", its test `compiler/pw-cli/tests/guide.rs`
counts a row for each, and a refused header's message names the code an old
number became. The registry's own tests of the list stay. Held by
`diagnostics::tests::no_comparison_translates_an_old_number` (no Rust source
names the translating function; none but those named reads the list;
`guide.rs` is allowed for when the two tracks merge) and
`codes::tests::an_old_number_is_no_registered_rule`.

## 7. The enforcement table is held to the corpus (ruling 2)

`docs/evidence/E2D/corpus-enforcement.txt` (2026-08-07: 46 of 46, of a
corpus now 74, with a column of old numbers) is re-recorded by `just
evidence-corpus`: **74 of 74** fixtures enforce their declared invariant.
`checking_source.rs`'s `the_committed_enforcement_table_is_what_the_corpus_checks_now` writes it under `PW_WRITE_EVIDENCE=1` and otherwise holds the
committed file to the table it computes, as `evidence_is_current.rs` holds
the WIT: `just ci` fails while it is stale, its repair in the message. The
file names no commit, so the planner judges it by that test.

## 8. e9-latency names its machine (ruling 3)

The recipe writes a `host:` line (architecture, CPU model and count) naming
the record `scripts/record-environment.sh` writes for the host
(`macbook.md`, `pc-wsl.md` where the kernel names Microsoft, `linux.md`),
the load averages before and after the measurement, and says each row is
the wall clock with, in brackets, the checking thread's CPU time that the
test's ceiling bounds. **Not run**: the measurement is left for the
integrator's quiet window; `docs/evidence/E9/check-latency.txt` still says
37 ms (2026-08-08, the Mac). `test_e9_latency_recipe.py` runs the recipe with
its programs stood in for and holds its evidence to this host's record and
its load lines.

## 9. Rulings 4 and 5

Accepted as built by track evidence-integrity: `--stale` at a train's push
only; `unnumbered_adrs` held on trains as well as master. Nothing changed.

## Other findings

- **The first full `just ci` caught Clippy's `manual_contains` lint** in
  the table's raw-code membership check. `caad384d` uses `contains` for
  the same comparison; targeted `pw-core` Clippy and the table's test pass.

- **The first resumed `just evidence-gates` caught a stale stand-in**:
  `evidence-corpus` now checks its enforcement summary, but
  `test_evidence_gates.py` supplied `fixture corpus`. Its stand-in now
  supplies the summary's actual shape; all 9 tests pass. No gate was
  weakened.
- **A unittest failure count includes failing subtests**: the own mutant
  "the gate reads no `||`" causes six AssertionErrors in subtests of one
  test method, among six methods run. It was a real test kill. The own
  mutation script now says "failure reports in N tests run" rather than
  incorrectly claiming six of six test methods failed.

- **`feed.spec.mjs`'s follow tests share `u-grace` on one host**: "a follow
  shows before the server answers" and "a follow reaches another reader"
  read a count before and after their own click, in a file run serially;
  copies of either run at once (`--repeat-each`) break each other. The
  second's comment records its failures in CI's runs; whether another
  suite on that host follows `u-grace` meanwhile was not looked into.
- **Each recipe run is a full Playwright start**: a spec of one test on one
  engine took 50 to 60 s here, nearly all the hosts starting (every
  `webServer` the configuration declares starts for any spec).
- With every recipe's body read, `e10-browser` (three full runs in three
  engines) is planned by most pushes that touch the spike's builds or
  specs, as its rule says; it was planned only by the nightly before.

## Acceptance

- Tests: `test_evidence_gates.py` (9; its enforcement-table stand-in updated to the new summary), `test_run_suite.py` (8), `test_suite_status.py` (6),
  `test_suites_fail_their_recipes.py` (2: 87 recipes, 139 suite calls),
  `test_builds_before_suites.py` (now 84 recipes, 194 builds),
  `test_recorded_browser_failures.py` (3), `test_ci_scripts.py` (3 more),
  `test_mutant_programs.py` (9, 5 new), `test_e9_latency_recipe.py` (1);
  `pw-core`: `no_comparison_translates_an_old_number`,
  `an_old_number_is_no_registered_rule`,
  `the_committed_enforcement_table_is_what_the_corpus_checks_now`.
- `scripts/evidence_integrity_2_mutations.py`, 35 mutants: 18 of ruling 6,
  2 of section 3, 4 of ruling 7, 6 of the coordinator's, 2 of ruling 1, 1 of
  ruling 2, 2 of ruling 3. Run whole on 2026-10-16 at `1dbf5e65`: **35 of 35 killed**;
  the memory bound stopped no process. Both new controls fail their
  behavioural regressions: a committed census is not a new failed suite,
  and a count of zero with leading zeros cannot pass.
  Repeated whole after the Clippy repair, at `caad384d`: **35 of 35**
  again; the memory bound stopped no process. The raw evidence now records
  that repaired code head.
- The other scripts this change reaches, run whole on 2026-10-16 at
  `b7e400c6`: `dealt_by_time_mutations.py` **48 of 48**,
  `evidence_integrity_mutations.py` **32 of 32**,
  `mutation_bound_mutations.py` **16 of 16**, and
  `kiokun_inventory_mutations.py` **27 of 27** killed. The memory bound
  stopped no process. The later code commit `1dbf5e65` adds the two own
  report/zero-count regressions and corrects comments; the implementations
  these four scripts mutate are unchanged.
- `just evidence-gates`: **216 tests, 2 intentional skips**, passing on
  this branch; 87 recipes and 139 suite calls failed in turn, 84 recipes
  and 194 builds failed in turn. `mutant-programs`: 239 of 239 source
  mutants judged, in 9 programs, 344 checks, no failures.
- `just e14-evidence-integrity-2` records them in
  `docs/evidence/E14/evidence-integrity-2.txt`; its evidence of record is
  the next train's run.

## Alternatives

- **A failed count failing the run's summary** (`ci_summary.py` reading the
  evidence), the other half of the question: ruled out; the recipe's job is
  evidence of passing behaviour.
- **Each recipe's own filter kept**: one filter keeps every recipe's
  evidence alike and is one place to read; the filters differed only in
  what they dropped.
- **A census that stops at its first failed run**: `e10-close-bench`
  exists to keep every run, so `--runs` runs all and fails after.
- **A static gate alone**: the stand-ins run the recipes as `just` and
  `bash` do, which a reading only approximates; the reading names each
  invocation and its floor. Both are kept.
- **Programs from the builds' command lines** (the justfile's `pw check`
  globs): a loop's `$f` and a script's own lists hide some; imports are
  what a program is.

## Next

- Merge with track evidence-integrity, after train-9.
- `just e9-latency` in a quiet window, and commit its evidence.
- Each failure of section 1 is the integrator's to route; none reproduced.

## Integrator rulings and merge notes

1. The benchmark's controls (`e14-harness`, `e14-pleris-controls`) fail
   their recipes too; the integrator confirmed this scope on resumption.
2. `e10-browser` stays scheduled in CI, as the integrator confirmed. With
   its body read it is planned by pushes that touch the spike.
3. The two committed files that still record a failure
   (`E10/handlers-browser-suite.txt`, `handlers-browser-suite-687aaf4.txt`,
   e10-browser's of September) are preserved as historical evidence. The
   current recipe records its own new file.
4. Track guide's page says a fixture's `@rule` "may still name" an old
   number; after ruling 1 none may: the integrator will amend its sentence
   when the tracks meet on train10.


## Train-10 integration (2026-10-16 UTC)

The reviews, guide, argument-invariant and scheduling recipes use the shared
runner. The guide builds both main and tutorial artifacts and the servers.
Behavioral injection now covers 143 suite failures across 91 recipes and 203
build failures across 88 recipes; all propagate. Python runs 223 tests with 2
intentional skips. Guide 15/15 and both strict-code tests pass. The corpus report is
current at 79/79, and the complete restarted own suite kills 35/35 controls
with no bound stops. Its table-drift control alters a stable fixture row.
[Raw integration results](../evidence/E14/train10-evidence-integrity.txt)
include commands and input hashes. Full assembled CI and train verification
remain pending. Quiet-host latency has not been rerun.


## Found at train-10's first verify (2026-10-17)

- **A recipe the planner could not read before ran on CI for the first
  time, and could not run there.** This track taught the planner to read a
  recipe with parameters (`e14-harness TASK="T08"`) and took its `|| true`
  away; on train-10's verify 39054436657 `e14-harness` failed at its first
  sandbox, whose stack's dependencies install from the machine's pnpm store,
  offline by design, and a runner's store holds no Next.js or SvelteKit.
  It is `LOCAL_ONLY` now (ADR-0281's fifth decision); its Pleris stack runs
  on CI as `e14-pleris-controls`, which passed.
- **A browser recipe of three runs now fails on one flaky run.**
  `e10-browser` failed there on one WebKit test of its first run,
  `store-accounts.spec.mjs:173` ("an address chosen goes back to the store,
  its estimate the chosen one's", a 30 s timeout), its other two runs
  green: a flake, recorded; the train's next verify runs it again.

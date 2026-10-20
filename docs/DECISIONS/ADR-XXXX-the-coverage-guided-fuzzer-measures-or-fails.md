# ADR-XXXX: the coverage-guided fuzzer measures, or it fails

Status: proposed by track `fuzz-floor` (2026-10-20) for the integrator; local
gates and the track's recipe recorded below; not yet in a train.
Date: 2026-10-20. Milestone: E9C (the fuzzer), E14 (the track's recipe).
Track `fuzz-floor`, on `af5c1758` (`track/train-11`'s tip, not on `master`
`32757966`).
Amends E9C: `tools/fuzz`, `just fuzz`, `docs/evidence/E9C/coverage-guided-fuzzing.txt`.

## Corrections first

- **Installing `llvm-tools` did not fix it.** The first account of the defect
  (HANDOFF-PC, 05:00Z) was that the component was absent. Reproduced at
  `af5c1758` with the component installed, `pw-fuzz --iterations 60`: all six
  targets "0 covered regions ... 0 finding(s)", `6 targets, 360 executions, 0
  findings`, exit 0. The cause is `llvm_bin()`: it looked for the tool at one
  machine's `~/.rustup/toolchains/1.97.1-aarch64-apple-darwin/...`, and
  `regions()` read both a tool that cannot be started and one that fails as
  "no regions". (The integrator's own correction of 05:10Z says so.)
- **The committed E9C evidence is the Mac's, and is not re-recorded here.** Its
  48 to 2,262 regions were measured where that path existed. This track records
  nothing by hand into `docs/evidence/E9C/`; what it records is its own
  recipe's file, `docs/evidence/E14/fuzz-floor.txt`, written by `just
  e14-fuzz-floor`. Whether E9C's file is made again is question 1.
- **A fixed number of inputs a target, one target after another, with the
  report printed at the end, is the wrong shape for a night.** With
  `--iterations` large and `timeout 8h`, the first target would have taken the
  night, the other five nothing, and a run killed by `timeout` would have left
  an empty log. Section 3.
- **A profile of 640 KB was left for every run that failed.** A campaign that
  fails often would have filled the disk (71 GB free at the time of writing, 91
  GB half an hour before, other workers' builds). Section 3.

## 1. Where the tool is

`llvm-profdata` is a program of the rustup component `llvm-tools`. It is at
`<sysroot>/lib/rustlib/<host>/bin/llvm-profdata` of the toolchain that builds
the harness: `rustc --print sysroot`, and the `host:` line of `rustc -vV`, both
asked in the repository's root, so that the toolchain file applies to them as it
does to the `cargo build` of the harness. `PW_LLVM_BIN_DIR`, when set and not
empty, names the directory instead. `RUSTC` is honoured as `cargo` honours it.
Nothing is a path of one user's. The tool is looked for, and run (`--version`),
before anything is built.

Primary sources: the channel manifest that came with the toolchain
(`lib/rustlib/multirust-channel-manifest.toml`) names the package
`llvm-tools-preview` and renames `llvm-tools` to it (`[renames.llvm-tools]`), as
it does `clippy` and `rustfmt`, so `llvm-tools` is the name for the toolchain
file; `rustup component add llvm-tools` was accepted ("up to date") by rustup
1.29.1. Its xz archive is 39,156,716 bytes (x86_64) and 31,450,908 (aarch64) (a
`HEAD` of `static.rust-lang.org/dist/2026-07-16/llvm-tools-1.97.1-*.tar.xz`;
nothing downloaded); installed, 26 MB of programs and a 199 MB `libLLVM`.

## 2. The floor

The fuzzer exits 2, with a message that names what is missing, where it
measured nothing:

- `llvm-profdata` is not at the place (the message names the path, `rustup
  component add llvm-tools` and `PW_LLVM_BIN_DIR`), or does not run;
- `llvm-profdata` fails on a run's profile, or lists no executed function in it
  (a run of the instrumented harness executes at least its `main`, so an empty
  listing is an output the parser no longer reads: the first version's 69
  regions for a compiler);
- a target ends with no covered region (also `--iterations 0`, and a target
  whose every run failed: its finding is still printed).

`regions()` returns a `Result`; `fuzz_one` and `campaign` propagate it. An
argument it does not know, and a count that is no number, are errors (a typo in
`--iterations` had been the default of 2000). Exit status: 0 clean, 1 a finding,
2 nothing was measured or it was called wrongly. `just doctor` names the
component (a required item: the tests need it), and `rust-toolchain.toml` lists
it.

## 3. A campaign

- **Progress.** Each target says on stderr, one write a line (six processes may
  append to one log), when it starts, every 30 seconds
  (`[parser-to-hir] 5400 execs, 1612 covered regions, 0 finding(s), 17.9
  execs/s, 301s`), each finding when it is found (`FINDING [target] panic on 412
  bytes: thread 'main' panicked at ... (saved: .../finding-0.bin)`), and when it
  is done (`done [target] ...`).
- **`--only TARGET[,TARGET...]`** runs the named targets alone: a process for
  each, side by side, so that all six progress through a night. An unknown name
  is an error that lists the six.
- **`--seconds S`** ends each target at a wall-clock limit (one input always
  runs), beside `--iterations`; the first reached ends it, and the final table
  is printed.
- **`--hang S`** (60): an input still running after S seconds is a finding
  (`hang`), and is killed. Without it one input that never ended would have
  stalled a process for good, and silently.
- **A finding is what failed and where**, not the input that showed it: its
  kind plus the first line of the harness's stderr (a panic's is its file, line
  and column). The same panic reached by a thousand lengths of input was a
  thousand findings, each with a file; it is one. The first input that showed it
  is kept.
- **Where findings are written**: `target/fuzz/<target>/finding-<n>.bin` under
  the repository root the fuzzer runs from (a raw mutated input, at most 64
  KiB), by `fuzz_one`, which empties `target/fuzz/<target>/` as it starts: **a
  later run in the same tree deletes the earlier run's findings.** Run one again
  with `target/fuzz/build/debug/pw-fuzz-harness <target> <file>` (exit 101: a
  panic, its message on stderr).
- The profile of a run that failed is removed, as a survivor's is.

## 4. What this makes CI do

Read from the workflows and the scripts, and measured with the planner; not yet
seen on a runner (the first push's `ci` run is the proof).

- **Workflows: unchanged, and the toolchain file does the work.** Every job
  uses `dtolnay/rust-toolchain@stable` (`ci.yml`: `build` on x64 and arm64,
  `components: rustfmt, clippy`, and `supply-chain`; `verify.yml`: the shards and
  the three browser jobs). The action runs `rustup toolchain install stable
  --profile minimal` with the components it is given and `rustup default
  stable` (its `action.yml`, read through `gh api`), and does nothing about
  `rust-toolchain.toml`. `stable` is not 1.97.1 (by the action's own date
  arithmetic it is 1.99 today), so the first `cargo` of a job in the checkout
  makes rustup install 1.97.1 with the file's profile, components and targets,
  as the `wasm32-unknown-unknown` story of ADR-0245 shows it already does
  (rustup 1.29.1 has a `rustup set auto-install` setting; its default on a
  runner was not read). **Each such job now fetches `llvm-tools` too: 39.2 MB
  (x86_64), 31.5 MB (arm64), 225 MB unpacked**, in `ci.yml`'s three jobs and in
  each of the `verify.yml` jobs that runs `cargo`. `components: rustfmt,
  clippy` in the workflows is for `stable`, not for 1.97.1, and is not touched.
- **`just ci`** runs the 43 tests of `pw-fuzz` in `cargo test --workspace` (2.6
  s; `the_real_llvm_profdata_reads_a_real_profile` is the one that needs the
  component, and says what to install where it is missing) and the 5 doctor
  tests in `evidence-gates`.
- **`just doctor`** names `llvm-tools` among the required items. CI does not run
  it.
- **The planner** plans only `e<N>-` evidence recipes; `fuzz` and `fuzz-long` are
  not, so no CI job has run the fuzzer, and none does but this track's
  `e14-fuzz-floor`, which runs 30 inputs of each target (an instrumented build of
  the compiler: minutes on a runner). Against this track's base `af5c1758` the
  planner plans one recipe, `e14-fuzz-floor`. **A push measures from `git
  merge-base origin/master`** (`master` `32757966`), and this branch carries
  train-11's unmerged work: `ci_plan.py --shards 9 --changed 32757966 HEAD
  --current` plans **190 recipes in 40 shards**. The integrator may cancel that
  run and dispatch `e14-fuzz-floor` alone (`workflow_dispatch`, `recipes`), as
  train-6's and train-10's precedent and the `ci-diagnostics` ADR say.

## 5. Evidence

- **Before** (`af5c1758`, `cargo run -q -p pw-fuzz --bin pw-fuzz -- --iterations
  60`, llvm-tools installed, load average 80 to 90): `parser-to-hir 60 execs 0
  covered regions 0 finding(s)`, the same for the other five, `6 targets, 360
  executions, 0 findings`, exit 0.
- **After** (the same command, this track's first commit): `parser-to-hir 1353`,
  `resolution-to-typing 4018`, `labels-and-effects 2715`, `exhaustiveness 2276`,
  `manifest-decoding 48`, `resume-compatibility 58` covered regions, exit 0,
  about 50 s for 360 executions, the harness already built. The two resume
  targets' 48 and 58 are the Mac's committed figures (E9C) exactly; the compiler
  targets differ from E9C's (1,249 to 2,262) and are not compared: another commit
  of the compiler.
- **The recipe**, `just e14-fuzz-floor` at `5e59af97`, recorded in
  `docs/evidence/E14/fuzz-floor.txt`: 43 tests in `pw-fuzz` and 5 in the doctor's
  file pass; a 30-input run covers 1,138, 3,265, 2,440, 2,191, 48 and 56 regions
  (4.8 to 6.0 executions a second a target at a load of 130); with
  `PW_LLVM_BIN_DIR` naming an empty directory it exits 2 before it builds
  anything; **57 of 57 mutation controls killed** (`scripts/fuzz_floor_mutations.py`:
  where the tool is, what a profile says, a measurement that did not happen read
  as nothing, the floor, the campaign's limits, saying and findings, the
  doctor, the toolchain file).
- **Gates**: `evidence-gates` 469 tests OK (2 skipped) with the first commit,
  `cargo clippy -p pw-fuzz --all-targets -- -D warnings` clean, `mutation_anchors`
  3,577 mutants each matching once, `case-check` OK. `just ci`: see the log in the
  track's progress file.
- **A cost**: one execution is about 10 ms of the harness and 30 ms of
  `llvm-profdata show --counts --all-functions` (a 630 KB profile becomes
  630 KB of text, 3,692 functions of which 535 ran), and the driver parses it.

## Claimed, and not

- **Claimed**: on this machine (Linux x86_64) the fuzzer finds the tool in the
  active toolchain and measures; the lookup's path is built from what `rustc`
  says, which the tests check against a stand-in `rustc` that names another
  sysroot and host; each way of measuring nothing that the tests construct
  stops the run with status 2 and a message; 57 mutants of those and of the
  campaign's pieces are killed.
- **Not claimed**: that it works on the Mac or on a runner (the layout
  `lib/rustlib/<host>/bin` is the same by construction and unverified); that
  E9C's figures are current; that the fuzzer finds anything (0 findings in 540
  inputs here is nothing); that a region count means the same across LLVM
  versions; that `--hang` stops a hang's children (the harness starts none); that
  a restart explores anything new (the generator's seed is fixed: question 3);
  that `llvm-profdata`'s text output is stable across rustc versions (the floor
  turns a change into a failure, and the sample in `tests.rs` is LLVM 22.1.6's).

## Next

- The overnight campaign (six processes, one a target, nice 19, 8 hours) is this
  track's last step; its record, and how to read its log and findings in the
  morning, are in `tracks/fuzz-floor.md` of the handoff. A finding is a defect
  of the compiler or the runtime for another track: a regression input under
  `examples/robustness/regressions/` where the target is a compiler one.
- A `--seed`; keeping a target's findings when it restarts; a minimiser;
  fuzzing in the process, not one process an input.

## Questions for the integrator

1. `docs/evidence/E9C/coverage-guided-fuzzing.txt` is the Mac's. Re-record it
   (`just fuzz` where the tool is), or take the figures from `e14-fuzz-floor`?
   Not touched here.
2. `llvm-tools` in the toolchain file costs every CI job 31 to 39 MB of
   download. Accept, or install it only in the jobs that run the fuzzer and
   its tests (`cargo test --workspace` is every job's)?
3. The generator's seed is fixed (`0x5EED_1234`): a second campaign repeats the
   first until it passes it. A `--seed` is three lines; not added.

# ADR-XXXX: a mutation script's mutants run in parallel slots

Status: proposed by track `parallel-mutants`, for the integrator's
numbering. Date: 2026-10-15. Milestone: E14; the verification run's harness
(ADR-0245, ADR-0281, ADR-0292).

## Context

- **Mutation controls are the project's core evidence, and its slowest
  part.** 261 `scripts/*_mutations.py` scripts (2882 mutants,
  `mutation_anchors.py` of 2026-10-14) each apply one mutant in place,
  rebuild, run its tests and restore, one mutant at a time.
  `store_accounts_mutations.py` took 37 minutes on the 32-thread PC (37 of
  37 killed); CI's verification shards take 4-5 hours and more, and reached
  GitHub's 345-minute limit a job (train-7's `e14-refusal` shard ran past
  4.5 h). Every track waits on them.
- **What the scripts share** (surveyed 2026-10-14, every script read by its
  syntax tree):
  - No runner. The scripts import the standard library alone (`sys`,
    `subprocess`, `re`, `pathlib` in each; `os` in 103, `signal` in 98,
    `shutil` 5, `random`, `glob`, `base64` one each). Each holds its loop.
  - **One module reaches every script**: each imports `mutation_baseline`
    first in its `main()` (ADR-0292; held by its tests). The import keeps
    what each command says and starts the memory bound, which wraps the
    script's output to read each verdict.
  - Each script's shape: `ROOT` its own tree (`Path(__file__).parent.parent`),
    a module-level list `MUTANTS` of `(what, [suite,] path, anchor,
    replacement)`, a baseline per suite (`baseline (<suite>): N passed, F
    failed`), then for each mutant in the list's order the anchor counted,
    applied, the suite run with `cwd=ROOT`, the source restored in a
    `finally`, and its verdict printed, then `<k> of <n> mutants killed`,
    then (at exit) the bound's line. 250 scripts loop `for ... in MUTANTS`;
    10 run their baselines for the suites their mutants name, in the order
    of the suites' names (`sorted({m[1] for m in MUTANTS})`);
    `row_reads_mutations.py` runs the mutants whose description holds one
    of its arguments.
  - Verdict lines: `{what}: {v}` (203 scripts), `{what} [{suite}]: {v}`
    (56), `{stack}: {what}: {v}` (bench_contract), `{gate}  {what}: {v}`
    (e9_value), `{what}: NOT RUN (..)` (messages, notifications, uploads),
    `DID NOT BUILD, so it shows nothing` (runtime_size).
  - Their readers: `evidence_survivors.py` (`: SURVIVED`, `ANCHOR NOT FOUND
    EXACTLY ONCE in`, `^(\d+) of (\d+) mutants killed`), `ci_summary.py`
    (`SURVIVED`, `ANCHOR NOT FOUND`, `memory bound: ` lines),
    `mutation_bound.Lines` (verdicts, `baseline` lines), and the recipes'
    `grep`s. The recipes keep the script's standard output alone as
    evidence (`{ ..; python3 scripts/x_mutations.py; } > docs/evidence/..`).
- **What two copies of one script running at once would share**, found in
  the tree:
  - **Ports.** The browser suites' hosts are `PORT + offset`, the largest
    offset +182 (`STORE_PAYMENTS_PORTS`, `playwright.config.mjs`). Two
    scripts start hosts on fixed ports instead:
    `messages_mutations.py` (`PW_MESSAGES_PORT`, 7141 where unset) and
    `notifications_mutations.py` (`PW_NOTIFICATIONS_PORT`, 6100). The
    server's tests bind `127.0.0.1:0`.
  - **Temporary directories of fixed names**: `one_comparison.rs`
    (`pw-one-comparison-control`, `-ok`, `-hash`, `-substring`),
    `name_keyed_maps.rs` (`pw-name-keyed-control`), and
    `tests/support/mod.rs`'s `temp_dir().join(name)`.
  - **git**: `marko_adapter.rs` asks `git ls-files '*.marko'` of the tree;
    with no git it would pass having read nothing.
  - **`target/`, by path**: the Playwright config starts
    `../../target/debug/pw-dev-server`, `run.sh` and `feed.sh` copy
    `$REPO_ROOT/target/wasm32-unknown-unknown/..`, and
    `runtime_size_mutations.py` writes `ROOT/target/..`. Nothing here sets
    `CARGO_TARGET_DIR`, nor does CI.
  - **PostgreSQL**: each test's schema is its process's
    (`pw_test_{pid}_{test}`, `pw_identity_{pid}_..`): shared safely.
  - **`node_modules`**: what is installed, read; the Next.js and SvelteKit
    builds (`bench_contract_mutations.py`) may write their tools' caches
    into it.

## Decision

1. **`PW_MUTANT_SLOTS=N`** asks for N slots. Unset or 1, nothing changes:
   every script runs as it did, in place. A number below 1 or not a number
   stops the run, said.
2. **Above 1, the tree is never mutated.** `mutation_baseline`'s import
   calls `mutation_slots.engage()` before it starts the memory bound; in
   the tree, that runs the slots (`scripts/mutation_slots.py`) and ends the
   process with the run's code. The script's own loop never runs in the
   tree, so a run stopped from outside leaves no mutant there.
3. **A slot is a copy of the tree**, `target/mutant-slots/<k>` (or under
   `PW_MUTANT_SLOT_DIR`), kept from run to run:
   - every file of the tree as it is, tracked, untracked and ignored, but
     its `.git` and `target/`; each `node_modules/` a link to the tree's;
   - brought to the tree at each run **by content**: a file that differs is
     written (its time now, so that cargo builds it again), one that does
     not is left (so that cargo's last build of it stands), what the tree
     does not hold is removed. What a stopped run left in a slot, a mutant
     or bytecode, is undone at the next;
   - its own git: the tree's commit, its index that commit's files, its
     objects read from the tree's repository (`objects/info/alternates`),
     registered nowhere in the tree's repository;
   - its own `CARGO_TARGET_DIR`, `<slot>/target`: where the repository's
     scripts look for what cargo built, and built from the slot's own
     paths. **A slot's target is not seeded from the tree's**: a copy of
     the tree's artifacts beside sources with the tree's times would look
     fresh to cargo, and a test binary built there reads its fixtures from
     the tree's `CARGO_MANIFEST_DIR`, not the slot's;
   - its own ports, `PORT + k x 200` (`PORT` 3141 where unset), and its own
     `TMPDIR`, emptied at each run;
   - and the bound of ADR-0292 in each slot's script, as in any script: the
     bound is a process's, and a mutant's verdict does not change with the
     number of slots.
4. **One run uses the slots at a time**: a run holds `<slots>/.lock`; a
   second waits for it, said on standard error.
5. **The mutants are dealt.** Slot 0 first takes the first mutant of each
   suite (so that its baseline, in the 10 scripts whose baselines follow
   their mutants' suites, is the script's whole); then each mutant goes to
   the slot holding fewest of its suite, then fewest in all. A slot runs
   the script's own copy in the slot, with `PW_MUTANT_PICK=<count>:<i>,..`;
   there `engage()` narrows `MUTANTS` in place to those (after checking
   the count they were dealt from), and takes `PW_MUTANT_PICK` and
   `PW_MUTANT_SLOT` out of the environment; `PW_MUTANT_SLOTS` is not given
   to a slot. Nothing a slot runs deals again.
6. **Each slot runs its own baseline**: a slot's copy is shown green before
   any mutant there counts. A slot whose baseline is red is said as the
   script says it, that slot's words alone, and the run stops.
7. **The output is the script's**, on standard output; each slot's
   standard error is passed on as it comes:
   - the baseline as slot 0 said it;
   - each mutant's lines (those since the slot's last verdict, and its
     verdict) in the script's order, each as soon as every mutant before it
     has been said. A line is a mutant's verdict where it begins with the
     mutant's first element, then `:`, ` [` or two spaces, holds its other
     elements, and says `KILLED`, `SURVIVED`, `ANCHOR NOT FOUND EXACTLY
     ONCE in`, `NOT RUN` or `DID NOT BUILD`; where it holds the first
     alone (`{what}: NOT RUN` names no suite), the next mutant's;
   - after the last, what any slot said there beside its count, once; the
     count summed, `<k> of <n> mutants killed`; and the memory bound's line
     as a `mutation_bound.Watch` says it over the lines said, its stops and
     verdicts in the order a run in order would have said them.
8. **In one slot**, in order, still out of the tree: a script given
   arguments (it chooses its own mutants, and slot 0's baseline would not
   be the run's whole), and those that cannot share the machine with
   themselves (`SEQUENTIAL`): `messages_mutations.py` and
   `notifications_mutations.py` (hosts on fixed ports, not `PORT`) and
   `bench_contract_mutations.py` (the baselines' builds may write caches
   into the shared `node_modules`).
9. **Stopped from outside** (SIGTERM, SIGINT), the run stops each slot's
   script by SIGTERM to its process group (each restores its slot's source),
   waits up to 30 s (then SIGKILL), and kills what each had started in
   sessions of its own (`bounded`'s commands) that is still the same
   process (its start time, read again).
10. **`python3 scripts/mutation_slots.py --remove`** removes the slots;
    `cargo clean` does too, with the tree's target.

## Acceptance

- `scripts/tests/test_mutation_slots.py`, 18 tests (8 s here): a toy tree
  holding a toy script of the scripts' shape (a suite-sorted baseline, one
  verdict a mutant, the count), the three modules beside it, its own git;
  its suite asks git what is tracked and records where it ran:
  - in order and in 3 slots, the same standard output byte for byte, and
    the same code; its control, a comment changed, `SURVIVED` in both;
  - the tree's file never written in slots (bytes and time); every suite
    run in a slot, with that slot's `PORT`, `TMPDIR` and `CARGO_TARGET_DIR`,
    and no `PW_MUTANT_*`; an uncommitted file carried; `node_modules`
    linked;
  - a red baseline in one slot said as the script says it, no verdict of
    another's; a run given arguments in one slot, said as in order;
  - a run stopped by SIGTERM while a slot holds its mutant: the tree as it
    was, in under 20 s, every process its suites started gone, the next run
    whole; a second run waiting for the slots the first holds, then saying
    what the script says;
  - each verdict shape above; a longer description not taken for a shorter
    one's; a slot's lines each the right mutant's, one it chose not to run
    skipped; dealing, narrowing, the copy by content, the bound's line, and
    the Playwright offsets within the 200 ports of a slot.
- `scripts/mutation_slots_mutations.py`, 31 mutants: FILL (own-4 run).
- The real scripts, in order and in 4 slots: FILL (target/proof).

## Not claimed

- **That every script's output is the same byte for byte in slots** is
  held for the toy, and FILL for the scripts run. A line a script prints
  between its baseline and its first mutant's verdict is said as that
  mutant's; a slot's memory-bound stop during its own baseline (other than
  slot 0's) is not said. The 261 scripts print neither.
- **That a slot's tests pass as the tree's do**: each slot's baseline is
  its own evidence; a slot that differs is red and stops the run.
- **CI**: no workflow sets `PW_MUTANT_SLOTS`; whether 2 slots help a
  4-vCPU runner is measured below, and the setting is the integrator's.

## Alternatives

- **Each script calls a shared runner.** 261 edits for one behaviour; the
  import that already reaches every script (ADR-0292's precedent) is one.
- **Detached `git worktree add` per slot.** Registers each slot in the
  tree's repository, where `git worktree list` shows it to every session;
  a slot's own git with the tree's objects as alternates is invisible.
- **A slot's target seeded from the tree's** (`cp -a`): cargo's
  freshness is by time, and a binary built in the tree reads the tree's
  fixtures (`CARGO_MANIFEST_DIR`); hard links share fingerprint files that
  cargo rewrites in place; ext4 has no reflinks. Kept slots are warm from
  their second run on.
- **Dealing mutants dynamically**, a slot taking the next as it finishes.
  The script's loop is its own: a slot learns its mutants once, at import.
- **One `bound` across slots.** A mutant's verdict would depend on what
  the other slots held.

## What was done, and what next

FILL: commits, measurements (in order against 4 slots, cold and warm; 2
slots on 4 CPUs), and the integrator's choices: whether CI sets
`PW_MUTANT_SLOTS=2`, the ports a worker's range must hold (`N x 200`).

## Questions for the integrator

FILL.

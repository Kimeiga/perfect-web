# ADR-XXXX: mutants run in parallel slots

Status: DRAFT (work in progress, track `parallel-mutants`). Date: 2026-10-14.
Milestone: E14; the verification run's harness (ADR-0245, ADR-0281, ADR-0292).

## Context

- **Mutation controls are the slowest thing in the project.** 261
  `scripts/*_mutations.py` scripts (2882 mutants, `mutation_anchors.py` of
  2026-10-14) each apply one mutant in place, rebuild, run its tests and
  restore, one mutant at a time. `store_accounts_mutations.py` took 37 min on
  the 32-thread PC (37 of 37 killed); CI's verify shards take 4-5+ hours and
  have reached GitHub's 345-minute job limit.
- **What the scripts share** (mapped 2026-10-14):
  - No script imports a shared runner: `grep -h "^from \|^import"` over them
    finds only `sys`, `subprocess`, `re`, `pathlib` (261 each), `os` (103),
    `signal` (98), `shutil` (5), `random`, `glob`, `base64` (1 each). Each
    script holds its own loop.
  - **One module reaches all of them**: every script imports
    `mutation_baseline` first in its `main()` (ADR-0292, held by its tests).
    On import it wraps `subprocess.Popen.communicate` (keeps what each
    command said, for a red baseline's `explain()`), and starts
    `mutation_bound` (a thread bounding every descendant process at 4 GiB, or
    a quarter of memory; `sys.stdout` wrapped by `Lines`, which reads each
    verdict line, `^(?P<what>\S.*?): (?:KILLED|SURVIVED)\b`, and each line
    beginning `baseline`; the script's last line, at exit, is the bound's).
  - Each script's shape: `ROOT = pathlib.Path(__file__).resolve().parent.parent`;
    file paths from `ROOT`; a module-level `MUTANTS` of tuples
    `(what, [suite,] path, anchor, replacement)` (`mutation_anchors.py` loads
    each script with `importlib` to read them, so a script's top level must
    stay import-safe); `main()`: import `mutation_baseline`, SIGTERM made
    `sys.exit(1)` so the `finally` restores, a baseline per suite
    (`baseline (<suite>): N passed, 0 failed`), then for each mutant: anchor
    counted, applied, its suite run with `cwd=ROOT`, restored in `finally`,
    `<what> [<suite>]: KILLED (k of n tests fail)` / `KILLED (does not build)`
    / `SURVIVED`; then the browser's pages rebuilt from the restored source,
    and `<k> of <n> mutants killed`.

## Decision (direction; to be proved)

1. `PW_MUTANT_SLOTS` (default 1: today's behaviour, unchanged) names how many
   slots a script's mutants are dealt to.
2. Above 1, importing `mutation_baseline` in the main tree hands the run to a
   coordinator (`scripts/mutation_slots.py`): the script's own loop never runs
   in the main tree, so the main tree is never mutated, and a stopped run
   cannot leave a mutant there.
3. Each slot is a copy of the tree (HEAD with the uncommitted changes), with
   its own `CARGO_TARGET_DIR` and its own `PORT` (`PORT + slot x 200`); the
   PostgreSQL tests already use per-process schemas, so slots share the
   databases. ADR-0292's memory bound holds in each slot's script.
4. Each slot runs its own copy of the script with the indices of the mutants
   dealt to it; the import there narrows `MUTANTS` to them. The coordinator
   prints each mutant's lines in the script's order and the script's own
   closing lines, so the evidence files and their parsers read as before.
5. A script that cannot run in a slot (fixed ports, absolute paths, global
   state) is listed with why, and stays sequential.

## Left to do

Survey all 261 scripts for slot hazards; build the coordinator and its tests
(including a mutant that must survive); prove parity and time on
`store_accounts_mutations.py` and `refusal_mutations.py` (sequential against
4 slots); measure 2 slots on 4 CPUs (`taskset`).

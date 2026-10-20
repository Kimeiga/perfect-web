# ADR-XXXX: a train is pre-verified locally, before it is pushed

Status: proposed by track preverify for train-11 (or the train after it),
unnumbered; the integrator numbers it. Date: 2026-10-20. Milestone: E14, the
verification run's own harness (ADR-0245, ADR-0281, ADR-0290). Builds on the
browser host map (`ADR-XXXX-browser-host-map.md`: the registry's stride and
the hosts a run needs) and the allocator
(`ADR-XXXX-mutants-run-in-parallel-slots.md`: `port_layout`, `port_for_slot`).

## Corrections first

- **The lanes' `CARGO_TARGET_DIR` is inside each lane's worktree**, not beside
  it: `spikes/own-renderer/run.sh` and `feed.sh` copy the wasm builds from
  `$REPO_ROOT/target/wasm32-unknown-unknown/..`, so a target outside the tree
  (the brief's "its own `CARGO_TARGET_DIR`") would leave them looking at
  nothing. Each lane still has its own, and it goes with its worktree.
- **A lane's `PORT` is not the only base a recipe starts hosts from.** The
  messages and notifications mutation scripts run Playwright on
  `PW_MESSAGES_PORT` (7141 where unset) and `PW_NOTIFICATIONS_PORT` (6100),
  and the benchmark harness on `BENCH_PORT` (3200). A lane sets all of them,
  or two workers' runs collide (the integrator found it in train-11's wholes,
  `train-11.md`, 03:15Z).
- **The repository moved** to `studiokimeiga/perfect-web` (the integrator,
  2026-10-20 04:12Z), and WORKER-RULES' `gh --repo Kimeiga/perfect-web` is out
  of date. This tool reads nothing from GitHub (decision 7).

## Context

- **Train-10 lost two days to failures that CI found one at a time.** A
  train's `verify` plans every recipe its range reaches into nine shards on
  4-vCPU runners; one shard takes 4 to 5 hours, `e14-refusal` ran past the
  345-minute job limit, and each push found the failures its predecessor's
  fix had not reached: push, wait six hours, read one red recipe, fix, push.
- **The PC has 32 threads, and the integrator was already doing this by
  hand.** Train-11's wholes ran in throwaway worktrees ("lanes A to E") with
  a `PORT` window each (`/home/haki/t11/lane-setup.sh`, `wholes.sh`), and
  found on the way that `messages_mutations.py` and `notifications_mutations.py`
  run Playwright on `PW_MESSAGES_PORT` (7141) and `PW_NOTIFICATIONS_PORT`
  (6100), not on `PORT`, so that two workers' runs collide unless both are
  set. What was a set of shell scripts is a command here, planned by CI's own
  planner so that "what the lanes ran" and "what CI will run" are one list.

## Decision

1. **`just preverify [BASE]`** (`python3 scripts/preverify.py`; `BASE` is a
   ref, `origin/master` where it is not given) plans what `verify.yml` plans
   for a track's push at HEAD, with the tree's own planner: `scripts/ci_plan.py
   --shards 9 --changed $(git merge-base BASE HEAD) HEAD --current`, and
   `--stale` on a branch `track/train-*` (`--stale` and `--no-stale` say it
   either way; `--no-current` keeps the recipes whose committed evidence is
   current). `--recipes "a b"` runs those instead, as the workflow's dispatch
   does (the planner refuses a recipe that is none). `--plan` says what would
   run, in queue order, with each recipe's planned seconds, CI's, and what it
   sets up, and stops. The planner's own notes (`ci_plan: X is current,
   recorded at ..`) are passed on. Tests read `verify.yml` and hold the
   tool's plan command (its shards, `--current`, the train rule, the merge
   base), its Build step and CI's cargo settings to it, both ways.
2. **It is a pre-check, and says so** (its help, its docstring, this ADR):
   it never writes evidence into the tree it is run in and never pushes.
   Each lane's evidence files and logs go under `/tmp/preverify-<head>/`
   (`plan.json`, `summary.txt`, `summary.json`, and `lane-<k>/{setup.log,
   out/<recipe>/{results.json,logs/,evidence/,runner.out}}`); an earlier run
   at the same head is kept as `.prev`. **CI's verify stays the evidence of
   record (ADR-0281)**: nothing here is fetched, committed or cited as a
   gate.
3. **Lanes.** The tree must be clean, and the tool says so (`git status
   --porcelain`, untracked files too): a lane is a detached worktree of
   HEAD, a commit, and an edit not committed would not be in it. `--lanes N`
   (default 4, never more than there are recipes). Each lane has:
   - its own `CARGO_TARGET_DIR` (its worktree's `target/`: `run.sh` and
     `feed.sh` copy from `$REPO_ROOT/target/..`), `TMPDIR`, and
     `CARGO_BUILD_JOBS` of the machine's threads over the lanes (32 / 4 = 8);
   - its own `PORT` window: the host registry's stride apart from the first
     port (`mutation_slots.port_for_slot`, the allocator's own convention),
     so that no two lanes' ranges overlap, inside `--ports` (default
     12741-14740, this worker's assignment: room for seven lanes of 256).
     `PW_MESSAGES_PORT`, `PW_NOTIFICATIONS_PORT` and the benchmark harness's
     `BENCH_PORT` are the lane's `PORT` too: each is a base the host map
     offsets from, and a lane runs one command at a time. Before anything
     runs, each reserved port of each lane is bound once to see that nothing
     listens there, and a port in use is refused, naming it (a port in
     TIME_WAIT is not in use). **Two runs on one machine do not share a
     range:** a run holds an `flock` on `/tmp/preverify-ports-<first>-<last>.lock`
     for as long as it lives, and a second run on the same range is refused,
     naming the pid;
   - a runner's environment: CI's cargo settings (`CARGO_INCREMENTAL=0`,
     no debug info, no colour), without the caller's `KIOKUN_DATA`,
     `KIOKUN_APP` (a runner's browser hosts read the repository's sample),
     `PW_STORE_DATABASE_URL`, mutant slots or any other run's ports, and the
     caller's `PW_FEED_DATABASE_URL` **only for a recipe `NEEDS_DATABASE`
     names**, as a shard with a database sets it. A recipe that needs one,
     with none to give it, is **not run and fails the run** (train-10's
     lesson: a recipe that skips without a database records nothing);
   - a marker (`PW_PREVERIFY_LANE`) in the environment of everything it
     starts.
4. **A shared queue, longest first.** The recipes are ordered by the seconds
   the planner deals by (`ci_plan.estimated`: `ci_seconds.json`, or its
   estimate for a recipe not yet measured), ties by name, and each lane takes
   the next when it is free. A lane sets up as a shard does: the toolchain
   copied (hard links, or a copy) from the tree the tool is run in, the Node
   workspace installed offline (`pnpm install --frozen-lockfile --offline`:
   nothing is downloaded), and `verify.yml`'s Build step (`cargo build
   --workspace --locked`, `run.sh`, `keyed-store.sh`, `feed.sh`) **once, in
   the first lane that is given a recipe that needs the build**. A lane whose
   setup fails stops, says why, and leaves its recipes to the others (the
   recipe that met the failed build goes back to the queue); where no lane
   can run what is left, it is "not run", and fails the run. Each recipe is
   run by `scripts/ci_recipes.py`, CI's own runner, from the lane's tree.
5. **The report is CI's.** A recipe fails on a non-zero exit or on a line
   its evidence says survived (`ci_summary.SURVIVED`), and nothing else; a
   test holds the verdicts to `ci_summary.py`'s on the same artifacts. The
   summary has CI's shape (the count, each failure and why, what a memory
   bound stopped, the table sorted by recipe) with each recipe's lane and
   CI's recorded seconds, then the wall-clock, the load average at the start,
   the end and sampled through the run, each lane's setup time, and for each
   failure its **failing test lines** (cargo's and libtest's, Playwright's,
   unittest's, a mutation script's survivors and baselines, the compiler's)
   and **the tail of its log and evidence** as CI prints them, with the path
   of the log. A failure is said when it happens, not only at the end; every
   five minutes a status line says what each lane runs. Exit status 1 where
   any recipe failed or did not run, or a lane stopped, or the run was
   stopped; 2 where it could not start (a tree not clean, a port in use, too
   little disk for the lanes, a plan the planner refused, a directory that is
   no earlier run's).
6. **It leaves nothing running.** A recipe is stopped at `--timeout` (a CI
   job's limit, 20,700 s), and SIGINT or SIGTERM stops every lane's process
   group (SIGTERM, SIGKILL after 30 s), starts nothing more, and says the
   recipes it did not reach are "not run: interrupted". What a recipe leaves
   running in a session of its own (a development server) is found by the
   lane's marker in `/proc/*/environ`, stopped after the recipe, and said in
   the report (CI's runner would have kept it). The lanes' worktrees, targets
   and temporary directories are removed at the end, whatever happened
   (`--keep-lanes` keeps them to look into a failure); logs and evidence
   stay. A run whose pid is alive is not taken from; one that died has its
   lanes' registrations pruned. A directory the tool did not make (no
   `plan.json`) is never moved or removed.
7. **It reads nothing from GitHub and names no repository.** The repository
   moved to `studiokimeiga/perfect-web` while this was written (the
   integrator, 2026-10-20 04:12Z); the plan comes from the tree's refs
   (`origin/master` is a local ref), and a test holds the tool and its
   recipe to naming no slug, no `github.com`, no `gh` and no `--repo`.
   WORKER-RULES' `--repo Kimeiga/perfect-web` is out of date.

## Measured

On the PC (WSL2, 32 threads, shared with ten-plus other workers), Python
3.10.12, just 1.58.0, git 2.34.1:

- **Smoke, four cheap recipes, two lanes** (`e10-ownership`, `e14-logical-mixed`,
  `e10-same-name`, `e14-unsafe-table`): 158 s wall-clock at load 37-50, all ok,
  each lane's setup 4 s (the node install; the toolchain not needed), the
  lanes removed, the tree it was run in untouched. CI records 2 to 10 s for
  each: in a cold lane each recipe compiles its own dependencies (23 to 93 s).
- **A cold lane's build**: `cargo build --workspace --locked` took 4m01 to
  4m14 in each of three lanes at once (8 jobs each, load 55); a lane's
  `target/` is 1.5 GB after it (no debug info, as CI), and the lanes are
  small enough that `--min-free-gib` (default 8 per lane) is a floor, not a
  budget.
- **The real subset** (train-11's last merges): <FILL: the proof run's
  wall-clock against the same recipes' recorded seconds on CI, and the load>.
- **The tool's tests**: 152, ~45 s on a quiet machine. <FILL: mutants>.

## Evidence

- `just e14-preverify` (`docs/evidence/E14/preverify.txt`): the 152 tests and
  the <FILL> mutation controls, each killed by a failed assertion.
- The real subset's summary and plan: <FILL: `/tmp/preverify-proof2/`, quoted
  above>.
- `just ci` at this branch's head, which is pushed only on its `ci: OK`.

## Not claimed; limits

- **It is not evidence, and not CI.** A green pre-check says the recipes
  passed on this machine at this load; CI's verify, on its runners, decides
  (ADR-0281). A red one is a failure CI would very likely find, found sooner;
  a flake on a loaded PC is a finding, recorded, and run again before it is
  chased (WORKER-RULES).
- **The machine is not a runner.** Lanes share 32 threads with the other
  workers (Playwright starts half the threads in workers, in each lane), so
  browser tests that CI's quiet runner passes can time out here; the
  timings are the PC's, at the load the report says. A runner's environment
  has `CI`, `GITHUB_*` and `RUNNER_*` set; the tool sets none of them
  (Playwright's trace-on-failure is the one thing in the repository that
  reads `CI`).
- **Lanes share the PC's PostgreSQL.** CI gives each database recipe one of
  its own (ADR-0278); tests make their schemas by process
  (`pw_test_{pid}_..`) and the lanes share the server (max_connections 100).
  A lane per database would need `createdb` rights.
- **Browsers are installed here, not by the run, and the toolchain is the
  tree's** (`just bootstrap` downloads; this does not). A tree with no
  `.toolchain` is told so.
- **The workflow's other jobs are not run**: `ci.yml`'s `just ci`, the
  browser jobs (`playwright test --project=<engine>` over the whole suite),
  the summary job. `verify`'s `browser` job is not a recipe and is not
  planned here.
- **A cold lane builds from nothing** (about 4 minutes, and each recipe's own
  dependencies after it). A warm target kept across runs would shorten that,
  but a kept worktree contradicts "a detached worktree of HEAD, removed at the
  end", and a target reused under another path is the stale-artifact hazard
  of the slots' ADR; not built.
- **Linux only** for the leftover-process scan (`/proc`); elsewhere a
  recipe's strays are not found, and everything else works.
- The default port range is this worker's assignment (see the questions).

## Questions for the integrator

1. **The default ports.** 12741-14740 is this worker's range, and `--ports
   FIRST-LAST` changes it. Once the track merges, which range should be the
   default (or read from an environment variable)?
2. **Shared PostgreSQL** (above): kept, as the conservative choice that is
   cheap to reverse. A lane per database, or at most N database recipes at
   once, is possible if connections run out.
3. **A warm target** across runs: not built; say if it is wanted, and whether
   a persistent worktree per lane (updated by `git checkout --detach`, as
   `lane-setup.sh` does) may replace "removed at the end".

## What to do next

- After train-11's verify, compare each recipe's seconds there and here
  (`ci_seconds.json`, the report's last column) and write down the ratio.
- Offer `just preverify` in the integrator's rules as the step before a
  train's push, with `--recipes` for the ones that failed.
- If the PC's other workers run lanes too, move the ports to one registry
  (the allocator's `port_layout` already reads the host registry's stride).

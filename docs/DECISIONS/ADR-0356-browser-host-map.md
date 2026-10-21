# ADR-0356: a browser run starts only the hosts its selected tests need (the host map)

Status: proposed by track browser-host-map for train-11, unnumbered; the
integrator numbers it. Date: 2026-10-19. Milestone: E14, the evidence's own
harness (ADR-0245, ADR-0281, ADR-0349). It carries the track's draft of
2026-10-16 onto train-10 (`track/train-10-land` 587657ff) with the changes
below; that draft and its whole uncommitted tree are kept on
`wip/host-map-all-2026-10-19` (a520282f, its staged overlay a commit of its
own, 036e9726).

## Corrections first

- **The suite started 77 development servers, not "about 40", and one after
  another.** With every optional build present the configuration declared
  77 `webServer`s (53 without them) for every run, whatever it selected: a
  run of one spec in one engine started all 77. Playwright 1.58.0 makes each
  `webServer` a plugin and awaits each plugin's setup in turn
  (`playwright/lib/runner/tasks.js`, `createPluginSetupTasks`), so a run
  waited for 77 servers to be ready, each a debug build of the development
  server compiling its components. On the PC at load 30-65 that wait was
  917.6 s of a 1374.5 s whole run: 52 hosts of the store's build at 14.6 s
  each, 15 of the feed's at 2.3 s, three of kiokun's at 40.1 s.
- **kiokun's hosts start close to their 60 s bound.** A `dist-kiokun` host
  took 36-57 s to be ready (load 30-95), and at load 64-100 two whole runs
  stopped on one past Playwright's 60 s `webServer` timeout, before any test
  ran. The bound and the hosts are the base's; the host map starts these
  three only where a selected test needs them.
- **The 2026-10-16 draft's evidence does not carry over.** Its whole mutation
  run (35 of 35), its EI run and its green `just ci` were recorded on another
  base (3019936e under a 344-path staged overlay), and its mutation harness
  loads `scripts/mutation_slots.py` only at one SHA-256 (c87d148c, an older
  version of the allocator's file, which this base does not have). Every
  result below was run again here, by the commands named.
- **The draft refused a selected test whose optional build was missing.** On
  this base that turns skips into failures: CI's browser job builds neither
  `dist-kiokun` nor `dist-guestbook`, and `run.sh` builds only `dist` before
  its whole suite. This branch keeps the base's policy (section 5); the
  question is the integrator's.
- **A native test's one-second collection timeout raced Playwright's own
  start** at load ~60: its collector had not loaded the hanging spec when
  the timeout fired (0 of 1 entered). The timeout is now 8 s; what the test
  claims is unchanged.

## Context

`spikes/own-renderer/playwright.config.mjs` declared every host: the shared
demo, twelve mutating suites times three engines, the kiokun slice, and
thirteen apps or deployments times three engines. Playwright starts every
declared `webServer` before it runs a test, so a recipe that runs one spec,
and each control of a mutation script, waited for all of them. CI's
browser-heavy recipes ran for hours (`e14-refusal` was split to fit the
345-minute job limit).

Two more defects sat in the same place. The mutation slots' ports
(`PORT + k * 200`) put slot 0's reviews and guestbook hosts (offsets 200..202,
210..212) on slot 1's base and public-fragment hosts. And the kiokun hosts
inherited the caller's `KIOKUN_DATA` (the PC's `env.sh` exports it), so local
browser evidence read the owner's dictionary where CI's read the sample.

## Decision

1. **One registry of the suite's hosts.** `host-registry.json` holds each
   host role's offsets from `PORT` (per engine, or one shared), its build,
   the auxiliary listeners it depends on (each payments and scheduled-orders
   host's provider, which runs in the host's process on a port of its own),
   the mutating suites, the port exports, and each spec's roles. Its stride
   is 256 (the highest offset is 212), and it is checked as it is read:
   offsets unique and inside the stride, every slot's ports within TCP,
   providers declared and in every engine, every spec's roles known.
   `host-metadata.mjs` exports the ports from it (`MUTABLE_PORTS`,
   `FEED_PORTS`, ..), with the old configuration's notes on why each family
   has the hosts it has; the 28 specs that imported ports from the
   configuration import them from here, since the configuration now plans
   as it loads. `scripts/mutation_slots.py` (the allocator, track
   parallel-instrument) reads the same file.
2. **The map is held to the specs.** A spec reaches a host only through its
   `baseURL`: the configuration's (the `base` host) unless the file
   overrides it at its top level, or an override naming a port
   `host-metadata.mjs` exports. `hostsReachedBy` reads that from each
   spec's source; a test in `just ci` (`host-plan.test.mjs`, run by
   `scripts/tests/test_browser_host_map.py`) holds every spec's registered
   roles to it, both ways, and a loopback port written any other way fails
   the reading rather than pass it. The one spec whose tests run on
   different hosts, `page-policy.spec.mjs`, names each test's host in an
   annotation (`pw-host`), and holds each test to run on the host it names
   (`roleOfURL`, in its `beforeEach`). An unregistered spec, an unknown or
   reserved role (TodoMVC's ports) and an annotation naming no host or two
   are refused before any host starts; an annotation naming another host
   than the test's fails that test.
3. **Playwright selects; the plan follows.** `host-plan.mjs` asks the same
   Playwright, with the same configuration, working directory and the
   caller's own arguments, which tests are selected: it runs the CLI again
   with `--list` inserted after `test` (every original token kept in its
   place), in a child whose preload installs a reporter of its own
   (`host-plan-reporter.mjs`) through the CLI's own option hook, and reads
   the receipt that reporter publishes only after one complete, successful
   collection. The receipt must be fresh (its nonce), its issuer's and its
   collector's, of the same configuration, with `forbidOnly`, each test
   once, its roles the registry's. A run's workers reuse it only as
   Playwright's own IPC workers of the issuing process; an independent run
   with the same arguments plans again. It interprets no selection itself:
   file and line filters, projects, `--grep`, shards, `--test-list`,
   `--last-failed` and project dependencies mean what Playwright says.
   `--list` does not apply `.only`, so `forbidOnly` holds in both; UI and
   watch mode, which select again after the hosts start, are refused.
4. **Only the selected tests' hosts.** `serversForSelection` declares one
   host per selected role and engine (a shared role once), with each role's
   providers, and nothing else. Every host keeps its data in memory
   (ADR-0257), never reads a deployment's kiokun data (`KIOKUN_DATA`,
   `KIOKUN_APP` removed from its environment), and gets its role's identity
   provider, blob storage or payment provider; `CARGO_TARGET_DIR` is
   honoured. `PW_PERFORMANCE` admits only the Chromium performance host. The
   run says what it starts: `browser host map: 1 of 77 hosts for 8 selected
   tests`.
5. **The build policy, as before the host map.** `dist`, the shared demo's
   build, is required, and refused where a selected test needs it when it
   is not whole, was built with another runtime, or a source it recorded
   changed or is gone (only `keyed-store.sh`'s copies outside the repository
   may be gone; the base ignored every missing source). An optional build
   (keyed, feed, kiokun, guestbook) that is unbuilt leaves its specs out;
   one that is stale leaves them out and says what to run. Its hosts are
   not started; page-policy's tests of it skip themselves, as they did; a
   selected test of one that does not skip itself is refused.
6. **The planner's reach.** A change to the registry or the planner changes
   every suite run in that directory, as a change to the configuration does:
   `ci_plan.py`'s `suite_configuration()` follows the configuration's
   imports, and what each names beside itself, to the files that plan
   every recipe that runs a suite there.

## Measured

On the PC (WSL2, 32 threads, shared with ten-plus other workers: load
averages 30-100 through these runs), `scripts/browser_host_map_timing.py`
ran each selection through a checkout of 587657ff (before) and this branch
(after), in turn, on the same builds and the same development server binary,
timing each run from its start to its exit and reading Playwright's own
account of its hosts (`docs/evidence/E14/browser-host-map-timing.txt`).

| selection | before | after |
|---|---|---|
| one spec, one engine: `e2e/signals.spec.mjs --project=chromium` (8 tests), a mutation control's run | 931.2 s: 77 hosts, the first at 4.2 s, all ready 917.2 s later | 24.9 s: 1 host, at 4.6 s, ready 11.9 s later |
| the whole suite in one engine, as a CI browser job runs it: `--project=chromium --workers=4` (401 tests) | 1145.3 s: 77 hosts, the first at 3.7 s, all ready 937.6 s later; 399 passed, 2 skipped | 473.2 s: 27 hosts, the first at 8.7 s, all ready 269.9 s later; 399 passed, 2 skipped |
| the whole suite in three engines, one invocation, 16 workers (1203 tests) | 1374.5 s: 77 hosts, 917.6 s starting them | the same 77 hosts: 984.9 s starting them (load 60-95) |

Each host is a debug build of the development server compiling its
components: 14.6 s each for the store's build at load 30-65, 2.3 s for the
feed's, 40.1 s for kiokun's. A run waits for its hosts one after another
before its first test, so what the host map saves is the hosts a selection
does not need: (77 - k) host starts a Playwright run, k its selection's
hosts. Measured: 906 s of 931 s saved for one spec in one engine (97%),
672 s of 1145 s for one engine's whole suite (59%). Its plan costs nothing
measurable for one spec (the first host at 4.6 s against 4.2 s) and about
5 s for a whole engine (8.7 s against 3.7 s: the collection loads every
spec once more). A whole run in three engines needs all 77 and saves
nothing.

On CI the browser jobs build `dist`, `dist-keyed` and `dist-feed`: each
job's engine run starts 25 hosts where it started 71, and a recipe or
mutation control that runs one spec starts its 1 to 3 where it started 71
to 77 (77 where the recipe builds kiokun's and the guestbook's too).
CI's per-host time is not measured here; train-11's verify will show it.

## Evidence

- `just e14-browser-host-map` (`docs/evidence/E14/browser-host-map.txt`):
  the map, registry, builds, declared hosts and receipts, 21 of 21 tests;
  the plan against the real Playwright CLI on private fixtures, 16 of 16;
  three specs of three kinds of host (signals, cart, feed) in three
  engines, 111 passed; and the mutation controls, 47 of 47 killed, each by
  the failed assertion (`ERR_ASSERTION`) of the test it names, none by an
  error or another test.
- The whole suite in three engines through the host map, engine by engine
  as CI's browser jobs run it (`playwright test --project=<engine>
  --workers=4`, PORT 11541, load 68-102): chromium 399 passed and 2
  skipped, firefox 392 and 9, webkit 393 and 8; 1184 passed and 19 skipped
  of 1203, none failed, each run on its 27 hosts. The same 19 are skipped
  through the base configuration. Run whole in one invocation (16 workers)
  at load 60-100 it twice stopped on a kiokun host past its 60 s start
  (corrections), and once ran with about forty WebKit tests past their 30 s
  at load 90-95. Through the base, at load 30-65, 1166 passed, 5 failed and
  13 did not run: firefox `lazy-handler.spec.mjs:191`,
  `messages.spec.mjs:123` and `notifications.spec.mjs:95`, webkit
  `messages.spec.mjs:232` and `feed.spec.mjs:506`, each of which passed in
  the engine runs above (the firefox two failed again in the one-invocation
  run at load 90-95): flakes of a loaded machine, recorded as found.
- `python3 scripts/browser_host_map_timing.py ..`
  (`docs/evidence/E14/browser-host-map-timing.txt`): the runs above.
- The scripts whose code or tests this reaches, run whole:
  `evidence_integrity_mutations.py` (its dist-sources control re-anchored on
  `buildProblem`), `evidence_integrity_2_mutations.py` (its planner control
  re-anchored), `dealt_by_time_mutations.py`, `kiokun_inventory_mutations.py`,
  `mutation_bound_mutations.py`: 42 of 42, 35 of 35, 54 of 54, 27 of 27
  and 16 of 16 killed.
- `just ci` at this branch's head, which is pushed only on its `ci: OK`.
- Also run: `cargo fmt --all -- --check`, `scripts/mutation_anchors.py`
  (3326 mutants in 274 scripts, each matching once), `scripts/case-check.sh`,
  `just evidence-gates`, `scripts/suite_status.py` (97 suites, each run by
  `run_suite.sh`), `scripts/evidence_counts.py` (this recipe's count agrees
  with its script).

## Not claimed; limits

- Playwright's `playwright/lib/program`, `common/configLoader` and
  `common/process.js` are implementation, not API: the planner refuses any
  version but 1.58.0, and an upgrade needs this module reviewed and its
  native tests run.
- Collection and execution are separate processes. A spec that registered
  different tests when listed than when run would be planned wrong; the
  repository's specs register statically.
- `hostsReachedBy` reads the repository's conventions (a `baseURL` override
  naming an exported port). A spec that reaches a host another way fails
  the reading; it is not passed.
- The whole suite needs every host, so it saves nothing: Playwright still
  starts them one after another (see what to do next).
- The messages and notifications mutation scripts keep their own fixed
  ports, and run alone in the allocator, as before.
- A browser host never reads a deployment's kiokun data now. Where a caller
  names `KIOKUN_DATA` (the PC's `env.sh` does), `e14-kiokun-word` and
  `e10-kiokun` still run their server tests on it; their browser suites read
  the repository's sample, as CI's always did.
- UI and watch mode (`--ui`, `PWTEST_WATCH`) are refused, and so is any
  Playwright command but `test` that loads this configuration (the VS Code
  extension's test server among them): each selects tests after the hosts
  would have started. A fallback that starts every host for them is possible
  and not built.

## Questions for the integrator

1. **Optional builds: skip, as before, or refuse?** This branch skips a spec
   whose optional build is unbuilt or stale (the base's policy); the
   2026-10-16 draft refused. Refusing is the stricter evidence (a selected
   test never quietly skipped), but needs CI's browser job to build
   `kiokun-site.sh` and `guestbook.sh`, which it does not today: those two
   specs never run in that job.
2. **The registry is shared with the allocator.** parallel-instrument
   (f6e8a14f) carries a byte-identical copy of `host-registry.json` and reads
   its `schema`, `stride` and every role's `offsets`; merged together, both
   add the same bytes. A later change to the registry keeps those fields.
3. **Stale fixtures at 587657ff** (found here, not this track's): the
   committed `store.hours.StoreStatus`, `StoresOpen` and `CartHours`
   components differ from what `pw emit-component` makes of the sources
   there, and `evidence_is_current.rs` compares only 14 of the 20
   components `e10-component` writes. The draft's fix is on the WIP branch.

## The integrator's answers (2026-10-19), applied in the train-11 merge

1. **Skip, not refuse, and never silently.** A spec whose optional build
   (the feed's, the keyed store's, kiokun's, the guestbook's) is missing or
   stale is still left out, since CI's browser jobs build neither kiokun's
   nor the guestbook's. But each skip is printed, with the spec's name and
   each build it lacks, and what to run: `unservedSpecs` now gives every
   spec it leaves out a warning (`e2e/feed.spec.mjs is not run: dist-feed is
   not built: run feed.sh`, or the stale build's problem), where an unbuilt
   one was quiet; `announce` prints them. Test: `host-plan.test.mjs`'s
   policy test, renamed "specs of an optional build that cannot be served
   are left out, each said with the build it lacks, and never dist's".
   Controls in `browser_host_map_mutations.py`: "a spec left out is left out
   silently" and "a stale optional build says nothing of why" (replacing
   "a stale optional build says nothing", whose anchor the change removed).
2. **Nothing goes unrun everywhere**: a test in `just ci`
   (`scripts/tests/test_skipped_specs.py`, by `scripts/skipped_specs.py`)
   holds that each spec an optional build can leave out (by its registered
   roles, or the roles its tests annotate) runs in at least one recipe that
   makes that build (runs its prerequisite) and runs the spec (a suite that
   names it or names none, or a mutation script that runs Playwright and
   names it). At the merge every such spec has one: the guestbook's in
   `e14-guide`, kiokun-site's in `e14-kiokun-word`, page-policy's kiokun
   tests in `e14-page-policy`, the feed's and the keyed store's in
   `e10-browser` among others. Controls: `scripts/skipped_specs_mutations.py`
   (5), recipe `e14-skipped-specs`.
3. The registry shared with the allocator: the host map merges first; the
   allocator's copy is the same bytes.
4. The stale components: fixed in the train before this merge
   (ADR-0355-the-committed-components-are-the-builds.md).

## What to do next

- Start the selected hosts at once rather than one after another (one
  `webServer` that launches them and waits for all), which would cut a whole
  run's start too.
- After train-11's verify, compare each browser recipe's seconds with
  `scripts/ci_seconds.json`'s, and refresh them.
- kiokun's hosts: find why one takes 40 s to be ready where the store's take
  15 s, before a loaded runner meets its 60 s bound.
- If UI or watch mode is wanted with the host map, start every servable host
  for it (the base's behaviour) rather than refuse.

## Numbered on train-11 (2026-10-20)

- **Numbered ADR-0356** (the integrator's ruling of 2026-10-20: a train is
  numbered before it is pushed, after master's ADR-0353, in train order),
  merged from `78f90684` (`track/browser-host-map-train11`). `ADR-XXXX`
  reads `ADR-0356` in `just/browser-host-map.just` (its recipe's header line
  too), `browser_host_map_mutations.py`, `browser_host_map_timing.py`,
  `ci_plan.py`, the tests `test_browser_host_map.py`,
  `test_builds_before_suites.py` and `test_ci_scripts.py`,
  `host-metadata.mjs`, `host-plan.mjs`, and the header of its two evidence
  files, as in this file's name and title.


## Train-12 reserved-role witness repair (2026-10-21)

The hosted campaign at `f15117de` (verify `39526960864`) killed 53 of
54 controls. The unchanged control "a test may name a reserved host"
survived: TodoMVC now has a real host, and its former reserved-role negative
had become a positive. No current registry role is reserved. This removed
the witness, not the supported `rolesForTest` contract: its caller may
supply a valid registry whose role has `reserved: true`.

The existing declarations test now validates a private registry with the
base role reserved, then requires both a fixed spec mapping and an
annotated spec to refuse that role in all three engines. Explicit false
and absent flags accept the same selections. The real TodoMVC positive
remains. Production behavior, the strict producer, and all 54 original
operators are unchanged.

The targeted native proof is in
[the reserved-host diagnostic record](../diagnostics/train-12/browser-host-map-reserved-proof.txt). The old full
fast suite passed 25 cases with and without the original fault. With the
new witness, the same fault produces exactly one `ERR_ASSERTION` failure
in the intended declarations test, with the other 24 cases passing; after
restoration all 25 pass. The native exits, complete unchanged named
inventory, and zero skipped/cancelled/todo cases were checked. The Python
host-map consumer, Node syntax check, and mutation-anchor check also pass.
This is targeted proof; the complete 54-control campaign remains due on
the combined hosted source. The failed historical report is retained.

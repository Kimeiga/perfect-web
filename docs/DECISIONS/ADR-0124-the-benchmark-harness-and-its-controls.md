# ADR-0124: the benchmark harness, and the controls a score needs

Status: accepted under the owner's instruction of 2026-10-02 ("start the
offline benchmark harness"). Date: 2026-10-02. Milestone: E14 (E14-B).

## Decision

`benchmarks/harness/bench.mjs` runs one task on one stack with one agent and
writes one result JSON. Offline, the agents are `noop` and
`replay:<reference|unsafe>`; a model is an `--agent` command, and is E14-E's.

**A task** (`benchmarks/tasks/T08-duplicate-submissions/`) is a prompt, an
optional per-stack setup patch, hidden tests written once for every stack,
and a reference and an unsafe patch per stack. The pilot is T08: one press
whose request is delivered twice adds once.

**A run:**

1. **Sandbox.** The stack's baseline is copied to a temporary directory: the
   Next.js or SvelteKit app, or the Pleris store's program (`domain.pw`,
   `lib/`, `store/`; the platform packages and the toolchain are the
   repository's, as Next is not the agent's to edit). The setup patch is
   applied and the result committed to a git repository of its own. A
   framework sandbox installs its dependencies from a lockfile the harness
   pins (`harness/locks/`), offline and frozen.
2. **Isolation, checked.** The run is refused if any file in the sandbox is,
   byte for byte, a hidden test or a patch (`test/isolation.test.mjs`, with
   its control).
3. **Agent**, in the sandbox, with `BENCH_PROMPT`.
4. **Grading**, each stage recorded: the stack's checker (`tsc`,
   `svelte-check`, `pw check`), its build, its own server from that build on
   a free port, then the shared contract (E14-A) and the task's hidden tests
   against it. Score 1 only when every stage passes. The result records the
   stage that failed, each failing test's title and message, the lines
   changed, and timings.

**The controls**, per task and per stack (`node bench.mjs controls`). A
score is admissible only when all four hold, each requiring tests that ran:

| control  | holds when |
|---|---|
| negative | the no-op agent passes the contract and fails a hidden test |
| positive | the reference patch scores 1, with contract and hidden tests run |
| unsafe   | the unsafe patch scores 0, failing a checker, a build, or a test that ran |
| harness  | no-op scores 0 and the reference replay scores 1, through the agent path |

## Found while building

- **The positive control found a broken harness.** The first Next.js runs
  failed at the build, every one, reference included: Turbopack refuses a
  `node_modules` symlinked from outside the project root. The unsafe control
  read "caught at build" for the same reason. Sandboxes now install their own
  dependencies.
- **Two controls read a stage of zero tests as a verdict.** Copied specs
  could not resolve `@playwright/test`, no test ran, and the negative control
  read "the hidden test failed". Each control now requires tests that ran,
  and the specs are graded beside the harness.
- **A Pleris build from the sandbox used the system's Rust 1.89**, because
  `rust-toolchain.toml` is the repository's. The build runs from the
  repository.
- **The standalone SvelteKit lockfile took `cookie` 0.6.0**, the version
  GHSA-pxg6-pf52-xh8x names, because the workspace's override does not reach
  it. The baseline carries the override itself.

## T08's controls

| stack | negative | positive | unsafe caught at | unsafe patch |
|---|---|---|---|---|
| Pleris | hidden 0/2 | 11/11 | **check**: PW0312, "declares `retry` but is not idempotent", repair "add `idempotent_by InteractionId`" | adds a retry policy |
| Next.js | hidden 0/2 | 11/11 | hidden: both tests fail, in both recorded runs | disables the button while pending |
| SvelteKit | hidden 0/2 | 11/11 | hidden: both tests fail, in both recorded runs | disables each button while pending |

For the two framework stacks the contract's "two quick presses" test also
fails in some runs and not others: a disabled button swallows the second press
only if it lands while the first is pending. The hidden tests are the stable
catch; the evidence file records each run's counts.

The Pleris row is the claim E14 exists to test, observed once: the plausible
wrong fix does not compile, and the diagnostic names the right one. One task
is not evidence of a difference; E14-C's twelve are the first that can be.

## Not claimed, and owed before E14-E

- **OS-level isolation.** The sandbox holds no hidden file, and a process in
  it can still read the repository by absolute path. An agent run needs a
  sandbox that cannot (E14-E).
- **Equal starting text.** The Pleris store's source carries long comments
  explaining its design; the Next.js and SvelteKit baselines carry few. T08's
  setup removes the one comment that named the answer. Whether every task's
  baselines are stripped of design comments needs a ruling before E14-E.
- **Query behaviour on Pleris** (ADR-0123): T02, T07 and T09 cannot be graded
  on Pleris yet.

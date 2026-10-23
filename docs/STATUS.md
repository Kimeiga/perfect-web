# STATUS

<!-- Charter §3.4 requires exactly these sections. Keep them. -->

Reviewed 2026-10-23 against master `360a4db520836acdb207839861fa99ce1638e525`,
train-13's landing, and its completed hosted runs. The
[snapshot at that landing](STATUS-HISTORY-2026-10-23.md), the
[one at train-12's](STATUS-HISTORY-2026-10-21.md) and
[the prior ledger](STATUS-HISTORY-2026-10.md) remain verbatim, including
failed runs, corrections and their original scope.

## current milestone

**E14, the AI benchmark**, ahead of E11-E13 under the owner's 2026-10-02
ruling ([ADR-0119](DECISIONS/ADR-0119-e10-closes-and-the-ai-benchmark-comes-next.md),
[E14 plan](milestones/E14.md)). The offline harness and reference tasks exist.
Agent runs still require the owner's choices of models, budget and how
Pleris is taught to an agent. Compiler and browser tests establish no agent
benchmark. Numbering remains engineering E0-E15, public proofs P0-P9 and
risk experiments RQ-* ([charter](../PROJECT_CHARTER.md)).

**Train-13 landed on master on 2026-10-23** at `360a4db5`: a command's
send is answered within its deadline
([ADR-0381](DECISIONS/ADR-0381-a-commands-send-is-answered-within-its-deadline.md)),
a page's on-demand modules load within it and a failed load is not kept
([ADR-0382](DECISIONS/ADR-0382-a-page-loads-its-modules-within-the-deadline.md)),
the benchmark counts the files a run makes, the E14-E
[decision brief](milestones/E14-E-decision-brief.md) is ready for the owner,
and the Next.js baseline is 16.4.1 for three advisories in 16.3.8
([ADR-0120](DECISIONS/ADR-0120-three-stores-one-contract.md), amended). The
[acceptance record](diagnostics/train-13/acceptance-record-2026-10-22.md) states what its evidence proves and what it
does not.

## last passing commit

**Latest verified master:** `360a4db520836acdb207839861fa99ce1638e525`
(train-13), fast-forwarded from `track/train-13` without rewriting history.

- Exact-commit CI passed where the commit was published: [39899237285](https://github.com/studiokimeiga/perfect-web/actions/runs/39899237285)
  on `wip/train-13`, [39904687494](https://github.com/studiokimeiga/perfect-web/actions/runs/39904687494) on `track/train-13` and
  [39912441526](https://github.com/studiokimeiga/perfect-web/actions/runs/39912441526) on `master`, each with x64, ARM and license/advisory
  checks.
- Automatic track verification [39904687487](https://github.com/studiokimeiga/perfect-web/actions/runs/39904687487) planned `e10-browser`,
  which every push plans (its record's path is a recipe parameter the
  planner cannot read), and passed it and the full Chromium, Firefox and
  WebKit jobs.
- On master, verify [39912441532](https://github.com/studiokimeiga/perfect-web/actions/runs/39912441532) (push) planned no recipe and no
  browser job, the track's run having passed at that exact commit.

Before its record commit, the source `5a4378fe` passed
[CI 39892760379](https://github.com/studiokimeiga/perfect-web/actions/runs/39892760379) and
[verify 39892771765](https://github.com/studiokimeiga/perfect-web/actions/runs/39892771765); the
[acceptance record](diagnostics/train-13/acceptance-record-2026-10-22.md) holds each track's runs, the failed ones
among them.

**Previous landing:** `e7664d2eb01aa8c6e91df594bd7270a84a8bf8de` (train-12,
2026-10-21), with [CI 39584219053](https://github.com/studiokimeiga/perfect-web/actions/runs/39584219053) and
[verify 39584218974](https://github.com/studiokimeiga/perfect-web/actions/runs/39584218974) at that exact commit on master.
Older snapshots and failed or cancelled runs remain in the
[train-13 landing snapshot](STATUS-HISTORY-2026-10-23.md#last-passing-commit),
the [train-12 one](STATUS-HISTORY-2026-10-21.md#last-passing-commit) and the
[last-passing ledger](STATUS-HISTORY-2026-10.md#last-passing-commit).

## completed gate items

On landed train-13:

- A command's send is answered within ten seconds or sent again as its
  `retry` allows, and a send held for the one before it waits its turn
  ([ADR-0381](DECISIONS/ADR-0381-a-commands-send-is-answered-within-its-deadline.md)):
  ten tests in three engines, 18 of 18 controls killed.
- A page's on-demand modules load within the deadline and a failed load is
  not kept
  ([ADR-0382](DECISIONS/ADR-0382-a-page-loads-its-modules-within-the-deadline.md)):
  eight tests in three engines, 10 of 10 controls killed.
- The benchmark's footprint counts the files a run makes
  ([ADR-0124](DECISIONS/ADR-0124-the-benchmark-harness-and-its-controls.md),
  amended): Next.js's T01, T06, T08 and T11 had been undercounted.
- The Next.js baseline is 16.4.1
  ([ADR-0120](DECISIONS/ADR-0120-three-stores-one-contract.md), amended):
  the contract (81 tests, 5 of 5 controls) and the museum (5 exhibits,
  4 of 4 controls) pass on it.

At train-12 source `ec15e5b1`, the latest complete campaign detects all
43 TodoMVC, 62 device-storage and 50 second-host faults, plus TodoMVC's
8 compatibility, 55 Cypress producer and 17 strict producer controls.
TodoMVC's official Cypress suite passes 29 of 29 on the pinned compatibility
copy, not as unchanged upstream execution. The repeated full-browser recipe
passes three whole runs, each with 1,258 passed and 20 explicitly skipped
tests; the skips are not passes. All four records were imported with the
canonical fetcher after workflow completion, and the evidence-count gate
reports zero disagreements and failures. The landing's track verification
then passed the legacy `e8-wit`, `e10-component` and `e8-contracts`
obligations the canonical plan kept. The earlier failed runs remain failed.

On landed train-11:

- Renders share identical calls within a snapshot
  ([ADR-0354](DECISIONS/ADR-0354-a-render-reads-each-call-once.md)); committed
  components are checked against the build
  ([ADR-0355](DECISIONS/ADR-0355-the-committed-components-are-the-builds.md)).
- Browser tests start selected hosts
  ([ADR-0356](DECISIONS/ADR-0356-browser-host-map.md)); mutation slots isolate
  inputs ([ADR-0357](DECISIONS/ADR-0357-mutants-run-in-parallel-slots.md));
  Kiokun cache publication preserves the reader's version
  ([ADR-0358](DECISIONS/ADR-0358-kiokun-cache.md)).
- CI retains failing traces and red baselines, bounds checker latency and
  avoids repeating a verified nightly head
  ([ADR-0359](DECISIONS/ADR-0359-ci-reliability-traces-latency-incremental-builds-and-the-nightly.md)).
  Coverage-guided fuzzing measures or fails
  ([ADR-0360](DECISIONS/ADR-0360-the-coverage-guided-fuzzer-measures-or-fails.md)).
- Check and build share validation
  ([ADR-0361](DECISIONS/ADR-0361-check-is-build.md)); guide mutation kills
  require valid producer outcomes
  ([ADR-0362](DECISIONS/ADR-0362-guide-producer-validity.md)); Wasmtime 48.0.7
  addresses its recorded advisories
  ([ADR-0363](DECISIONS/ADR-0363-wasmtime-48-0-7.md)).

Earlier task controls and commands remain in the [E14 plan](milestones/E14.md)
and [completed-gates ledger](STATUS-HISTORY-2026-10.md#completed-gate-items).
Changed inputs can stale a historical pass;
[ADR-0281](DECISIONS/ADR-0281-a-merge-is-held-to-ci-s-verification-run.md) governs current
evidence. No failed result is promoted by this summary.

## failing gate items

- **A command given up is told as unsent, and may still run** (ADR-0381's
  "not claimed", landed with train-13): an unknown outcome is told as
  "could not be sent", a command given up may still run after those behind
  it, and the host's own waits have no deadline
  ([KNOWN_LIMITATIONS](KNOWN_LIMITATIONS.md)).
- **The computed values keep no deadline** (ADR-0382's "not claimed",
  landed with train-13): a slow boot is waited for, and a device value's
  migration that cannot load them leaves the value under its older key
  ([KNOWN_LIMITATIONS](KNOWN_LIMITATIONS.md)).
- **A follow once went unanswered in Chromium** (`feed.spec.mjs:691`,
  verify 39701638548 at `11d7917c`): its request released, the test timed
  out waiting for its answer. Not reproduced in fifty local runs, two
  concurrency profiles among them, nor in its diagnostic rerun on GitHub at
  `d57f18a1`. Cause unknown; open in [NEXT](NEXT.md).
- Hosted evidence does not prove the local-only performance, memory, load
  or Kiokun obligations. E14 agent benchmarking still requires the owner's
  model, budget and teaching choices. Neither performance universality nor
  production readiness follows from these checks.
- Device storage becomes durable when the queued handler applies the
  assignment. Navigation before it runs can discard pending input; the
  privacy-test repair establishes storage before reload and does not change
  that runtime boundary. See [known limitations](KNOWN_LIMITATIONS.md).
- **E14 gates 3 and 4 remain open:** agent runs and analysis await model and
  budget decisions. Framework-wide performance superiority and broad
  application correctness are not established.
- The landed tree's [backend refusal inventory](../examples/accepted/build-refusals.json)
  records nine exact refusals: resource mounting (A-007), body-level secret
  binding (A-010), build-time page values (A-013), style directives (A-018),
  and placeholder dependencies (A-014/A-025/A-026/A-027/A-034). Repairing
  A-008's WIT collision exposed six existing gaps; the historical compiler
  reproduces the same nine when only A-008 is omitted. The
  [causal probe](evidence/tooling/corpus-after-wit.txt) preserves both runs.
  All 42 historical accepted programs remain unchanged, and A-008 builds.
- The scheduled-order Chromium timeout, WebKitGTK navigation/cancellation
  investigation, E7 long-frame instrument and E10 carry-over work retain
  their scope in [NEXT](NEXT.md), [KNOWN_LIMITATIONS](KNOWN_LIMITATIONS.md)
  and the [historical failing gates](STATUS-HISTORY-2026-10.md#failing-gate-items).
  Temporal authorization, live-version compatibility, unknown outcomes,
  composed budgets and browser-owned editing remain proposals where
  generated-runtime enforcement is not demonstrated.
- Train-12's dated failures and repair obligations are retained verbatim in
  the [landing snapshot](STATUS-HISTORY-2026-10-21.md#failing-gate-items);
  their replacements are reconciled in the acceptance record. None of the
  original failed runs becomes a pass.

## exact commands to reproduce

Quick inspection and focused regressions on master:

```sh
just doctor
cargo fmt --all -- --check
python3 scripts/mutation_anchors.py
bash scripts/case-check.sh
cargo test --locked -p pw-core --test query_values --test let_discard --test effect_convergence
cargo test --locked -p pw-dev-server --bin pw-dev-server -- --exact tests::a_flight_another_reader_waits_for_is_not_stopped tests::a_longer_read_is_not_applied_over_a_commit_it_did_not_see
python3 scripts/ci_plan.py --changed origin/master HEAD
```

The full gate is `just ci`; the changed recipe plan and browser suites run
on GitHub under [PARALLEL](PARALLEL.md)'s branch and port rules. Run reached
mutation scripts whole, then check restoration with the anchor command.
`just evidence-gates` validates machinery using injected producers; it does
not establish real compiler, runtime or browser behavior. Fetching and
freshness follow [ADR-0281](DECISIONS/ADR-0281-a-merge-is-held-to-ci-s-verification-run.md).
`just doctor` is read-only; `just bootstrap` installs pinned dependencies
under existing authorization, not a grant from this document.

## known environmental issues

Development uses Windows/WSL2 and GitHub hosted Linux x64/ARM. Quick local
checks stay under roughly ten minutes; heavy campaigns, full browser suites
and busy-host CI run on GitHub under the owner's 2026-10-20 grant. Workers
use assigned ports and their own targets. Pass
`--repo studiokimeiga/perfect-web` to `gh` for the connected repository.

The PC's WSL `gh` credential failed during this integration and again on
2026-10-21; authenticated Windows `gh.exe` works. Shared load affects timing;
build caches can exhaust disk space. On 2026-10-21 the PC still ran
development servers and `python3 -m http.server` processes that earlier
sessions left a day before, and on 2026-10-22 four such servers and a
dozen `http.server` processes, two days old; a run checks that its ports
are free rather than assuming its range is. On 2026-10-22,
`extractions/setup-crate` found no `just` 1.58.0 release in two jobs of
verify 39808499525 that the run's other jobs found; the two ran again. Follow [PARALLEL](PARALLEL.md)'s bounded diagnostic
rerun and owned cleanup rules; never loosen a bound to obtain a pass. No new
platform-wide outage is asserted here.

October 2 Firefox stalls and September macOS disk/cache incidents are
[dated observations](STATUS-HISTORY-2026-10.md#known-environmental-issues),
not current WSL or hosted-runner claims. Partial output, a missing producer
exit or an existing evidence file is never a successful result.

## last benchmark summary

The E10 close benchmark retained here is 2026-10-02 at
`bff437c6ae641529461ca56410b1f0337e06557a`, Apple M2 Pro arm64:
[raw evidence](evidence/E10/close-bench.txt), `just e10-close-bench 8`.
Store components were 3,037-4,221 bytes. The host's `add_to_cart` median
was 18.767 microseconds versus 20.620 for the Rust guest and 0.302 natively.
Three of eight Chromium runs reported an interaction long frame. These
are old measured inputs, not train-12 performance or framework comparisons.

[Benchmark requirements](BENCHMARKS.md) retain revision, machine, warm/cold
state, samples, distributions and raw commands. Narrow checker timings do
not replace an application benchmark. ADR-0027's correction still applies
to older frame attribution.

## next three concrete tasks

1. E14-E: the owner's model, budget and teaching choices, from the
   [decision brief](milestones/E14-E-decision-brief.md); run nothing paid on
   an assumed choice.
2. Device-local durability (ADR-0366's boundary).
3. The remaining [NEXT](NEXT.md) follow-ups: the unexplained feed timeout,
   and `e10-browser`, which every push plans because the planner cannot read
   its record's path.

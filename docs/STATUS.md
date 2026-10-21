# STATUS

<!-- Charter §3.4 requires exactly these sections. Keep them. -->

Reviewed 2026-10-21 against assembled train-12 source
`2f6dd58b` (with combined hosted verification still pending).
[The complete prior ledger](STATUS-HISTORY-2026-10.md) remains verbatim,
including failed runs, corrections and their original scope.

## current milestone

**E14, the AI benchmark**, ahead of E11-E13 under the owner's 2026-10-02
ruling ([ADR-0119](DECISIONS/ADR-0119-e10-closes-and-the-ai-benchmark-comes-next.md),
[E14 plan](milestones/E14.md)). The offline harness and reference tasks exist.
Agent runs still require the owner's choices of models, budget and how
Pleris is taught to an agent. Compiler and browser tests establish no agent
benchmark. Numbering remains engineering E0-E15, public proofs P0-P9 and
risk experiments RQ-* ([charter](../PROJECT_CHARTER.md)).

**Unlanded train-12** assembles generic-result/effect repairs, device-local
data and TodoMVC, cross-host database checks, verification/producer repairs,
security pins, component namespaces, keyed-read retry and effect convergence.
Individual track evidence does not establish the combined tree. Final
exact-commit CI, full affected verification, evidence reconciliation and
landing are pending.

## last passing commit

**Landed and verified master:**
`115895de63d336352cd9051a828257abb92afedb` (train-11, 2026-10-20).
[CI 39489264202](https://github.com/studiokimeiga/perfect-web/actions/runs/39489264202)
and [verify 39489264142](https://github.com/studiokimeiga/perfect-web/actions/runs/39489264142)
passed at that exact commit. Its failure census also passed.

Train-11's initial source verification
[39442025240](https://github.com/studiokimeiga/perfect-web/actions/runs/39442025240)
at `320c52a4` failed six recipes. The follow-up
[39478386992](https://github.com/studiokimeiga/perfect-web/actions/runs/39478386992)
at `2b8a8b81` passed those six; final master's exact-commit CI and verify
then passed as linked above. The initial failed run remains failed. These
records establish no pass for train-12. Older snapshots and failed or
cancelled runs remain in the
[last-passing ledger](STATUS-HISTORY-2026-10.md#last-passing-commit).

## completed gate items

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

- **Train-12 final gates remain pending.** Narrow controls do not replace
  whole reached campaigns, combined browser suites or exact-head CI.
  The earlier combined [CI 39518614014](https://github.com/studiokimeiga/perfect-web/actions/runs/39518614014)
  at `d8369604` failed five tests on both architectures: the exact corpus
  refusal inventory and four lookups using pre-namespace UI identities.
  Focused checks of their repairs pass locally; the failed run remains failed.
  The [generic campaign 39513687603](https://github.com/studiokimeiga/perfect-web/actions/runs/39513687603)
  completed 73 recipes, with 71 passing and only the case-handling and
  template controls failing. Both control repairs are integrated; their
  full combined rerun remains pending.
- **The 239-recipe combined verification is still running.**
  [39526960864](https://github.com/studiokimeiga/perfect-web/actions/runs/39526960864)
  at `f15117de` passed all three full browser jobs, but native recipes
  `e14-todomvc`, `e14-device-store`, `e14-check-is-build`, `e14-bind`,
  `e14-speculated-arms`, `e14-representations`, `e14-computed-conditions`,
  `e14-mutation-bound`, `e14-guide` and `e14-browser-host-map` failed. TodoMVC's unchanged Cypress
  execution passed 24/29 (native status 3); device-store's strict campaign
  credited 53/62 controls and refused nine invalid producers. The source
  census missed TodoMVC; one of 14 binding controls and one of eight
  speculation controls survived. Representations detected3/4 and computed
  conditions9/12; the memory-bound campaign refused one invalid witness,
  and the guide campaign stopped on a stale control-map entry. The browser
  host map detected53/54 controls; its reserved-host rejection witness
  missed the injected fault. Repairs
  and their whole affected follow-ups are pending; [NEXT](NEXT.md) records
  the causes and acceptance obligations. Post-dispatch guide ownership
  scope changes also require current evidence.
- The graph-on-the-wire campaign returned native status zero, but its written
  evidence records a memory guard stopping the renderer mutation above 3.9 GiB.
  That campaign is invalid. The canonical summary/fetch guard reads the evidence
  files and rejects it; an earlier status/log-only audit missed that stop.
  Its witness and complete rerun are now a separate follow-up.
- The assembled repair gate at `1549c4bb` completed 1,015 tests in 240.385s
  with five explicit skips, and formatting, path-case and all 4,374 mutation
  anchors passed. The guard still rejects failed historical records as accepted
  evidence; mixed transcripts are retained byte-identically in
  [diagnostics](diagnostics/README.md). Subsequent effect-queue integration
  still requires combined hosted verification.
- The isolated effect queue at `fb0bea66` passed
  [CI39532444813](https://github.com/studiokimeiga/perfect-web/actions/runs/39532444813)
  on x64/ARM and both verification batches, covering eight campaigns. Its bounded deep
  helper-chain checker improvement is recorded in
  [ADR-0380](DECISIONS/ADR-0380-effect-inference-queue.md); this is not a
  framework-wide performance result.
- Published repair candidate `34f3c2d1` has
  [155-recipe verification](https://github.com/studiokimeiga/perfect-web/actions/runs/39538607183)
  and [exact-source CI](https://github.com/studiokimeiga/perfect-web/actions/runs/39538597877)
  in progress. The graph campaign is a separate required repair. Chromium's
  setup job hit a GitHub release API HTTP502 before running tests; it must be
  rerun when permitted or replaced by current full-browser proof. This is one
  recorded setup failure, not an application-test result.
- Exact-source [CI39527396553](https://github.com/studiokimeiga/perfect-web/actions/runs/39527396553)
  at `152a4a41` passed x64, ARM and license/advisory checks. Subsequent
  repairs still require their own exact-source CI; this pass does not
  establish the current unlanded combined tree.
- **E14 gates 3 and 4 remain open:** agent runs and analysis await model and
  budget decisions. Framework-wide performance superiority and broad
  application correctness are not established.
- The assembled tree's [backend refusal inventory](../examples/accepted/build-refusals.json)
  records nine exact refusals: resource mounting (A-007), body-level secret
  binding (A-010), build-time page values (A-013), style directives (A-018),
  and placeholder dependencies (A-014/A-025/A-026/A-027/A-034). Repairing
  A-008's WIT collision exposed six existing gaps; the historical compiler
  reproduces the same nine when only A-008 is omitted. The
  [causal probe](evidence/tooling/corpus-after-wit.txt) preserves both runs.
  All 42 historical accepted programs remain unchanged. A-008 now builds
  on this candidate; the combined repairs remain unlanded.
- The scheduled-order Chromium timeout, WebKitGTK navigation/cancellation
  investigation, E7 long-frame instrument and E10 carry-over work retain
  their scope in [NEXT](NEXT.md), [KNOWN_LIMITATIONS](KNOWN_LIMITATIONS.md)
  and the [historical failing gates](STATUS-HISTORY-2026-10.md#failing-gate-items).
  Temporal authorization, live-version compatibility, unknown outcomes,
  composed budgets and browser-owned editing remain proposals where
  generated-runtime enforcement is not demonstrated.

## exact commands to reproduce

Quick inspection and focused regressions on this assembled source:

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

The PC's WSL `gh` credential failed during this integration; authenticated
Windows `gh.exe` works. Shared load affects timing; build caches can exhaust
disk space. Follow [PARALLEL](PARALLEL.md)'s bounded diagnostic rerun and
owned cleanup rules; never loosen a bound to obtain a pass. No new
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

1. Finish train-12's exact-commit CI and full affected verification; inspect
   failed or stopped producers, repair defects, reconcile current evidence
   under ADR-0281, and land only the verified combined tree.
2. Resolve remaining [NEXT](NEXT.md) integration follow-ups using recorded
   reproductions for the scheduled-order timeout, WebKit traffic and
   adapter/estimator gaps; retain unresolved limits.
3. Prepare E14-E's runs and analysis for the owner's model, budget and
   teaching-method choices; run no benchmark on an assumed choice.

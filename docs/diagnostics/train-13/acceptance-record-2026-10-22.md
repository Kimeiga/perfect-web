# Train-13 source acceptance, 2026-10-22

Source: `5a4378fe`, the benchmark's Next.js baseline moved to 16.4.1 on the
merge `c57e5570` of `wip/press-modules` at `826d08d6` (which contains
`wip/command-deadline`) and `wip/e14-readiness` at `d4758ae4`, both from
master `e7664d2e`. This record changes only documents and evidence. It does
not assert that its own commit has passed CI or landed: promotion requires
exact-commit CI and the automatic track verification at the train's head.

## What the train carries

- **A command's send is answered within its deadline**
  ([ADR-0381](../../DECISIONS/ADR-0381-a-commands-send-is-answered-within-its-deadline.md)):
  ten seconds for the whole answer, then sent again as `retry` allows; a
  send held for the one before it waits its turn and is no retry of its own.
- **A page loads its modules within the deadline**
  ([ADR-0382](../../DECISIONS/ADR-0382-a-page-loads-its-modules-within-the-deadline.md)):
  a failed load of a handler, the speculation module, the renderer or the
  computed values is forgotten and asked for again; a press whose
  speculation cannot load is sent unspeculated.
- **E14-E, decision-ready**: the benchmark's changed-lines count includes
  the files a run makes (Next.js's T01, T06, T08 and T11 had been
  undercounted, 27->77, 3->48, 32->52 and 10->37), and the
  [decision brief](../../milestones/E14-E-decision-brief.md).
- **The Next.js baseline at 16.4.1**
  ([ADR-0120](../../DECISIONS/ADR-0120-three-stores-one-contract.md),
  amended): 16.3.8 is named in GHSA-673q-mjm6-r5xr and GHSA-2464-fhr4-w9xw
  (critical, unauthenticated remote code execution) and GHSA-p3m5-59mp-jv5r
  (high, denial of service through a Server Action); no 16.3.x is patched.
  The owner approved the download.

## Completed source gates

- `command-deadline` at `bfda9ae5`: CI [39658029334](https://github.com/studiokimeiga/perfect-web/actions/runs/39658029334), verify
  [39658074296](https://github.com/studiokimeiga/perfect-web/actions/runs/39658074296) (141 reached recipes, `e14-pleris-controls`, three
  browser engines); evidence at `973fbbcc`.
- `press-modules` at `11d7917c`: CI [39701625541](https://github.com/studiokimeiga/perfect-web/actions/runs/39701625541); the 140 recipes of
  verify [39701638548](https://github.com/studiokimeiga/perfect-web/actions/runs/39701638548) that ran to their end, `e14-press-modules`
  among them (eight tests in three engines, ten of ten controls killed);
  evidence at `07977aa8`. At `d57f18a1`, after the test repair: CI
  [39808460517](https://github.com/studiokimeiga/perfect-web/actions/runs/39808460517) and verify [39808499525](https://github.com/studiokimeiga/perfect-web/actions/runs/39808499525) (`e10-browser`, three
  runs of 1,312 passed and 20 skipped; `e14-browser-host-map`;
  `e14-command-deadline`, 18 of 18 controls; `e14-second-host`; three
  browser engines); evidence at `826d08d6`.
- `e14-readiness`: verify [39690926936](https://github.com/studiokimeiga/perfect-web/actions/runs/39690926936) at `ef20110d` and
  [39695970480](https://github.com/studiokimeiga/perfect-web/actions/runs/39695970480) at `61c6f9af`; CI [39701707273](https://github.com/studiokimeiga/perfect-web/actions/runs/39701707273) at `d4758ae4`.
- The merge `c57e5570`: verify [39857464106](https://github.com/studiokimeiga/perfect-web/actions/runs/39857464106) ran the three recipes the
  merge left stale, each branch having recorded `e14-contract`,
  `e14-museum` and `e14-pleris-controls` without the other's change, and
  the three browser engines; evidence at `5ef2af08`.
- The source `5a4378fe`: CI [39892760379](https://github.com/studiokimeiga/perfect-web/actions/runs/39892760379) and verify [39892771765](https://github.com/studiokimeiga/perfect-web/actions/runs/39892771765),
  the recipes the bump reaches on 16.4.1 and the three browser engines:
  `e14-contract`, 81 contract tests on three stacks, the Next.js store
  built and typechecked on 16.4.1, 5 of 5 controls killed; `e14-museum`,
  5 exhibits, 5 refused, 0 missed and 0 not as expected, J01's stale page
  shown on 16.4.1 and its repair holding there, and 4 of 4 controls
  killed. Its evidence is fetched with this record.
- `e10-browser` is not re-run at the merge or the source: the planner names
  it at every push, its record being written through a parameter it cannot
  read, but nothing it reads has changed since `d57f18a1`, by the planner's
  own range rule. The track verification at the train's head runs it.

## Failed and cancelled runs, retained

- Verify [39701638548](https://github.com/studiokimeiga/perfect-web/actions/runs/39701638548) at `11d7917c` **failed**. Two of ADR-0381's
  tests counted given-up sends by Playwright's `requestfailed`, which
  Firefox and WebKit do not report for a request the page aborts while a
  route holds it; the tests now time each send in the page (`d57f18a1`),
  the runtime unchanged. And `feed.spec.mjs:691` timed out once in Chromium
  waiting for a released follow's answer: cause unknown, open in NEXT.
- Verify [39808499525](https://github.com/studiokimeiga/perfect-web/actions/runs/39808499525)'s first attempt failed before any test in two
  jobs: `extractions/setup-crate` found no `just` 1.58.0 release. Those two
  jobs ran again, and the run's result is its second attempt's.
- CI [39857310130](https://github.com/studiokimeiga/perfect-web/actions/runs/39857310130) at the merge `c57e5570` **failed** at the Node audit
  (the three Next.js advisories), its x64 and ARM jobs passing.
- Cancelled, not evidence: 39692598667, 39695782250, 39695794627,
  39695958222.

## Proof limits

- ADR-0381's own "not claimed": an outcome given up is told "could not be
  sent" though it may have committed; a command given up may still run
  after those behind it; the host's own waits have no deadline.
- ADR-0382: the computed values keep no deadline, so a slow boot is waited
  for; a device value's migration that cannot load them leaves the value
  under its older key ([KNOWN_LIMITATIONS](../../KNOWN_LIMITATIONS.md)).
- The feed timeout above is unexplained; fifty local passes and one hosted
  pass do not show it cannot recur.
- The Next.js stack's E14 task controls (`e14-harness`) run only on the
  development machine, where its sandbox could not install offline when
  last tried (2026-10-21); they have not run on 16.4.1. Its contract and
  museum have.
- E14's agent runs await the owner's choices of models, budget and teaching
  method; no paid run has been made. Nothing here measures performance.

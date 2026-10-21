# Train-12 source acceptance, 2026-10-21

Source: `ec15e5b1858b608d5a4c8aa39e00a0683c678505`. This record changes
only documents, evidence and measured recipe timings. It does not assert
that its own future commit has already passed CI or landed. Promotion requires
exact-commit CI, automatic track verification and a current master check.

## Completed source gates

- [CI 39558466827](https://github.com/studiokimeiga/perfect-web/actions/runs/39558466827):
  x64, ARM and license/advisory checks passed at this exact source.
- [Verify 39558474966](https://github.com/studiokimeiga/perfect-web/actions/runs/39558474966):
  completed successfully with exactly four expected recipe records, all
  native-valid, and successful Chromium, Firefox and WebKit browser jobs.
- [TodoMVC](../../evidence/E14/todomvc.txt): 43/43 application controls,
  8/8 compatibility controls, 55/55 Cypress producer controls and 17/17
  strict native producer controls. The application inventory remains 50
  browser identities; invalid producers, action errors and setup failures
  are not credited as assertion kills. Cypress uses the pinned compatibility
  copy; this is not a claim about unchanged upstream execution.
- [Device storage](../../evidence/E14/device-store.txt): 62/62 controls.
  [Second host](../../evidence/E14/second-host.txt): 50/50 controls.
- [Repeated browser suite](../../evidence/E10/browser-suite.txt): three whole
  runs, each 1,258 passed and 20 skipped, with no failures. Skips remain skips.
  The three durations were 23.0m, 23.0m and 22.9m. This recipe is not a mutation
  campaign; no mutation-count inference is made from it.

## Evidence reconciliation

The canonical fetcher imported only completed, source-current, native-valid
recipe outputs. Failed and stale outputs were excluded without altering the
failed historical workflows. Independent audits used complete immutable
source snapshots, the exact recipe output path and commit header, native
receipts, and nonempty control inventories where applicable. Count checks
were not inferred from an empty recipe mapping.

| Completed run | Eligible files imported | Integration commit |
| --- | ---: | --- |
| [39526960864](https://github.com/studiokimeiga/perfect-web/actions/runs/39526960864) | 82 | `ee72a177` |
| [39542637844](https://github.com/studiokimeiga/perfect-web/actions/runs/39542637844) | 5 | `40672cc8` |
| [39538607183](https://github.com/studiokimeiga/perfect-web/actions/runs/39538607183) | 125 | `bd267ad6` |
| [39543811122](https://github.com/studiokimeiga/perfect-web/actions/runs/39543811122) | 17 | `eb1b2e35` |
| [39545311229](https://github.com/studiokimeiga/perfect-web/actions/runs/39545311229) | 4 | `693e0d7c` |
| [39549550596](https://github.com/studiokimeiga/perfect-web/actions/runs/39549550596) | 3 | `bc805844` |
| [39558474966](https://github.com/studiokimeiga/perfect-web/actions/runs/39558474966) | 4 | this record |

These are per-import counts, not a unique recipe total. Reconciliation at
`ec15e5b1` found all 33 recipes in the repair follow-up union covered by
eligible completed replacements, and the first 82 imported records still
current. The canonical whole train plan is also required: it retains legacy
`e8-wit`, `e10-component` and `e8-contracts` for the final automatic track
verification. The first two have no evidence text; the third lacks a versioned
header. No rule was relaxed to credit those files.

After the final four-file import, `python3 scripts/evidence_counts.py`
reports zero disagreements, zero stale/planned disagreements and zero
failures. Reproduce source reach and the final promotion obligations with:

```sh
python3 scripts/ci_plan.py --shards 9 --changed 115895de63d336352cd9051a828257abb92afedb HEAD --current --stale
python3 scripts/evidence_counts.py
```

## Proof limits

Sixteen catalog recipes are local-only and excluded from hosted proof. The
reached local performance, load, memory and Kiokun obligations remain
unproved by these runs. Bounded effect-inference timings do not establish
framework-wide performance. E14 agent benchmarking has not run; model,
budget and teaching choices remain with the owner. Existing backend refusals
and runtime limitations remain explicit in KNOWN_LIMITATIONS.

A device assignment is stored when its queued handler applies it. Reloading
before that happens may discard pending input. The privacy-test repair now
observes storage before reloading; it changes no production behavior. See the
[diagnostic](device-store-marker-race-2026-10-21.md). Passing these checks is
not a claim of universal bug prevention or production readiness.

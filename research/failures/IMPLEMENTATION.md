# Implemented changes and verification

Date: 2026-09-10. Base repository: `Kimeiga/perfect-web` at
`0c2579d2bda9edb91a1ccb817b93f2ba0ab24f45`.

## What changed

1. Both layout spike pages now use `public/loaf.js`, a shared serializer that
   reads `forcedStyleAndLayoutDuration` from each frame's `scripts` records.
   The summarizer distinguishes observed, partial, and unobserved attribution.
   Observer absence and installation failure are separate states. It does not
   report missing observations as zero or infer browser non-support from the
   wrong record type. Census U01; source S64.
2. The measurement harness uses that summary, retains per-sample checksums
   instead of running an extra unreported workload, checks equivalence before
   reporting a speedup, and cleans up its server on browser startup failure.
3. `tools/validate.py` validates the census's fields, IDs, status vocabulary,
   enforcement layers, evidence references, and obligation links, and can derive
   JSON. Its mutation tests reject several false-green inputs.
4. Seven bounded reference countermodels make representative failure scenarios
   reproducible. They are not implementations of Pleris runtime protocols.
5. The root README no longer says that no compiler exists. It directs current
   implementation claims to the status/next-step ledgers instead of repeating
   a stale E0 snapshot.

## Evidence actually produced

| Check | Result | Evidence |
|---|---|---|
| Census structure | 224 leaves, 24 families; 72 CF and 72 RT obligations; 91 evidence anchors | `evidence/census-validation.json` |
| Node regression suite | 20 tests passed, including the real harness startup-failure subprocess | `evidence/node-tests.txt` |
| Python validation and countermodel suite | 20 tests passed | `evidence/python-tests.txt` |
| Actual installed Chromium, local scripts delivered in memory | Known-thrashing control produced nonzero per-script attribution while the old frame-level property remained undefined | `evidence/browser-attribution.json` |
| JavaScript harness syntax | `node --check spikes/layout-phase-scheduler/measure.mjs` succeeded | Reproduce command below |

The browser evidence records the module's SHA-256. It uses Chromium
144.0.7559.96 in a Linux container, not the earlier recorded Chrome 150 build.
The no-long-frame phased case is **unobserved**, not proof of zero layout.
Raw timing values are retained for diagnosis, not advertised as a performance win.

## Reproduce

From the repository root:

```sh
npm run test:failure-census
npm run test:layout-attribution
node --check spikes/layout-phase-scheduler/measure.mjs
python3 research/failures/tools/validate.py --export research/failures/census.json
```

Optional real-browser probe, using an already installed Python Playwright and
browser binary (no download is performed):

```sh
python3 spikes/layout-phase-scheduler/test/browser_probe.py \
  --browser-executable /path/to/chromium \
  --output /tmp/pleris-attribution.json
```

The recorded isolated-container run additionally used `--inline --no-sandbox`.
`--inline` links the same local helper in memory and simulates the query string;
it does not test HTTP/module delivery. This was necessary because the environment's
browser policy refused localhost navigation. No browser policy was changed.
Use sandboxed browser execution for ordinary development.

## What this does not prove

The full Rust/compiler suite, original HTTP benchmark harness, E7/E8 browser/host
gates, Safari, Firefox, real mobile devices, screen readers, database faults, and
production deployment were not rerun locally. The report does not invent results
for them. Remote CI, when available, must be checked separately from these results.

The 224 classifications describe the **audited base revision**. This patch does
not mark all of U01, C01, or any other broad failure family solved because one
instrumentation path was repaired. The 72 CF and 72 RT rows are acceptance
obligations, not 144 newly implemented Pleris tests.

## Historical claims requiring correction or remeasurement

Preserve the original raw E0 evidence, but do not reuse its conclusion that an
undefined frame-level metric establishes browser non-support. The affected
interpretation appears in the layout spike README, `docs/SEMANTICS.md`,
`docs/RISK_REGISTER.md` R20, `docs/KNOWN_LIMITATIONS.md`, `docs/STATUS.md`,
`docs/EVIDENCE_LEDGER.md`, and the E0 milestone notes. All are superseded on
this narrow interpretation by ADR-0027. The wall-clock observations are separate.

## Research and implementation boundary

ADR-0027 accepts the census process and targeted tooling repair. Broader
proposals, including temporal authority, integrity labels, optimistic overlap,
compatibility graphs, and new browser/resource contracts, still need focused
ADRs, accepted-program tests, and generated-runtime evidence. The existing E9
repair and E10-I integration remain ahead of new feature breadth.

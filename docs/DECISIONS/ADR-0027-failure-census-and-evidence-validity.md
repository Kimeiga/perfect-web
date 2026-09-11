# ADR-0027: failure census and evidence validity

Status: Accepted for research governance and the targeted instrumentation repair.
Date: 2026-09-10.
Authorization: the project owner requested research, implementation of concrete fixes,
commit, push, and merge. The census's language/runtime proposals are not all accepted
language changes by this ADR.

## Decision

Maintain `research/failures/` as an invariant-level failure census. Its Markdown
inventory is the classification authority; JSON is a derived convenience export.
Every item records evidence, a root invariant, existing coverage, the reliable
enforcement boundary, proposed treatment, and developer-cost review.

The corpus must distinguish source rejection, deployment-plan rejection, runtime
behavior, domain policy, and external assumptions. Specification, representation,
implementation, and independently observed behavior are not equivalent states.

A proposed rule becomes a guarantee only after its prerequisites, accepted neighbor,
intended diagnostic or forbidden observation, negative/mutation control, code revision,
and applicable runtime/host/browser evidence are linked. Reference countermodels do
not prove Pleris's generated implementation. Structural validation does not prove the
truth of a historical claim.

The existing E9 repair and E10-I order remain in force. Do not merge the unfinished
`3b3-signature-resolved-types` branch merely to make the roadmap appear complete.
Prioritized cross-boundary questions in the census are design obligations, not an
instruction to introduce a second type system or a collection of new annotations.

## Concrete measurement correction

The original layout probes read `forcedStyleAndLayoutDuration` from a frame entry.
The Long Animation Frames specification defines it on `PerformanceScriptTiming`,
reached through the frame's `scripts` records. An undefined frame-level property
therefore did not establish that the browser lacked the measurement.

Use one shared recorder and summarizer for both spike pages. Preserve observed
zero, missing attribution, unavailable observation, and failed installation as
separate states. Sum only reported script durations, with coverage counts; never
claim this is total page layout or that zero long frames proves zero forced layout.

The harness also must not perform an unreported extra workload invocation merely
to obtain a checksum, and must close its server when browser startup fails.

Old raw evidence remains unchanged. Interpretations claiming unsupported attribution
need correction and remeasurement, not invented replacement numbers. The targeted
inline Chromium smoke test is not the historical Chrome build, a full HTTP harness
run, a Safari/Firefox test, or an E7 gate rerun.

Source: https://w3c.github.io/long-animation-frames/#sec-PerformanceScriptTiming
Repository finding and evidence: `research/failures/IMPLEMENTATION.md`.

## Consequences

The repository gains a durable research backlog and falsifiable acceptance contracts,
not a declaration that every listed failure has been eliminated. Semantic changes
still need focused ADRs and real implementation witnesses. Current status remains
owned by `docs/STATUS.md`; old E0 snapshots must not override that ledger.

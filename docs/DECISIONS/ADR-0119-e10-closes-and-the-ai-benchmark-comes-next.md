# ADR-0119: E10 closes, and the AI benchmark (E14) comes before E11-E13

Status: accepted on the owner's ruling of 2026-10-02 ("close E10 and move on
to the AI benchmark"). Date: 2026-10-02. Milestones: E10, E14 (charter §3.1,
§14 M10, §14 M14).

## Context

STATUS left E10's close to review. All five gate items had evidence from
2026-09-25. Since then about 80 ADRs changed the compiler, and Wasmtime moved
from 47.x to 48.0.3 (ADR-0116). A close resting on week-old evidence would be
a claim about different code, so every gate item was re-recorded at `bff437c`:

| gate item | evidence | result at `bff437c` |
|---|---|---|
| 1. the store builds from source without Koka or Marko | `build.txt` (`just e10-build`) | builds; components and handlers byte-identical to the recorded artifacts; 3/3 tests |
| 2. semantics match the oracle | `oracle.txt` (`just e10-oracle`) | 9 declarations x 300 cases agree; 3 wrong references caught |
| 3. no leaks or unbounded growth under sustained load | `load.txt` (`just e10-load`) | 16/16 server tests; 19,000 compiled calls, resident memory +0 KiB |
| 4. size and performance against baselines | `close-bench.txt` (`just e10-close-bench 8`) | components 3.0-4.2 KB against 5.3 KB hand-written Rust; `add_to_cart` 18.8 µs per call against 20.6 µs for the Rust guest |
| 5. no ownership syntax for ordinary values | `ownership.txt` (`just e10-ownership`) | 5/5 tests |

Re-recording found two things.

1. **A defect, fixed (`0ad72c7`).** ADR-0115's authorization lookup accepted
   only an `interface`, `function` export path, so a function a component's
   world exports directly was refused whatever its contract declared. Gate
   item 4's hand-written Rust baseline is such a guest, and its benchmark is an
   ignored test, so CI did not see it. A one-segment path now names an export
   declared with an empty interface; an undeclared export is still refused
   (`a_world_level_export_is_found_by_its_declaration`).
2. **An unstable instrument, not fixed.** E7's gate 8 fails in roughly half
   of runs on this machine, at HEAD and at `6545029` alike
   (`gate8-control-2026-10-02.md`). `just e10-bench` stops at that gate, so
   gate item 4 is recorded by `just e10-close-bench`, which keeps every run.

## Decision

1. **E10 is closed.** Its five gate items pass at `bff437c`.
2. **Charter tasks that are not gate items are carried, not dropped.**
   Each is a row in EVIDENCE_LEDGER's deferred obligations, owed at E15
   (hardening) unless a later ruling moves it:
   - task 2's Wasm for compute-heavy browser modules (`E10-T2`);
   - step 10, replacing the host's `store:data/carts` with compiled Pleris
     (`E10-S10`);
   - an automatic memory strategy beyond invocation regions; ADR-0046's
     measurements favour instance reuse (`E10-M`).
3. **E14 (the AI benchmark) is the current milestone, before E11, E12 and
   E13.** None of E14's gate items depends on the network lab, HTTP/3 or
   Servo. E14 tests the charter's central claim (§14 M14 rationale): that the
   language measurably narrows what humans and agents can get wrong. The other
   three refine delivery. This is the charter's own precedent for moving a
   milestone (E2D was pulled forward out of E9).
4. **E14 starts with the harness and the tasks, not with agents.** Charter
   §14 M14 task 9 defers smaller agents until failures are meaningful. The
   first slices build the three reference implementations, the hidden tests,
   and the controls that make a score admissible. Agent runs follow, with the
   models and budget the owner chooses (`docs/milestones/E14.md`).

## Consequences

- E11-E13 stay in the charter's order after E14. Nothing in them is started.
- The benchmark will reach language gaps the store has not: a form needs the
  event as a handler parameter (ADR-0058, ruling needed), and an accessible
  dialog needs view composition (ADR-0072, ruling needed). A task Pleris
  cannot express is reported as a result, not removed from the set.
- The E7 gate 8 instability is an open obligation (`E7-G8`). Until it is
  ruled on, P1/P2 material may not say interactions produce no long frame on
  this machine.

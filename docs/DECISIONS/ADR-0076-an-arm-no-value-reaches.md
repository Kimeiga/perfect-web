# ADR-0076: an arm no value reaches is refused

Status: accepted under the owner's instruction of 2026-09-26 ("keep working
until its perfect"). Date: 2026-09-26. Milestone: E10 (charter §7.1;
ADR-0011, exhaustiveness).

## Context

A match tries its arms in order, so an arm whose values the arms before it
already take never runs. The exhaustiveness analysis has computed such arms
from the start (`exhaust::MatchReport::unreachable`, with its own unit
test), and nothing read them. ADR-0059 recorded that "an arm no case reaches
is refused by the backend and not reported by the checker". A probe on
2026-09-26, at 15d261c, found three shapes, each of which checked:
- **`_ => 0` before `Circle(r) => r`.** The circle's arm is dead, and a
  circle answers `0`. The backend refused it when a query called it: "a
  case matched twice ... the second can never run".
- **`Some(y)` after `Some(x)`.** Refused by the backend the same way.
- **`1 => 20` after `1 => 10`.** It built, with its second arm dead: the
  backend has no rule for a literal matched twice.

## Decision

**An arm no value reaches is refused** (PW0333, "every arm of a match must
be reached by some value"), on its pattern: "no `Shape` reaches this arm:
the arms before it take every value it matches". The analysis's result
records each such arm (`MatchAnalysis::unreachable`), and the diagnostic is
its projection, as PW0305 is of the missing cases. Where the analysis did
not run, nothing is recorded, and nothing is reported.

A template's `{#match}` already refused an arm matched twice (PW5019).

## Acceptance

- **`compiler/pw-core/tests/unreachable_arms.rs`**, 5 tests, each with
  controls.
  - The four diagnostic tests fail at 15d261c, the commit before.
  - The fifth reads the analysis's new field, which does not exist there.
- **No rejected, rule or generality fixture's diagnostics change.** The
  store, kiokun, the standard library and the accepted corpus have no
  unreachable arm, and check clean.
- **Mutation controls:** `scripts/unreachable_arm_mutations.py`,
  `just e10-unreachable-arms`, 3 mutants.

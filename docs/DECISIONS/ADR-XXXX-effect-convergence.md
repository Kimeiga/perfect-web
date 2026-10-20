# ADR-XXXX: inferred effects reach a fixed point before checks and contracts

Status: proposed for integration; ADR-0375 reserved. Date: 2026-10-20.
Milestone: E14.

## Context

Effect inference said it propagated until no row changed, but stopped after
eight sweeps. A caller-first chain of nine or more helpers without declared
rows could therefore hide a database read from a view declared pure. Both
a direct call and a member read at the end escaped PW0401. The same program
in reverse declaration order was refused. A query using the chain also lost
`database.read<Item>` from its capability contract.

This predates the generic-effects repair; that repair's independent review
exposed the effect-convergence limit.

## Decision

Continue propagation until no known effect row grows. Every known row is a
monotone union of effect strings from finite source rows/signatures and other
known rows. Inference does not synthesize new effect strings, so the finite
union reaches a fixed point, including cycles. The existing final stable
sweep also refreshes performed-effect evidence before checking bodies.

No arbitrary depth bound silently certifies a partial result. No new worklist
or cache is introduced; optimizing the iteration is a separate measurement
question that must preserve this fixed point.

## Evidence

Four tests first built and failed equality assertions, then passed with the
change. They cover fifteen-helper direct and member chains in both source
orders, pure field-read neighbors, recursive cycles with and without an
effect, and the query's required capability as well as the view's rejection.

`just e14-effect-convergence` records those four witnesses and two strict
controls: restore the eight-sweep cap; stop after the first sweep. Only
complete Cargo results accounting for all four named witnesses and their
equality assertions earn kills. It also runs the opt-in bounded scale smoke
with `cargo test --locked -p pw-core --test effect_convergence -- --ignored
--nocapture`. Mutation and remote CI results are pending.

On the same local debug build, each latency below is the median of five
checks after one warmup. These are observations under shared machine load,
not a performance guarantee. Reference feed diagnostics remain zero; every
effectful chain now has the expected rejection, whereas caller-first chains
had none before.

| Input | Before, ms | After, ms |
| --- | ---: | ---: |
| Reference feed | 225.3 | 221.6 |
| Caller-first chain, 15 helpers | 51.6 | 55.9 |
| Caller-first chain, 100 helpers | 92.9 | 201.0 |
| Caller-first chain, 250 helpers | 197.7 | 712.2 |
| Callee-first chain, 250 helpers | 192.2 | 200.1 |

The committed opt-in smoke repeats the feed and both orders at depths 15,
100 and 250, prints timing ranges, and verifies every diagnostic count. It
completed locally in 9.27 seconds with one passing smoke test. There is no
machine-speed assertion in the ordinary suite.

## Limits and follow-up

Long caller-first chains require more work because inference now finishes
it. The smoke bounds the measured depth, not the language's allowed depth.
A future dependency worklist may reduce that cost; neither arbitrary deep
source performance nor global compiler soundness is claimed here.

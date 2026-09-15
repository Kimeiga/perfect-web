# Evidence-gate review, 2026-09-15

## Scope and provenance

Repository: `Kimeiga/perfect-web`. Starting master:
`4a9bcddc095b3b4d0eac0e0c2ddb8a0143de03d9`.
Original `justfile` Git blob: `a4d954f8042d0bc33761d081e636254a8e9cd0e7`.
Repaired `justfile` Git blob: `31c9db61ad83f76bb8c9ee01605cfc0611d1ea1e`.
Test module Git blob: `993f757d47b60ca4d89c6c38d9f0d116a3c4d24a`.

The source snapshot came from GitHub Actions run 35034596544 at
`3bd5dbdb02b84d52a49e033a12bb71cff6a62b31`, the starting master plus a
read-only snapshot helper. The archive digest was checked and the original
justfile's Git blob matched before testing. The temporary workflow is removed
by this repair; no review binary or temporary source archive is committed.

Local environment: Linux 6.18.44 x86_64, Python 3.13.5, Node 22.16.0,
just 1.58.0. No local Rust build or real-browser gate was performed.

## Reproduced defects and repair

The original shell was `bash -uc`. A pipeline's successful last command hid a
failed producer. Adding `pipefail` alone was insufficient for grouped commands:
a subsequent successful command could still determine the group's result.

The recipe shell now uses `bash -euo pipefail -c`. `resume-matrix` uses
`sed -n '1,3p'` instead of an early-closing `head -3`, preserving the displayed
summary while draining input to avoid creating an unrelated SIGPIPE failure.
`just ci` now depends on `evidence-gates`.

## Results actually observed

The tests copy the real justfile into temporary trees. They replace Cargo and
renderer producers with deterministic programs that record their invocations,
print plausible successful sub-suite summaries, and optionally exit 17. This
isolates the recipe's exit handling from compiler/browser correctness.

| Local check | Observation |
|---|---|
| Original recipe matrix | 24 of 28 injected failures incorrectly returned success |
| Repaired recipe matrix | 17 success cases and 28 failure cases passed, over 17 recipes |
| Grouped report controls | Failure stops later producers and post-test claims in four report recipes |
| Combined renderer control | Failure prevents the combined pass table |
| Large-output controls | 20,000 summary lines drain successfully; an exit-17 failure remains a failure |
| Deliberately remove pipefail | Reproduces false success through `tail` |
| Deliberately remove errexit | Reproduces continued execution inside a report group |
| Existing census suite | 20 Python tests passed; structural inventory validated |
| Existing instrumentation suite | 20 Node tests passed; measurement harness syntax check passed |

The real-recipe matrix was run in independent local batches because the review
host limits individual command execution time. The committed unittest suite runs
all cases without a selection filter. Its ordinary command is:

```sh
just evidence-gates
# Equivalent when just is on PATH:
python3 -m unittest discover -s scripts/tests -p 'test_evidence_gates.py' -v
```

To observe the original failures, use the same tests with the original justfile
in a separate worktree. The weakened-shell controls also reproduce each masking
mechanism without editing the checkout. Missing `just` is an error, not a skip.

## Limits

These are harness tests, not 45 compiler or browser guarantees. This repair
cannot prevent every failure hidden inside a child script, make redirected
reports atomic, or establish that every printed statement remains current.
Failed commands may leave partial files; file presence is not evidence of success.
No historical raw evidence is rewritten, and the defect does not establish that
historical underlying tests failed.

The full Rust build, current dependency audit and new complete test command must
be evaluated from this patch's CI checks. Earlier successful baseline CI is not
substituted for those checks. E9's argument/return repair and E10-I remain open.

## Relationship to the prior recommendations

The merged census already records assumptions and trust boundaries, authority
across versions, distinct intent/attempt/resource identities, browser-owned
state, semantic constraints on optimization, and evidence levels. See its
README highest-risk findings and REQUIREMENTS parts 12-14. This review does not
promote any of those proposals to an implemented guarantee. It repairs one
concrete evidence path under ADR-0027 and clarifies the current status documents.

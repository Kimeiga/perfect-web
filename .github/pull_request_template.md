## What invariant or behavior changes?

<!-- State the contract, not merely the files changed. -->

## Reproduction and tests

- [ ] A regression would fail without this change, or explain why not
- [ ] A valid adjacent case and an invalid/failure case are covered
- [ ] `just preflight` (or CI preflight if local Rust is unavailable)
- [ ] Affected unit, browser and mutation/recipe checks completed at this SHA
- [ ] Failures, skips and environment blockers are stated explicitly

## Independent review

What could be wrong with authorization, privacy, transaction outcomes,
cancellation, concurrency, input bounds, or the test's own observations?
Document the strongest counterexample tested.

## Scope and limits

What is explicitly not implemented or not established by this PR?
Add links to the exact CI and verify run before marking ready.

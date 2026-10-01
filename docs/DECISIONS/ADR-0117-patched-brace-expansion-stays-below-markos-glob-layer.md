# ADR-0117: patched brace expansion stays below Marko's glob layer

Status: accepted under the owner's 2026-10-01 instruction to close known gaps.
Date: 2026-10-01. Milestone: E15 supply-chain hardening.

## Context

The Node advisory gate found three denial-of-service advisories in
`brace-expansion 5.0.9`, reached through:

```text
@marko/run 0.11.8
  -> glob 13.0.6
    -> minimatch 10.2.6
      -> brace-expansion 5.0.9
```

The vulnerable package is transitive. The parent accepts the 5.x line, so this
does not require replacing Marko, glob, or minimatch.

## Decision

Pin the currently resolved 5.x line to `brace-expansion 5.0.12` with a pnpm
override and regenerate the lockfile with pnpm. Do not allow-list the
advisories.

The override is deliberately scoped to `>=5.0.0 <5.0.12`; it does not force a
5.x ESM package into a future dependency that expects the incompatible 1.x,
2.x, or 3.x API.

## Acceptance

- `pnpm install --frozen-lockfile` succeeds.
- the lock contains `brace-expansion 5.0.12`, not 5.0.9.
- the Node audit reports none of the three advisories that triggered this
  repair.
- no entry is added to `tools/node-audit-allow.txt`.

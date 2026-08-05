# ADR-0002 — Marko 6 is the temporary renderer behind an adapter

**Status:** Accepted
**Date:** 2026-08-05
**Milestone:** 0

## Context

Charter §14 M3 proposes lowering `pw` templates to generated Marko 6 rather than
writing a streaming resumable renderer first. `spikes/marko-stream-resume`
measured what Marko 6.3.32 actually delivers.

## Decision

Adopt **marko 6.3.32 / @marko/run 0.11.8 / @marko/run-adapter-node 2.0.6 /
vite 8.2.0** as the Milestone 3 rendering target, behind a generated adapter.

Generated Marko lives in a build directory and is **never** the authoring source
of truth (charter §14 M3 task 3). No Marko concept may appear in `pw` source or
in a user-facing diagnostic.

**Deletion condition:** Milestone 7's own document-parts renderer passes the
renderer-independent golden suite, with any intentional differences documented.
Marko is retained after that as a benchmark oracle only.

## Consequences

**Measured, not assumed:**

- Static route: **588 bytes of HTML, zero `<script>` tags, zero downloaded JS.**
  The charter §18.4 budget is met exactly.
- Streaming works and is out-of-order: shell at 3.1 ms, 400 ms subtree at
  408 ms, 1200 ms subtree at 1206 ms, chunked, 3 chunks.
- Resumption is real: a 9.6x larger document produced **1.002x** the client JS
  (176 B vs 182 B route-specific, over a shared 3,745 B runtime).
- The emitted HTML *is* a document-parts representation — `<!--M_*2 a-->`
  anchors plus an inline resume manifest — which gives Milestone 7 a concrete
  oracle to diff against.

**Qualification we must not lose:** "zero JS" is only exactly true for the fully
static route. The streamed route downloads nothing but carries **849 bytes of
inline script** to apply out-of-order patches. That is the same mechanism charter
§8.6 proposes building, so it is a cost to account for, not a defect.

**Cost accepted:** Node stays in the Milestone 3 loop, and Marko's syntax has
sharp edges (finding F-8) — but none of them reach `pw` source, because the
adapter generates the Marko.

## Revisit when

- Marko 6 introduces a breaking change in its resumption format or tag API.
- Milestone 7's renderer reaches parity.
- Measured streaming or resumption behavior regresses against this baseline.

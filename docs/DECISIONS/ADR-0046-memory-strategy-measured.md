# ADR-0046: invocation regions stay, as measured on kiokun

Status: accepted under the owner's instruction of 2026-09-25 ("ok do all of
that"). Decisions marked **(ruling needed)** were made without a ruling and are
offered for reversal. Date: 2026-09-25. Milestone: E10 (charter §14 M10
task 4).

## Context

Charter §14 M10 task 4: "Evaluate memory strategies with real benchmarks:
stack allocation, escape analysis, unboxed values, reference counting,
region/arena allocation, Wasm GC, Perceus-inspired reuse analysis." The
rationale is that a strategy is "selected from measurements, not ideology".

A compiled component allocates in one bump region per invocation, reset by
its post-return (ADR-0032, where it is called provisional). No value outlives
an invocation: the Canonical ABI copies arguments in and results out, and
the host gives each call a fresh instance.

## What was measured

The host now reports, for any call, through the path every call takes
(`pw_host::engine::Usage`):
- the instructions it executed, as wasmtime's fuel counts them;
- the largest linear memory the instance had, a multiple of 64 KiB;
- the time to link and instantiate beside the call's.

`just e10-memory` runs every compiled kiokun query on the whole shard, in
release (`docs/evidence/E10/memory.txt`):

| query | calls | instructions p50 / p99 / max | peak memory | instantiate p50 | call p50 |
|---|---|---|---|---|---|
| `shards.Place` | 19,597 | 1,626 / 6,679 / 12,615 | 64 KiB for all | 7.5 µs | 0.9 µs |
| `shards.Places`, 1,000 words | 20 | 2.2 M / 4.4 M / 4.7 M | 128–320 KiB | 9.6 µs | 300 µs |
| `kiokun.page.Lookup` | 17,597 | 498 / 1,548 / 6,348 | 64 KiB for all | 7.3 µs | 7.2 µs |
| `kiokun.page.Search` | 28,951 | 8,159 / 337,524 / 26.2 M | 64 KiB for 98.9%; 3.2 MB at most | 13.9 µs | 213 µs |

Three things follow.
- **Memory management costs no instructions.** Reclaiming a region is
  resetting one global.
- **A region grows with a call's allocation, not its live data.** Search's
  most expensive query is the one letter `T`: 26.2 M instructions, with the
  region grown to 3.2 MB. Every intermediate list of its chain stays until
  the call ends: scored, filtered, sorted, grouped and ranked. Its twenty
  hits are a small fraction of that.
- **The fresh instance costs more than most calls.** A call to `Place` does
  0.9 µs of work inside 7.5 µs of instantiation, and `Lookup` 7.2 µs inside
  7.3. Per-call instantiation, not memory, is where the time goes.

## Decision

**Invocation regions stay the strategy for compiled components**, no longer
provisional. For each strategy the charter names:

| strategy | here |
|---|---|
| region / arena | the strategy: O(1) reclamation, a peak equal to one call's allocation |
| stack allocation, escape analysis | nothing escapes an invocation, so the region already is one frame; they would only reuse memory within a call |
| unboxed values | already the representation: records and lists sit in the Canonical ABI's flat layouts, with no box or header per value |
| reference counting | a header and an increment or decrement on every copy, to bound a single call's peak. 98.9% of Search calls stay in their first page, so the cost buys nothing measured |
| Wasm GC | the Canonical ABI lifts from and lowers into linear memory, and a GC reference does not cross a component boundary. Not applicable on the server; the browser's Wasm step (task 2, later) is where to evaluate it |
| Perceus-style reuse | in-place `map` and `filter` over a list nothing else holds would bound Search's worst growth, and is the first to build when a call's peak nears its limit |

The conditions for revisiting this, each checkable:
1. A call whose peak nears its memory limit (64 MiB in the slice's host, 20×
   Search's worst). Then reuse analysis comes first.
2. State that outlives an invocation, such as a component that keeps values
   between calls. Then that state needs reference counting or a collector;
   invocation values still do not.
3. Compiling to Wasm for the browser. Then evaluate Wasm GC there.

**(ruling needed)**: whether the next performance work is instance reuse (a
pooled instance per component, with its region and memory reset), which the
measurements favour, rather than a memory strategy. ADR-0032 gives each call
a fresh instance so that a revoked grant is revoked for the next call, and a
pool would have to keep that.

## Not claimed

Reference counting, a collector and reuse analysis were not built and
measured against the region. This evaluation measures the region on the
largest real program there is, and argues the rest from those numbers and
the invocation model. The instruction counts are wasmtime's fuel, which
counts Wasm operations, not machine instructions.

# Benchmarks

**There are no benchmarks yet.** Benchmarking begins in Milestone 3
(charter §14 M3 task 8); baselines against Next/React, SvelteKit and Marko are
charter §18.1.

This file exists so that when numbers do appear, they appear in the required
form rather than as anecdotes.

## Required record for every result (charter §18.5)

```text
commit
machine and OS
browser/version
network profile
warm/cold state
sample count
median and distribution
commands
raw result file
```

> "Do not publish one-run anecdotes as conclusions." — charter §18.5

## Host caveat that applies to every future number

This machine is an **Apple M2 Pro, 12 cores, 16 GiB, macOS 26.5.2 arm64**.
The charter assumes an M3 Max with 64 GB (`docs/ASSUMPTIONS.md` A-001).
**No number produced here is comparable to a figure from an M3 Max**, and every
record must carry the host string.

## Method rule discovered in Milestone 0

**Client JS must be measured over the transitive module graph, not the entry
chunk.** Counting only `<script src>` reported 176 bytes where the true cost was
3,921 — a 20x understatement in our own favour. Milestone 3's baseline
comparisons must use the same transitive method for every stack, or the
comparison is dishonest. See `spikes/marko-stream-resume` finding F-7 and risk R14.

Report the split, not one number: a shared runtime chunk amortized across routes,
plus route-specific code. Reporting either alone is misleading.

Count **downloaded** and **inline** JS separately: they cost differently
(request + parse + cache vs bytes in the HTML).

## Milestone 0 spike measurements — NOT benchmarks

Single runs, one machine, localhost, no shaping, no distribution. Recorded as
evidence for the Milestone 0 gate only. Raw output: `docs/evidence/M0/`.

```text
marko 6.3.32 / @marko/run 0.11.8 / vite 8.2.0 / node v22.21.1

route            html    html.gz  downloaded JS  inline JS  script tags
/static           588        357          0 B        0 B    0
/stream          1669        986          0 B      849 B    3 inline
/counter         1721        930       3921 B      350 B    1 ext + 1 inline
/counter-large  16537       2468       3927 B      356 B    1 ext + 1 inline

streaming   shell 3.1 ms | 400 ms subtree at 408.2 ms | 1200 ms subtree at 1206.0 ms
            chunked, 3 chunks, response complete 1206.7 ms
resumption  HTML 9.6x larger -> route-specific client JS 1.03x (176 B -> 182 B)
            over a shared 3,745 B runtime chunk

wasmtime 47.0.3 / wit-bindgen 0.60.0 / rustc 1.97.1 / wasm32-wasip2 (@0.2.9)

component        size       imported WASI instances
std guest       43,837 B    14 (+1 declared)
no_std guest     5,276 B     0 (+1 declared)
```

## Aspirational budgets to test (charter §18.4)

Targets, not facts. If one is missed, record why and decide whether the
semantics, the implementation, or the target should change.

```text
static page: 0 application JS and 0 runtime JS        <- MET by marko /static
interactive runtime: single-digit KB compressed        <- 2.2 kB gzip so far
one in-flight request per resource key by default      <- Milestone 4
no whole-tree hydration                                <- MET (1.03x over 9.6x HTML)
unrelated resource change: no unrelated DOM updates    <- Milestone 7
unrelated store invalidation: no unrelated regeneration<- Milestone 6
compiler feedback: interactive after warm cache        <- Milestone 9
```

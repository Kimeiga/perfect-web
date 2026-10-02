# ADR-0123: the development server runs what `pw build` built

Status: accepted under the owner's instruction of 2026-10-02 ("start the
offline benchmark harness"). Date: 2026-10-02. Milestone: E14 (E14-B).

## Context

E14's harness gives an agent a copy of the store's program, lets it change
it, builds it, and grades the running result. For Pleris that only measures
the agent if the running result is what its program says. It was not:

- the command components and their contracts were read from
  `docs/evidence/E10/*.wasm` and `docs/evidence/E8/component-contracts.json`,
  the committed build of the repository's own store;
- the resource graph was compiled into the server binary
  (`include_str!` of `runtime/pw-materialize/tests/store-graph.json`);
- only the templates, handler modules and speculations came from the
  program being run.

So a sandbox that removed `idempotent_by` from `add_to_cart` and rebuilt kept
running the committed `add_to_cart`, with its committed contract. Every
Pleris benchmark result about a command would have been about the
repository's store, not the agent's.

## Decision

1. **`pw build` writes everything a host runs**: it adds `graph.json` (the
   graph `pw emit-graph` prints, from the same derivation) and
   `speculations/` (ADR-0122) to the templates, contracts, handlers and
   components it already wrote. A dangling graph edge and an unexecutable
   speculation are build refusals.
2. **The development server loads one build directory** (`Server::from_build`):
   templates, contracts, every component in `components/`, the graph,
   handlers and speculations. `pw-dev-server DIST [BUILD]`, `BUILD` defaulting
   to `DIST/build`. The committed-artifact loaders remain, test-only, for the
   unit tests that pin behaviour to the committed build.
3. **`run.sh` builds through `pw build`** and takes `PW_SOURCES`, `PW_OUT` and
   `PW_STORE_IR`, so the harness can build a sandbox's program without
   touching the repository's.

## Not claimed

**The store's query values are still computed in Rust.** The server fills
`store.name`, the menu and the cart count from its own state, as E10's build
evidence says ("the queries compile and are audited; the server still
computes the page's values itself"). A Pleris agent's change to a query's
body, freshness or concurrency therefore does not change what the page shows.
Benchmark tasks about query behaviour (T02, T07, T09) cannot be graded on
Pleris until the queries run as components; that is E10-S10's neighbour and
is recorded in EVIDENCE_LEDGER as E14-Q.

## Acceptance

- `pw build` on the store writes `graph.json` identical to the committed
  `store-graph.json`.
- The server's unit tests pass on the test-only loaders, and the own-renderer
  suite passes in WebKit (109 of 109) against `from_build`.
- E14's T08 negative control: the sandbox without `idempotent_by` double-adds,
  which it could not do if the server ran the committed contracts.

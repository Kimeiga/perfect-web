# Perfect Web / Pleris

A clean-slate research language and platform for web applications. Application
code describes domain values, state transitions, permissions, privacy, freshness,
consistency, resources, and semantic HTML. The compiler and platform derive
placement, interfaces, rendering, caching, invalidation, scheduling, and cleanup.

The compatibility target is ordinary browsers. A custom browser is not required;
browser-native experiments are a separate later research track.

[`PROJECT_CHARTER.md`](PROJECT_CHARTER.md) is the project constitution.
[`AGENTS.md`](AGENTS.md) summarizes the operating rules.

## Implementation status

Current milestone state, completed evidence, open gates, and reproduce commands
are owned by [`docs/STATUS.md`](docs/STATUS.md) and
[`docs/NEXT.md`](docs/NEXT.md). Read those before changing code. Older E0 summaries
are historical snapshots, not current implementation claims.

The failure-census addition does not close the reopened value-checking gates or
the compiled-command integration obligation. It does not merge the unfinished
resolved-signature migration branch.

## Web Failure Census

[Read the research report](research/failures/README.md). It includes the executive
gap analysis, hierarchical failure taxonomy, full evidence-backed table, duplication
audit, source/plan rejection obligations, runtime requirements, explicit domain
policies, DX and performance reviews, security threats, prior art, priorities, and
residual uncertainty.

[Implemented changes and test evidence](research/failures/IMPLEMENTATION.md) are
separate from proposed guarantees. The corpus validator checks structural integrity,
not that the compiler already prevents every listed failure.

## Development

```sh
just doctor       # Inspect the pinned environment without installing tools.
just bootstrap    # Set up the repository's pinned dependencies.
just ci           # Run the existing project checks.
```

The census and targeted attribution regression tests do not require the Rust build:

```sh
npm run test:failure-census
npm run test:layout-attribution
```

The optional browser instrumentation probe and its environment limitations are
recorded in the implementation report. It does not replace the renderer's
cross-browser gates.

## Repository map

`compiler/` contains the compiler; `runtime/` contains runtime/host work;
`examples/` contains source-language witnesses; `spikes/` contains focused
experiments; `docs/` contains decisions, status, and evidence; `research/failures/`
contains the census and proposed acceptance obligations.

## License

Dual licensed under [MIT](LICENSE-MIT) or [Apache-2.0](LICENSE-APACHE).

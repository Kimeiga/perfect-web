# Architecture

Source map reviewed 2026-09-16 from `49398dc02be6f2c3ea1a28fdfb77bf66c15266b6`
with the resolved-signature cutover.
This describes ownership and open boundaries, not a claim that every charter
feature is implemented. [STATUS](STATUS.md) owns completion claims and
[NEXT](NEXT.md) owns the current implementation order.

The previous architecture file began "No compiler exists yet". It is retained
byte-for-byte as [the E0-era architecture snapshot](ARCHITECTURE-history-2026-09-15.md).
The charter and accepted ADRs remain authoritative for design decisions.

## Compiler ownership

`compiler/pw-syntax` owns syntax. `compiler/pw-core` owns HIR, resolution,
semantic analyses, diagnostics, manifests, contracts and backend work.
`compiler/pw-cli` exposes commands and renders results; it is not a second
semantic checker. Existing analysis modules include effects, privacy, placement,
resources, scopes, templates, resumption, binding, WIT and Wasm emission.

The Pleris declaration is the ABI authority for operations declared by the
program. An operation's identity is distinct from the capabilities authorizing
it (ADR-0026). Generated projections must not become independently maintained
semantic authorities.

`Signature` now owns recursive semantic parameter/return slots. Missing and
unresolved annotations are explicit alternatives. `Interface` is a projection
of that signature, not a second derivation from syntax. Member and boundary
facts are keyed by resolved receiver identity; WIT and contract identities are
derived projections. The remaining source-fragment readers use the language's
existing type grammar, not independent string splitters (ADR-0030).

Comprehensive ordinary-call argument/return checking remains unfinished. The
backend's remaining internal value-layout approximations and E10-I integration
are separate obligations; a resolved callable signature does not prove a full
backend implementation.

## Runtime ownership

| Area | Source location | Responsibility represented in the repository |
|---|---|---|
| Resources | `runtime/pw-resource` | Resource entry identity and resource behavior |
| Tasks | `runtime/pw-tasks` | Scoped task runtime work |
| Materialization | `runtime/pw-materialize` | Stored materializations and outbox work |
| Document identity | `runtime/pw-document` | Document-part address vocabulary |
| Transport contract | `runtime/pw-protocol` | Patch and recovery protocol data |
| Rendering | `runtime/pw-render` | Renderer implementation |
| Resumption | `runtime/pw-resume`, `runtime/pw-resume-wasm` | Resumption decisions and Wasm wrapper |
| Host | `runtime/pw-host` | Component contracts, admission and engine integration |

These locations identify ownership. Presence of a crate does not establish its
entire charter contract. One semantic resource-entry identity has distinct storage
and wire projections; one operation can require multiple capabilities, and two
operations can share an authority without sharing an ABI.

## Open end-to-end boundary

E10-I requires `add_to_cart` compiled through the production Pleris component
backend, executed through the E8 host, with no alternate Rust closure path.
Host-layer tests and core Wasm validation are separate evidence and must not be
combined into that unperformed integration claim. ADR-0023 records the boundary.

## Evidence and proposed extensions

The [failure census](../research/failures/README.md) connects proposed rules to
invariants, assumptions, enforcement boundaries and adversarial obligations.
ADR-0027 separates specification, representation, implementation and observed
behavior. The [recipe-gate repair](evidence/tooling/evidence-gates-2026-09-15.md)
protects the exit-status path by which some of that evidence is produced.

Temporal authority, compatibility across live versions, commitment outcomes,
browser-owned state and composed budgets remain explicit design/implementation
obligations where their census records say so. No new language semantics or
production-readiness claim is introduced by this documentation repair.

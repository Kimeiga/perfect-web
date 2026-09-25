# ADR-0034: `pw build` — one program, every artifact

Status: accepted under the owner's instruction of 2026-09-24 ("finish the rest
of E10"); implemented with the evidence linked below. The decision marked
**(ruling needed)** was made without an architect ruling and is offered for
reversal.
Date: 2026-09-25. Milestone: E10 gate item 1, "Store application builds from
source to browser artifacts and Wasm Components without Koka or Marko."

## Context

After the compiled handlers (ADR-0033), every artifact of the store could be
produced, but by four commands:

- `emit-template`;
- `emit-handlers`;
- `emit-component`, once per id and only for the two commands;
- `emit-contracts` and `emit-wit`.

The three queries were compiled only inside a test. Nothing showed the whole
program building, and nothing showed that Koka and Marko were not involved.

## Decision

### 1. `pw_core::build::build` composes the stages that own each artifact

| artifact | owner |
|---|---|
| template IR, with handler identities | `build::templates` (shared with `pw emit-template`) |
| handler modules | `backend::js::compile` |
| components | `backend::component::compile_all`, each audited against its world |
| contracts, WIT | `contract::contracts`, `wit::package` |

`compile` and `compile_all` share one private setup (`with_program`: check,
lower, WIT), so a component cannot be built two ways. `pw emit-template` now
calls `build::templates`. It had re-read every file from disk with
`unwrap_or_default()`, which would have turned a read error into an empty
source.

`pw build --out DIR` writes the lot. It is all or nothing: one refusal writes no
file and fails the build.

### 2. Each contract has exactly one outcome

- **Component:** a command or query, compiled and audited.
- **No body:** a page or view, which the template IR renders, or a resource,
  which declares a lifecycle.
- **Placeholder:** a command or query whose body is `todo`.
- **Refused:** a command or query the backend declined, with that
  declaration's own reasons. `lower::program_by_declaration` now pairs each
  refusal with its declaration; before, one refusal message quoted every other
  refusal in the program.

### 3. A placeholder is not a refusal until something depends on it (ruling needed)

The library's `Resources.Cart`, `Menu`, `Order` and `Store` are `query X(..) {
todo }`. Their author has not written them. That is different from the backend
declining something it was asked to compile. A build lists placeholders and
succeeds. But if any contract imports a placeholder as a component, the build
fails and says which contract depends on which placeholder: a page would
render by calling a body nobody wrote.

The alternative is to fail any build containing a `todo`. That would stop the
store from building over four declarations nothing in it calls.

### 4. The gate's evidence runs the build where Koka and Marko cannot be reached

`just e10-build` runs `pw build` with `PATH=/usr/bin:/bin`. The recorded
control shows `koka`, `node`, `npm` and `pnpm` on the ordinary PATH and absent
from the build's PATH. Marko is an npm package, so without Node it cannot run.
The build's outputs are then compared byte for byte with the artifacts recorded
separately (`evidence_is_current` holds those to the compiler).

## Acceptance

- `compiler/pw-core/tests/build.rs`:
  - the store's exact outcome per contract;
  - a placeholder depended on is a refusal, and the same program with the body
    written builds;
  - an unchecked program builds nothing.
- Evidence: [build.txt](../evidence/E10/build.txt), from `just e10-build`.

## Consequences

E10 gate item 1 is met for the store as it stands. Not claimed:

- **The queries do not run as components in the development server.** They
  compile and are audited, and the server computes the page's values itself.
- **The data layer is the deployment's** (NEXT step 10).

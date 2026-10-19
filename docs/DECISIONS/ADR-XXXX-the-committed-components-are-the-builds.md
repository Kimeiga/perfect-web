# ADR-XXXX: the committed components are the build's, every one

Status: accepted, the integrator's ruling of 2026-10-19 (train-11), under the
owner's delegation of 2026-10-02; applied by the integrator's delegate
assembling train-11. Builds on ADR-0125 (the store's committed page plan and
components, which the development server's tests run without linking the
compiler).

## Context

`docs/evidence/E10/<id>.wasm` holds the store's compiled components: the
development server's tests and pw-host's run them without linking the
compiler. Three lists named them, each kept by hand: `just e10-component`'s
(20 ids), the server's test loader's (20), and
`evidence_is_current`'s (14). The test held 14 of the 20 committed to the
compiler; of the six it left out, three were stale at train-10's head
(587657ff): `store.hours.StoreStatus`, `StoresOpen` and `CartHours`, last
emitted 2026-10-13, before scheduled-orders and reviews changed the hours
module (`CartHours` committed 31,114 bytes, built 16,628 now). No test saw
it. The store's build emits 55 components.

## Decision

- **The test's list is the build's**: `evidence_is_current`'s
  `the_committed_components_are_what_the_compiler_builds_now` builds the
  store as `pw build` does and holds `docs/evidence/E10/*.wasm` to exactly
  its components (`Build::components`, each `Built::Component`): each built
  one committed, byte for byte, and none committed that the build no longer
  emits. Never a list kept beside the build.
- **`just e10-component` commits what the build emits, whole**: `pw build
  --out`, then every `components/*.wasm` (the old ones removed first). The
  bytes are `pw emit-component`'s (checked equal for `StoreStatus` and
  `CartHours`, and for the 17 committed ones that were current). 55
  components are committed (600 KB), 35 of them new.
- **A control**: `a_component_the_build_adds_fails_the_check_until_it_is_committed_current`
  builds the store with one query more and shows the check naming it as
  not committed, then stale, then nothing once committed as built; and a
  committed component the build no longer emits named too.
- The server's test loader keeps its own list: it loads what its tests
  run, and a component it needs and lacks fails them loudly; staleness was
  the silent failure.

## Evidence

- Red before: with the old committed set, the derived check names 3 stale
  components and 35 built and not committed; green after `just
  e10-component`.
- `scripts/committed_components_mutations.py`: 5 of 5 mutants killed (the
  build's components not read; a component not committed, committed stale,
  or committed and no longer built, passing; a kept 14 of them checked).
  Recipe `e14-committed-components` (`just/committed-components.just`).

## Not claimed

- Other committed fixtures kept by hand lists: the store's template IR
  (`spikes/own-renderer/store-ir.json`, which `run.sh` rewrites) is not held
  here; `wip/host-map-all-2026-10-19` (a520282f) has a check of it for the
  integrator.

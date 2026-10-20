# ADR-XXXX: component identity retains the declaration namespace

Status: integrator-approved policy, 2026-10-20; implementation in progress.
Milestone: E14; A-008 follow-up to ADR-0361.

## Finding

The accepted corpus deliberately permits a query and a view both called
Recommendations in store.recommendations. They are distinct Term and Ui
resolver declarations. contract::component_id erased that distinction, emitting
two identical contract IDs; WIT then overwrote one declaration lookup and emitted
two interfaces/worlds with identical names. Renaming only WIT cannot repair the
underlying contract identity.

## Decision

The canonical contract::component_id keeps existing Term IDs and appends ~ui to
every Ui namespace declaration. The marker is portable in artifact filenames and
cannot occur in a Pleris source identifier or module path. The identity is stable
when other declarations are added; no whole-program collision suffix is used.
Consumers that need component identity use this authority, including template and
graph projections. Type and effect declarations do not become component identities.

The WIT projection retains its existing collision-refusal policy: when distinct
canonical IDs mangle to the same WIT identifier, generation refuses them rather
than merging declarations. A crafted neighbor tests this boundary. No new WIT
parser, semantic namespace, or language feature is introduced.

## Compatibility

UI contract IDs, associated graph/template/page identities, and generated artifacts
change. Term IDs and ordinary callable exports stay unchanged. Existing generated
fixtures must be regenerated through their producing commands and behavior tested.
This repair is not a compatibility guarantee for an already running old UI build.

## Acceptance

- Red-before-green same-name Term/Ui complete build and separate contract/WIT IDs.
- Distinct signatures/declaration identities; both declaration orders and split files.
- Actual accepted A-008 complete check/build before removing its named refusal.
- Mangling collision negative control, mutation controls, relevant whole campaigns,
  and CI on the final source. Evidence and further limitations will be recorded here.
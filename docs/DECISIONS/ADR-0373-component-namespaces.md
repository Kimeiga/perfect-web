# ADR-0373: component identity retains the declaration namespace

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
## Consumer audit

The same-name browser fixture revealed that view-tag checking and two template
lookups searched every namespace, choosing a query before its same-named view.
They now resolve the Ui namespace, matching the existing value checker. A tag
without a Ui declaration keeps the existing diagnostic about its other declaration.
The fixture proves both query results, nested view markup, derived page bindings,
and resumed signal updates. The view-tag regression failed with PW5020 before
this repair. A semantic review regression also failed before repair: changing
query freshness beside a same-named view produced no change report.

Review declarations use one namespace-aware helper: Term and Ui agree with the
canonical component identity; Type, Effect, Event and Predicate use review-only
~type, ~effect, ~event and ~predicate suffixes. The separate Model.types map and
written type references retain raw type paths. A query/type freshness witness
also failed before repair and covers both declaration orders. No noncomponent
WIT identity changes are made by this review repair.

A second concrete map-overwrite witness is a query Same followed by event Same:
build exited successfully with one query contract and zero executable components,
reporting the query as a bodyless Event. Reversing source order emitted the query.
WIT's contract-declaration map now admits Term/Ui namespaces only, the namespaces
of its component and callable declarations. Other namespaces keep their existing
WIT naming; they cannot replace a contract's declaration. The regression requires
both source orders to retain the query's signature and audited executable body.


### Accepted corpus integration consequence (2026-10-20)

Repairing A-008's generated-WIT collision exposes six pre-existing backend or
fixture-placeholder refusals that the old package failure masked. Historical
`115895de` full-corpus four-refusal output becomes nine when only A-008 is omitted
from the probe's arguments; integrated `d8369604` full-corpus output has those
same nine. The unchanged A-008 now builds its actual query and view separately.
[ADR-0361's corpus census amendment](ADR-0361-check-is-build.md#corpus-census-after-the-wit-collision-repair-2026-10-20)
records the integrator's explicit correction, all six limitations and raw proof.
Complete validation remains fail closed and frozen `.pw` sources remain unchanged.


## Landing (2026-10-21)

Landed on `master` with train-12 at `e7664d2eb01aa8c6e91df594bd7270a84a8bf8de`
after exact-commit CI ([39584219053](https://github.com/studiokimeiga/perfect-web/actions/runs/39584219053))
and automatic track verification ([39575516650](https://github.com/studiokimeiga/perfect-web/actions/runs/39575516650)).
Dated statements above that call landing or combined gates pending describe
the candidate before it landed and are kept as written. The
[acceptance record](../diagnostics/train-12/acceptance-record-2026-10-21.md)
states what the landing's evidence does and does not prove.

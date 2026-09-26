# ADR-0075: a stream and a mounted resource do not build

Status: accepted under the owner's instruction of 2026-09-26 ("keep working
until its perfect"). Date: 2026-09-26. Milestone: E10 (charter §8.4, §8.5).

## Context

The template IR has no representation for a streamed region or for an
element that mounts a resource. Found writing ADR-0074, on 2026-09-26 at
e688739, each lowered as a literal element:
- **A-008's `<stream query={Recommendations(id)}>`** lowered as an element
  named `stream`, its `<placeholder>`, `<ready>` and `<failed>` parts as
  elements too. `pw build` refused it, for the wrong reason: "`query` is
  read by path, and this is a call" (ADR-0073).
- **A-007's `<map-container resource={StoreMap} center={center} />`**
  built: an element with two attributes. Nothing mounts the resource, and
  the page fails when rendered, since `center` is a record.

Both are valid Pleris, and check. The Marko adapter renders a stream
(ADR-0017); nothing compiles a mounted resource.

## Decision

- **The template IR refuses both by name**, so `pw build` refuses them for
  what they are:
  - "a `<stream>` is not compiled by this renderer; the Marko adapter
    renders one (ADR-0017)";
  - "an element that mounts a resource is not compiled".
- **An element mounts a resource when its `resource` attribute holds a
  value**, `resource={StoreMap}`. RDFa's `resource="/x"` is an ordinary
  attribute, and an attribute beside it is text (ADR-0074). The value
  relations and the template IR read this one predicate.

## Acceptance

- **`compiler/pw-core/tests/streams_and_resources.rs`**, 3 tests. A-008
  and A-007 themselves check and do not build, each for its reason. RDFa's
  `resource` builds, and the attribute beside it is related as text. The
  tests fail at e688739, the commit before.
- **No rejected, rule or generality fixture's diagnostics change.** The
  store, kiokun and the accepted corpus check clean, and the store's and
  kiokun's artifacts are byte-identical, apart from ADR-0058's two
  handlers.
- **Mutation controls:** `scripts/stream_mutations.py`, `just e10-streams`,
  3 mutants.

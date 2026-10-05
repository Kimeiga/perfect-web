# ADR-0134: every handler is resumable, and what it captures is what it reads

Status: accepted under the owner's delegation of 2026-10-02. Date: 2026-10-02.
Milestone: E14. Found building ADR-0133.

## Context

A handler ran in the browser only if it was written `resumable(captures =
{ .. }) => ..`. Anything else written in an `on:` attribute was not a
handler to any analysis:
- `on:press={() => count = count + 1}` checked and built;
- its event part had no identity, the build compiled no module for it, and
  the page shipped a button that did nothing when pressed.

Nothing said so. A silent dead button is the kind of defect Pleris exists
to make unwritable, and the ceremony that avoided it, a capture list
written by hand, is what Qwik and Marko infer. A second defect sat under
the first: `() => e` lowered with one parameter, a unit or an error
pattern, so even a resumable `() =>` was refused as a handler taking the
event.

## Decision

1. **Every lambda in an `on:` attribute is a handler**
   (`resume::handlers_in`). One written `resumable(..)` still is, wherever
   it is.
2. **What a handler captures is one derivation, `resume::captures_of`,**
   which every analysis reads: the resume manifest and its artifact, the
   capture checks, the template's capture paths and the handler's lowering.
   - With a list, `resumable(captures = { item })`, it captures what it
     lists, and PW5025 holds it to reading nothing else, as before.
   - Without one, it captures what it reads that the body around it binds:
     a parameter, a binding, a loop's item, an arm's name. That is exactly
     the list it would have had to write.
   - It never captures what it binds itself, a declaration (its code is
     compiled, not carried), or a signal (the browser holds it).
   - The checks that hold a capture to the boundary (PW5016, R-010, R-030,
     the browser's grants) apply to an inferred capture as to a listed one.
3. **The template carries what the manifest derived.** A handler's capture
   paths are computed with its identity, where names are resolved, and the
   template IR is given both.
4. **`() =>` takes no parameter.**
5. **An event part with no code is a build refusal.** What remains without
   code is a handler that is not a lambda, `on:submit={save}`, which waits
   for the event to be passed (ADR-0131). It is refused at build, with the
   repair, instead of shipping inert.
   Superseded in part by ADR-0199: a function or a command named as a
   handler is the lambda that calls it, and anything else named there is
   refused when the program is checked.

## Alternatives

- **Refuse a handler not written `resumable(..)`.** Rejected. It closes the
  silent failure at the cost of the ceremony, and an agent (E14) writes
  `() =>` first. Inference gives the same captures by construction.
- **A check-time diagnostic for a non-lambda handler.** Deferred. Several
  corpus fixtures write `on:submit={on_press}` to test other invariants, and
  the event parameter (ADR-0131) will make most such handlers compile.

## Consequences

- No accepted, store or kiokun program changes, and no rejected or
  generality fixture's diagnostics change. The store's handlers keep their
  identities (`e1ab9fca1f6fc15b`, `5e53c6a9aaee307a`), so its committed
  modules are unchanged.
- `examples/demo/panel.pw` writes its handlers `() => ..`, and the
  own-renderer suite passes across its browsers (351 tests).

## Acceptance

- **`compiler/pw-core/tests/every_handler_is_resumable.rs`, 4 tests.**
  Three fail at the commit before; the fourth is PW5025's control:
  - a `() =>` handler has an identity and a module;
  - captures are inferred: a loop's item read by a field, as the listed form
    carries it, and a page parameter whole. Controls: a handler's own binding
    and a signal are not carried;
  - a listed capture is still held to what the handler reads (PW5025);
  - a handler that is not a lambda is refused at build.
- **Mutation controls:** `scripts/handlers_resumable_mutations.py`, `just
  e14-handlers-resumable`, 7 mutants.

# ADR-0061: a declared sum type in a template's `{#match}`

Status: accepted under the owner's instruction of 2026-09-26 ("keep working
until its perfect"). Decisions marked **(ruling needed)** were made without a
ruling and are offered for reversal. Date: 2026-09-26. Milestone: E10
(ADR-0042's template branches, extended).

## Context

ADR-0042 gave templates `{#match e}{:Some(x)} .. {:None} .. {/match}`, over
an `Option` or a `Result`. A declared sum type's cases were refused
(PW5019), "as the component backend refuses them". ADR-0059 made the
backend build and match them, and ADR-0060 nested patterns, so the reason
no longer held. KNOWN_LIMITATIONS: "Template matches take apart `Option` and
`Result` only."

An arm's marker also bound at most one name. A case of several fields,
`Rect(Int, Int)`, could not have been taken apart in a template even if the
subject had been admitted.

## Decision

### 1. What an arm is

- **An arm's marker names a case,** alone or through its type:
  `{:Circle(r)}`, `{:Shape.Circle(r)}`. It binds one name per field of the
  case's payload, `{:Rect(w, h)}`, or none, `{:Rect}`, which renders
  without reading the payload.
- **The HIR holds an arm as a `TemplateArm`:** the case as written, and a
  list of bindings. It held a case and one optional name.

### 2. What is checked

A `{#match}` over a declared sum type is checked as any match is:
- **A case the type does not declare is PW0608.** So is a qualifier that
  names another type: `{:Other.Rect(w, h)}` over a `Figure`.
- **A field count other than the case's is PW0603.** Binding none is
  allowed.
- **A case with no arm is PW0305.**
- **A second arm for one case can never render** (PW5019), as before.
- **An arm's names are typed** from the case's fields, where the type has
  no type arguments.

A subject of unknown type still takes its family from its first arm, as
ADR-0042 read `Option` and `Result`. A declared case there is PW5019: the
subject's type is not known.

**(ruling needed)**: a declared case named like the language's own (`Some`,
`None`, `Ok`, `Err`) is refused in a template (PW5019). A template reaches
its value by the case's name, and the value the component gives names the
language's cases so.

### 3. How it renders

- **The template IR names a declared case as a component value does,** by
  its WIT name: `circle` for `Circle`. The language's four keep their
  names. The renderer finds an arm by the value's case, so the two must
  agree, and the value comes from a component, which knows only the WIT
  name.
- **A case of several fields carries them as a list.** A server converting
  a component value turns the case's `tuple` payload into a list. The IR
  arm's `fields` bind each element, and `binding` still binds a payload of
  one field. The IR's JSON is unchanged for every arm but one of several
  fields.
- **kiokun's server converts a declared case** (`Val::Variant`), and a
  `tuple`, where it dropped both. The store's development server builds
  its values by hand and reaches no sum type.

## Acceptance

- **`compiler/pw-core/tests/template_blocks.rs`:**
  - a declared sum type is taken apart, and its IR names each case by its
    WIT name and binds a case's several fields;
  - a missing case (PW0305);
  - a case the type lacks (PW0608);
  - a field count (PW0603);
  - a qualifier naming another type (PW0608).
  - The table's PW5019 case for a `Shape` subject is gone. The case for an
    untyped subject now says the subject's type is not known.
- **`runtime/pw-render/tests/branches.rs`:** a declared case renders with
  each of its fields bound, and a payload whose fields the arm does not
  have is refused.
- **kiokun's server:** `Rect(3, 4)` from a component reaches the renderer as
  `rect` with a list of two.
- **Mutation controls:** `scripts/template_match_mutations.py`,
  `just e10-template-sum-types`, 10 mutants.

## Not done

- **A pattern nested in a template arm**, `{:Some(Circle(r))}`, or a
  literal one. A template arm takes one case apart, and the runtime has no
  decision tree.
- **Patches for a `{#match}` region.** It renders on the server, as
  ADR-0042's matches do, and the development server's patch generator is
  written per operation.
- **An arm's names under a generic sum type** are not typed.

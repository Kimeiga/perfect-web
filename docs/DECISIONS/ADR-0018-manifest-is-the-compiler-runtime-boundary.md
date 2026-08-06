# ADR-0018 — The resource manifest is the compiler/runtime boundary, and it is data

**Status:** Accepted
**Date:** 2026-08-06
**Milestone:** E4
**Depends on:** ADR-0014 (HIR), ADR-0016 (the runtime is behaviour, not guarantees)

## Context

E4 landed two halves that did not touch. `hir::Policy` recorded what an author
wrote — `freshness 30.seconds`, `retry bounded_exponential(max = 3)` — and
`pw_resource::Manifest` was constructed by hand in tests. Nothing connected
them, so "the runtime honours the declared policy" was a statement about two
independent pieces of code that happened to agree.

`docs/STATUS.md` named this as the single thing E4 gate 1 and E3 gate 3 both
waited on.

The obvious implementation is a code generator: read the manifest, emit Rust
that calls `pw_resource`. That is wrong here. E8 has not chosen the host
execution model, and a generator emitting Rust would make `pw-resource` the
only possible runtime by making the compiler depend on its API.

## Decision

**The manifest is a serialized data artifact. Neither crate depends on the
other; both read the schema.**

```text
.pw source
    ↓  parse, lower                         pw-syntax → pw-core::hir
    ↓  pw-core::manifest::build             policy values interpreted here
    ↓  JSON  (pw emit-manifest)             ← the boundary
    ↓  a host reads it                      pw-resource today, E8's choice later
```

Three properties make it work:

1. **Policy values are interpreted in the manifest, not in the parser.**
   `30.seconds` is a duration only because `freshness` says so; `one_per_key` is
   a concurrency mode only because `concurrency` says so. Parsing them in the
   grammar would bake one policy's vocabulary into the syntax of every other,
   and adding a policy would change the language.

2. **A value the schema cannot read is reported, never defaulted.** A manifest
   with a silently defaulted freshness is worse than no manifest: the runtime
   serves stale data and nothing says why. `pw emit-manifest` exits non-zero
   when any value is not understood.

3. **The schema can express illegal declarations.** `Retry::Forever` is
   representable, because the rule that rejects it (`PW0313`) belongs in the
   checker where it can explain itself. A schema that could not represent an
   illegal policy would move the rule into parsing, where the diagnostic would
   be "syntax error".

### Where the conversion lives

In the **test**, not in either crate. `pw-resource` has `pw-core` as a
*dev*-dependency only. That is deliberate: the correspondence between a declared
policy and runtime behaviour is exactly the thing to test, and putting the
conversion in either crate would make one depend on the other and turn the test
into a tautology.

## What this is, and is not, evidence for

**Is evidence for:** that a policy written in `.pw` governs what the runtime
does. `runtime/pw-resource/tests/from_manifest.rs` edits the source, regenerates
the manifest, and watches the freshness boundary move.

**Is not evidence for:**

- **that a `.pw` program runs.** There is no code generator producing calls into
  the runtime; a host still writes the fetch itself. The manifest tells it *how*
  to behave, not *when* to run.
- **that the policies are enforced.** The manifest records them; E5's checker
  rejects some illegal combinations; nothing yet stops a host ignoring the file.

## Consequences

- E8 can choose any execution model and read the same artifact.
- Adding a policy means adding a field and a parser for its vocabulary, both in
  one module, with the corpus coverage test as the acceptance criterion.
- Cost: a schema to version once anything outside this repository reads it.
  Not yet — nothing does.

## Revisit when

E8 selects the host execution model, or the manifest needs to describe something
that is not data — a policy whose meaning is a function rather than a value.

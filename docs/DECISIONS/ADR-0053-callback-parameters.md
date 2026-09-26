# ADR-0053: a lambda's parameters take the types its use declares

Status: accepted under the owner's instruction of 2026-09-25 ("ok fix all the
known gaps"). Decisions marked **(ruling needed)** were made without a ruling
and are offered for reversal. Date: 2026-09-25. Milestone: E10 (a checker
correction, found while compiling opaque values for ADR-0054).

## Context

A lambda's parameters were typed in two places, and neither read the
callee's declaration:

- **`infer.rs`** gave a callback's FIRST parameter the element type of any
  list passed beside it. The rule came from the architect's ruling of
  2026-08-07: `el.getBoundingClientRect()` resolves through `el`'s type, not
  through the spelling. Its comment called the rule "deliberately narrow".
  The value relations and the effect checker both start from these
  bindings.
- **`values.rs`** typed a lambda's body against the instantiated function
  type of the parameter it was passed as. But it did so only while that one
  call was being solved. The walk that relates operands, members and
  conditions reads a lambda's body on its own, with `infer.rs`'s bindings.

`fold`'s callback is `fn(A, T) -> A`, whose first parameter is the
accumulator. So `List.fold(ws, 0, (t, w) => t + w.score)` gave `t` the type
`Word`, and was refused: "the left side of `+` must be `Int or Float`, and
this is `t.Word`". It failed with the compiler before this change too.

The same rule gave a program's own callback taker the wrong types:
`render(ws, (acc, w) => .. String.length(acc) ..)`, with
`f: fn(String, Word) -> String`, was refused (PW0605) with `acc` typed as the
word. Every parameter but the first was untyped, so nothing in a lambda's
body that read one was checked:
- `fold`'s element;
- a lambda bound with a written type, `let inc: fn(String) -> Int = x => x + 1`;
- a returned lambda.

## Decision

### 1. `infer.rs`: a parameter's type is what the callee declares

A lambda passed to a call is typed from the callee's signature. The piped
value is the call's first argument. A parameter is typed where the declared
function type gives it either:
- a type that mentions no type parameter, as written; or
- exactly the type parameter that a list argument's element instantiates.
  `items: List<T>` passed a `List<Word>` makes `T` a `Word`.

Anything else needs the call solved, which `values.rs` does. A call with a
named argument is skipped, since a position is not a parameter there.

**(ruling needed)**: a callee with no signature types no parameter. The
first-parameter rule typed one anyway, whatever the callee was.

### 2. `values.rs`: a lambda's parameters are what its use gives them

A lambda is typed against the function type it is used as:
- the parameter it is passed as, with the call solved;
- a `let`'s written type;
- the declared result it is returned as, seen through the branches of the
  returned value.

Each parameter keeps the closed type its use gives it, for the whole body,
in the walk that relates operands, members and conditions. A type with a hole
is not kept, so a list typed only by a later binding types its lambda in a
later round. The use is solved in the same bounded rounds as an unannotated
`let`.

A name bound at more than one site in a body stays unknown, as it was. The
environment is flat, and cannot say which binding a use means.

## Acceptance

- `compiler/pw-core/tests/callback_parameters.rs`, 8 tests:
  - `fold`'s accumulator and element are each typed and checked;
  - a program's own callback takes its declared types;
  - a lambda bound with a written type, and a returned lambda;
  - a piped list's callback;
  - a lambda over a list bound by a `let`;
  - a name two lambdas bind is not guessed.
- `compiler/pw-core/tests/causal_evidence.rs`: the effect chain reaches
  `getBoundingClientRect` through the receiver's type, through `fold`'s
  element and through a program's own callback. The first fails against the
  previous rule.
  - The fixture's `List.map` was the pre-ADR-0031 stand-in,
    `map(items: List<Unknown>, f: Decoder)`. It is now `list.pw`'s
    declaration.
- Every fixture reports what it did against a build of `01a21c7`, and
  kiokun's program checks clean.
- `compiler/pw-conformance/tests/opaque.rs` (ADR-0054) folds a list of an
  opaque type, which this refused.
- Mutation controls: `scripts/callback_mutations.py`, `just e10-callbacks`,
  8 mutants.

## Not done

- **A name bound at two sites is unknown** in the value relations, whatever
  it is bound to. A scoped environment would answer for each binding.
- **A lambda in a branch of an annotated binding's initialiser** is not typed
  by the annotation.

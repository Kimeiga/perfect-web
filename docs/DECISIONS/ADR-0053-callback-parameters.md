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

## Amended, 2026-10-11: a parameter known in part is checked by that part

Accepted under the owner's delegation of 2026-10-02.

W8 (track `store-accounts`) ran `just e10-callbacks` on a clean archive of
master at `7a701c2`, and two of its eight mutants survived. The nightly
verifications of 2026-10-08, -09 and -10 had found the same. The recorded
evidence was from `b8f9547`, before either change that let them survive. No
push ran the recipe again: a push runs a mutation script only where a mutant
sits near a changed line (ADR-0281), and neither change was near one.

- **"a parameter with a hole in its type is kept"** (`values.rs`).
  `Typer::lambda` kept a parameter's type only where it was known whole
  (`is_closed`). When this ADR was written, a type bound in one round was
  fixed for good, and the guard kept a hole from being fixed. ADR-0063 then
  made `Typer::bind` complete a type that a later round knows more of. Since
  then the guard has done only one thing: it left unchecked a parameter whose
  type is known in part. Take `fn each_list<T>(f: fn(List<T>) -> Int)`, where
  nothing fixes `T`. In `each_list(xs => xs + 1)`, `xs` is a `List<?>`, and it
  was not checked at all: `xs + 1` and `xs.score` were accepted.
  **The guard is removed.** A parameter keeps what is known of its type and is
  checked by it. An unknown part decides nothing (unification's `Undecided`),
  so only what the known part refuses is reported. The mutant now puts the
  guard back: "a parameter known only in part is not typed".
- **"any type parameter takes the list's element"** (`infer.rs`). Since
  ADR-0063 the value relations no longer start from `infer.rs`'s bindings.
  Its lambda typing is read only to resolve a member by its receiver's type,
  and no test held an accumulator to be anything other than the list's
  element there. `causal_evidence.rs`'s
  `a_folds_accumulator_is_not_its_lists_element` does. In
  `List.fold(items, 0, fn(total, el) total.getBoundingClientRect().width)`,
  `total` is no `ElementRef`, so the member resolves nothing through it and
  the effect does not reach the body.

Not claimed: a lambda passed before the argument that fixes its type
parameter, as in `later(xs => .., w)` with `fn later<T>(f: fn(List<T>) -> Int,
x: T)`. Such a lambda is typed by what the arguments before it fixed, so `xs`
is a `List<?>`, not a `List<Word>`. TypeScript types a context-sensitive
argument after the others. Every callback in `pw-std` is its function's last
parameter, so no program here meets this (KNOWN_LIMITATIONS).


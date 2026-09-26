# ADR-0063: every name means one binding

Status: accepted under the owner's instruction of 2026-09-26 ("keep working
until its perfect"). Decisions marked **(ruling needed)** were made without a
ruling and are offered for reversal. Date: 2026-09-26. Milestone: E10
(charter §7.1, the value checker; §7.8, privacy labels on values; §14 M10).

## Context

KNOWN_LIMITATIONS said: "A name bound at two sites in one body is unknown to
the value relations, whatever each binding holds (ADR-0053). Their
environment is flat and cannot say which binding a use means."

That understated it. Three analyses kept an environment per body keyed by a
name, each with its own rule for a name bound twice. None of them knew which
binding a use meant. The backend did know, because it lowers in order with a
scope. Each item below passed `pw check` on 2026-09-26.

**The value relations** read a name bound at two sites as unknown, wherever it
was used, so every relation through such a name was undecided:
- `Some(x) => x + 1` over an `Option<String>`, where another match also binds
  an `x`;
- `s => s + 1` over a `List<String>`, where another lambda also binds an `s`;
- `let k = "one"` then `k + 1` in an `else`, where the `then` branch also
  binds a `k`.

The names programs reuse most (`x`, `s`, `i`, `t`, `r`) were the ones least
checked. A `for` loop's name had no type at all, so `for x in xs { t = t + x
}` over a `List<String>` passed too.

**Two accepted programs disagreed.** A-017 passed its
`LayoutSnapshot<Float>` to `set_width`, which takes a `Float`. A-016 reads
its snapshots through `.value`, as assumption A-020 states. The loop's
`badge` had no type, so nothing checked A-017's call. It now reads `.value`
too. A-020 is open: under its alternative, a phase rule in which a snapshot
reads as its value after its measure, A-017's original was right, and A-016
would change.

**The declared-type environment** (`infer.rs`) gave a name its last
binding's type. It types a resumable handler's captures, and the handler
backend reads them from it. Give a page a second `{#each}` that binds `item`
to a `Store`, after the store's `{#each menu as item}`. The add-to-cart
handler's `item.id` was then typed as a `StoreId`, where the value is a
`MenuItemId`:
- the capture schema and the handler's identity named the wrong type;
- the build succeeded;
- the value relations checked the call against the first binding, and the
  backend compiled it against the second.

Where the second type had no `id`, the handler was refused, and a correct
page did not build.

**The privacy labels** (`labels.rs`) gave a name its first binding's label,
and gave no label to a `for` loop's name, a lambda's parameters or an
`{#each}` block's name. So each of these passed:
- `for t in tokens { log.public("{t}") }`, over a list of
  `Secret<Payments>`;
- the same through `List.map(tokens, t => log.public("{t}"))`;
- `{#each keys as k}<li>{k}</li>{/each}`, rendering secrets to the browser.

Logging one of those secrets directly is PW5006, and rendering one is PW5003.
A public value logged under a name that a secret also bound was refused, a
false PW5006.

## Decision

### 1. One rule for which binding a name means

`crate::lexical` resolves every local name in a body once. Each use of a name
means the innermost binding in scope where it is written, or no binding, and
then it is the program's own name, resolved as before. The scopes are the
ones the backend lowers with:
- **A declaration's parameters:** its whole body.
- **A `let` or a `use`:** the statements after it in its block. Not its own
  initialiser, which reads the binding before it.
- **A lambda's parameters, a match arm's names, and a `for` loop's names:**
  the body they head. A lambda's descriptor, `resumable(captures = { item
  })`, names what is around the lambda.
- **A template's `{#each xs as x}`:** the block's children. A `{:Some(x)}`
  marker binds from itself up to the next marker.
- **A policy term's binders:** its own tree. The tree sees the declaration's
  parameters and nothing the body binds. Until now a flat environment also
  showed it the body's `let`s. **(ruling needed)**
- **A pattern's name that is a case**, such as `None` or a case a visible
  type declares, tests and binds nothing. There is one predicate for this,
  `lexical::names_a_case`, and every reader uses it.
- **An or-pattern** binds its first alternative's names. The backend refuses
  an or-pattern that binds.

A binding is identified by where it is bound, never by its name:
- a parameter's position;
- a pattern's `PatternId`;
- a `use` statement;
- an `{#each}` block;
- a `{#match}` arm's marker and position;
- a policy term's binder.

### 2. The value relations type a binding

`values.rs` keeps its types by binding, so a name bound at two sites is two
entries, each typed:
- a parameter by its declared type;
- an annotated `let` by its annotation;
- an unannotated `let` or `use` by its initialiser;
- a lambda's parameters by the function type their use gives;
- a match arm's names by the scrutinee's payload;
- a `for` loop's name by its `List`'s element;
- an `{#each}`'s name by its collection's element. The collection is read
  from the directive as a name and the fields read from it, so `e.rows`
  inside `{:Some(e)}` is typed.
- a `{#match}` arm's names by the case its subject holds;
- a policy term's binders by its header;
- a record's shorthand field by the binding its name means.

The types are solved in rounds, at most eight. A binding keeps its first type
unless a later round completes it: `List<?>` may become `List<Int>`, and a
complete type is never changed.

The flat environment's `shadowed` set is gone. The guard that dropped a
binding typed with a callee's own `T` is gone too: the bindings no longer
start as `infer.rs`'s, and every type the typer records is closed over its
callee. ADR-0041's control for it is retired with it.

### 3. The declared-type environment types a binding

`infer::Types` keeps its types by binding and reads a name through the same
resolver:
- **A handler's capture** is typed by the binding its root's name means
  where the descriptor writes it (`resume::capture_roots`).
- **The effect checker's member lookup** takes its receiver's type from
  `Types::of`, so from the binding the receiver's name means.
- **An optimistic transition's binders** are seeded by term and position.

Its sources are unchanged: parameters, `self`, annotated `let`s, `{#each}`
and `{#match}` arms, callback parameters, and `let` initialisers. An
`{#each}` over a field path is typed by the value relations and not here, as
before.

### 4. The privacy labels label a binding

`labels.rs` keeps its labels by binding, and gives one to each name bound
over the elements of a value. An element of a labelled collection carries the
collection's label **(ruling needed)**:
- a `for` loop's names, from its iterable;
- an `{#each}` block's name, from its collection's first name's binding with
  each field's label joined on;
- a `{#match}` arm's names, from its subject, as a match arm's are;
- a lambda's parameters, where the lambda is passed to a call, from the join
  of the call's other arguments and a piped value **(ruling needed)**. This
  errs toward private: `List.fold(secrets, 0, (acc, s) => ..)` labels `acc`
  too.

A diagnostic's origin span is the binding's.

## Acceptance

- **`compiler/pw-core/tests/lexical_scope.rs`**, 14 tests, each with a
  control that the right program is not refused:
  - two lambdas, two branches, and two arms binding one name;
  - a lambda's parameter that shadows a `let`, and the `let` after it;
  - a `let` from the next statement, with its initialiser reading the one
    before;
  - a `for` loop's name;
  - a record's shorthand field;
  - an `{#each}` over a field of an arm's binding;
  - two template arms binding one name;
  - a secret logged through a `for` loop's name and through a lambda's
    parameter;
  - a name bound once public and once secret;
  - a secret rendered through an `{#each}`;
  - a handler's capture beside a second binding of its name.
- **Every one of the 14 fails at 6c07ec0,** the commit before this.
- **`match_exhaustiveness.rs`:** a name an arm binds again, which the flat
  environment left unknown, now finds the case its own type is missing
  (`Green`).
- **No rejected, rule or generality fixture's diagnostics change,** compared
  file by file with 6c07ec0's `pw`.
- **Every relation the value relations decided before is decided the same
  after,** relation by relation over the store, kiokun and the accepted
  corpus. Undecided relations fall from 58 to 20 in kiokun, from 43 to 41 in
  the store, and from 96 to 70 in the corpus.
- **The store's and kiokun's artifacts are byte-identical,** apart from
  ADR-0058's two handlers.
- **Mutation controls:** `scripts/lexical_mutations.py`, `just e10-lexical`,
  19 mutants.
  - The controls of ADR-0059 ("an arm's body is walked without its
    bindings"), ADR-0038 ("a match arm's names are not typed") and ADR-0042
    ("an arm's binding has no type") are re-anchored where this moved their
    code.
  - ADR-0041's "an unannotated binding of a generic call keeps the callee's
    `T`" is retired, with the guard it undid.

## Found on the way, and not done

- **A label through a declared function.** `List.get(tokens, 0)`, over a list
  of secrets, is public, and so is `List.map`'s result. A declaration's label
  is its contract, and `get` declares none for a result that holds its
  argument's element. Next: ADR-0064.
- **A record literal whose first field is shorthand,** `P { x }`, parses as
  the name `P` and a block `{ x }`, so `let p = P { x }` checks and means
  something else. The parser reads a brace as a record only as `{ }` or
  `{ name:`, so that `if x { .. }` stays a block. The fix is a grammar
  decision **(ruling needed)**; recorded in KNOWN_LIMITATIONS.
- **A value with a hole that means any type.** Kiokun's remaining 20
  undecided relations are all this kind: `None`, `[]`, `Err(e)` against a
  declared result, `todo`, and a generic opaque value built from its
  representation (`Secret("")`). Each is undecided where it is well-typed.

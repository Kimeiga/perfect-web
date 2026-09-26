# ADR-0066: a nested declaration sees the bindings around it

Status: accepted under the owner's instruction of 2026-09-26 ("keep working
until its perfect"). Decisions marked **(ruling needed)** were made without a
ruling and are offered for reversal. Date: 2026-09-26. Milestone: E10
(charter §7.1, the value checker; §7.8, privacy labels on values).

## Context

ADR-0063 gave every use of a name the binding it means, within one body.
Writing its follow-ups found five places that one body was not the whole
answer. Each item below passed `pw check`.

**A secret logged through a nested function.** A declaration may nest
another: a `fn` inside a `fn`, a `command` or a `component`. The nested body
was analysed alone. A name it read from the declaration around it was bound
to nothing by:
- the value relations, which left it untyped;
- the declared-type environment, which left it untyped too;
- the privacy labels, which left it unlabelled.

So this passed, where the same `log.public` in `outer`'s own body is PW5006:

```text
fn outer() -> () !{ log<Public>, secret<Payments> } {
    let key = secrets.payments()
    fn inner() -> () !{ log<Public>, secret<Payments> } {
        log.public("key {key}")
    }
    inner()
}
```

So did a nested function logging `acct.token`, a field its enclosing
parameter's type declares a secret. The name check alone saw the enclosing
body, and saw every binding of it, wherever each was.

**A nested declaration's annotations did not resolve.** A module listed only
its top-level declarations, so `Hir::module_of` gave a nested one no module.
Every annotation its body was read by was left unresolved ("the annotation's
declaring module is unavailable"), and nothing through its parameters was
checked. The rejected fixture R-002 had two defects no one saw because of
this:
- its nested `load` named a `Store` it never imported;
- `load` declared a `Store` where `Stores.get` answers a
  `Result<Store, StoreError>`.

Both are corrected. R-002 now emits only its one declared defect.

**Other bindings no one resolved.** A stream's parts, `<ready as={items}>` and
`<failed as={e}>`, and a clause's `release(h) { .. }` bound names that only
the name check knew. `{#each items as item}{item.nmae}` inside a stream's
`ready` part passed, as did `release(h) { count(h) }` where `h` is the
`Handle` the resource acquires and `count` takes an `Int`.

**A call to a function value out of its scope.** In `g(v)`, where `g` is bound
only in an earlier block, `g` was taken as a local, because the call check
read every name the body binds anywhere. The name check reads scopes, and
did not examine a call's callee.

**`(x) => x + 1`.** One parameter in parentheses was taken for a descriptor,
like `resumable(..)`. So `x` was bound to nothing, and its use was PW0021.

## Decision

### 1. A nested declaration sees what is in scope where it is written

A nested declaration's names resolve, innermost first, to:
- its own bindings and parameters;
- the enclosing declaration's parameters, and the bindings of the enclosing
  body in scope at the statement before it is written;
- the declaration around that one's, and so on out.

`lexical` records the scope at each nested declaration's position, and gives
each binding it sees a `Binder::Outer`, which names the enclosing declaration
and its binder there. Each analysis reads that binding's fact where the
enclosing declaration computed it:
- the value relations, from the enclosing typer's types;
- the declared-type environment and the labels, from the enclosing
  declaration's environment.

**(ruling needed)**: a nested declaration written before a `let` does not see
it, as a closure in a language with block scope would not. The alternative is
the name check's old rule, every binding of the enclosing body.

### 2. A nested declaration's module is its enclosing declaration's

`Hir::module_of` answers for a nested declaration by walking out to the
top-level declaration a module lists.

### 3. Two more binding forms, resolved as the name check reads them

- **A stream's `<ready as={x}>`** is its query's success, and
  **`<failed as={e}>`** its failure. Each is typed by the query's result
  where it is a `Result`, and labelled by the query's label.
- **`release(h) { .. }`** binds `h` in its block to what the resource's
  `acquire { .. }` before it produces: its success where it can fail.

### 4. A call's callee is the binding in scope, or a declaration

The call check reads the resolver: a callee's name is local where a binding
of it is in scope, and must otherwise name a declaration.

### 5. One parenthesised parameter is a parameter

`(x) => ..` binds `x`, as `x => ..` and `(a, b) => ..` do.

## Acceptance

- **`compiler/pw-core/tests/nested_scope.rs`**, 9 tests:
  - a nested function's secret (PW5006);
  - its types (PW0609), through a binding and a parameter around it;
  - a declared field label through a nested function (PW5006);
  - the binding in scope where a nested function is written;
  - a nested declaration's own annotations (PW0609);
  - a stream's part (PW0610);
  - a release clause (PW0605);
  - `(x) =>`;
  - a call out of scope (PW0021).

  Each wrong program is refused, with a control that the right one is not.
  Eight fail at 85899fc, the commit before. The ninth guards the position
  rule against a false positive.
- **No rejected, rule or generality fixture's diagnostics change,** apart from
  R-002's correction. The store, kiokun and the accepted corpus check clean.
- **Two more of the corpus's value relations are decided:** A-008's
  `item.name` through its stream, and A-019's `Maps.destroy(handle)` through
  its release clause. Every relation decided before is decided the same.
- **The store's and kiokun's artifacts are byte-identical,** apart from
  ADR-0058's two handlers.
- **Mutation controls:** `scripts/nested_scope_mutations.py`,
  `just e10-nested`, 12 mutants.

## Not done

- **The name check keeps its own walk.** `names.rs` resolves scopes for
  PW0021 itself. The difference that remains is its keyword-modifier rule:
  any keyword statement's first word is bound, `query Store(..)`'s `Store`
  too. It keeps a modifier quiet, and the resolver does not bind it. One rule
  for both is next.

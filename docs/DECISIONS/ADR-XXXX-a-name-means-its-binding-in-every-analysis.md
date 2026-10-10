# ADR-XXXX: a name means its binding, in every analysis

Status: accepted under the owner's delegation of 2026-10-02. Date:
2026-10-10. Milestone: E14. The first part of NEXT's "the language's, found
on the way": a template's bindings shadow module members (W8's finding,
2026-10-09) and a call through a local is charged its namesake's row
(ADR-0283's limitation). ADR-0063 ruled that every name means one binding;
the analyses after it did not all ask which.

## Context

- **The compiler has one answer to "which binding"**: `Lexical`
  (`lexical.rs`), which resolves each local name in a body by its scopes:
  a declaration's parameters, `let`s and patterns, lambda parameters,
  `{#each … as x}`, a `{#match}` arm's names, `<ready as={x}>` and
  `<failed as={e}>`, policy terms, and a nested declaration's outer names.
  The HIR keeps names as spellings (`Expr::Name(String)`), so every analysis
  that wants a name's meaning must ask `Lexical`. Several did not.
- **W8's finding.** A page's `<stream>` named its answer `estimate`, and the
  store's addresses module declared `fn estimate`. The page was refused,
  PW0401, for an effect `fn estimate` performs: the name where `as={…}`
  binds it was in no scope (`Lexical` skipped it), so the effect inference
  took it for the module's function named as a value (ADR-0078) and
  charged its row. W8 renamed the function.
- **ADR-0283's limitation.** The effect inference's call walk resolved a
  callee by its spelling: `found()`, where a `let` bound `found`, was
  charged the module's `fn found`'s row. ADR-0283 stated it, as an
  over-statement, refusing work rather than granting authority.
- **The same walk, elsewhere.** The privacy analysis's reads (a page's
  label, a materialization's reads, the cause a label names) resolved a call
  by its spelling too; and the rule that a handler runs in the browser
  counted a call as a command's request by its spelling, where the named
  handler beside it already asked `Lexical`.

## Decision

1. **`<ready as={x}>` and `<failed as={e}>` bind where they are written**:
   `Lexical` records the name's own occurrence as the part's binding
   (`Binder::Stream`), as it records each use inside the part.
2. **A call through a local is that local's.** Where a call's callee is a
   name a binding in scope holds (a `let`, a parameter, a pattern), the
   effect inference charges no declaration of its spelling: what the value
   names was charged where it was named (ADR-0078), a lambda's body where
   it is written. The inference keeps, from its `run`, each body's `Lexical`
   by the body's unit and owner, and asks it.
3. **So do privacy's reads and the handler's requests**: a call through a
   local reads no query and requests no command of its spelling.
4. **A member call on a local value**, `x.m()`, resolves as before: by its
   member, through the value's type (ADR-0078) and the declarations its unit
   sees.

## Found

1. **`Lexical` left `<ready as={x}>` unbound where it binds**, so any
   analysis that asked took the name for a module's member. Three walks had
   worked around it by hand (`names.rs`, `values.rs`, `template_ir.rs`).
2. **The privacy analysis resolved a local call by its spelling**, as the
   effect inference did: a local named as a query was read as that query.

## Alternatives

- **Resolved names in the HIR** (`Expr::Local(Binder)` beside
  `Expr::Name`): every analysis would read the binding without asking. The
  right end state, and a change to every walk of the HIR; `Lexical` already
  is the answer, and asking it is the change that closes these.
- **Skipping every name some binding in the body holds** (`resolve.rs`'s
  flat set): a call outside that binding's scope, to the module's function,
  would be skipped too, and its effects not charged: an under-statement,
  which grants authority. Scopes are `Lexical`'s.

## Acceptance

`just e14-name-bindings` (docs/evidence/E14/name-bindings.txt):

- `compiler/pw-core/tests/names_mean_their_binding.rs`:
  - a stream part named `estimate` beside an imported `fn estimate` that
    reads the clock, on a page built once: reported exactly as the part
    named `waited` is, with no clock read;
  - a local `found` beside `fn found`, called on a page built once: not
    charged `clock.wall`; the module's own `found`, called, is;
  - a shared query that calls a local `mine` beside a `fn mine` that reads
    the session's cart: not the session's; the module's `mine`, called, is
    (PW5001);
  - a handler's local `add_to_cart` beside the command: its argument's read
    of the cart is the browser's, and refused (PW5005), as the same read
    outside any call is;
  - a shared query whose local `current_session` is called: not the
    session's, where the platform's, called, is;
  - a shared query that reads the cart after calling a local `mine`: the
    cause its refusal names is not `mine`;
- pw-core's tests and the corpus, unchanged;
- `scripts/name_binding_mutations.py`: 7 mutants.

## Not claimed

- **The page plan** still reads a root by its spelling among the page's
  bindings, signals and parameters before its loops (ADR-0277's
  limitation): the second part, a name in a template read by its binding.
- **A member call on a local** is resolved by its member's name among the
  declarations its unit sees, as before.

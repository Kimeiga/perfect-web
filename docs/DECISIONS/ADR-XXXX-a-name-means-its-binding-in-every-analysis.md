# ADR-XXXX: a name means its binding, in every analysis

Status: accepted under the owner's delegation of 2026-10-02. Date:
2026-10-10. Milestone: E14. The first item of NEXT's "the language's, found
on the way": a template's bindings shadow module members (W8's finding,
2026-10-09), a call through a local is charged its namesake's row
(ADR-0283's limitation), and a loop's name is read as a page's binding of
its spelling (ADR-0277's limitation). ADR-0063 ruled that every name means
one binding; the analyses after it did not all ask which.

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
- **The page plan reads the template by name.** The template IR names what
  a part reads by a path (ADR-0073), its root a name: a page's parameter,
  query binding or signal, or a block's binding. The plan, the speculation
  backend and the walks under them match a root against the page's names
  first, by spelling. Lowering renamed a block's binding only in a composed
  view, and only where it would hide a path the view was given (ADR-0136).

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
5. **Each name the template reads is one binding's.** Lowering carries the
   names the template can read at each node: the page's parameters, query
   bindings and signals, and each enclosing block's binding as the template
   writes it. A block (`{#each}`, a `{#match}` arm, `<ready as>`, `<failed
   as>`) that binds one of those names is given one no source writes
   (`line~1`, ADR-0136's rename), as is every one a layout binds: a
   layout's markup is the same on every page that names it, whatever the
   page binds, and a soft navigation compares it by its schema. Inside the
   block its binding hides a signal of its spelling too, and a composed
   view is lowered with the names its use can read.

## Found

1. **`Lexical` left `<ready as={x}>` unbound where it binds**, so any
   analysis that asked took the name for a module's member. Three walks had
   worked around it by hand (`names.rs`, `values.rs`, `template_ir.rs`).
2. **The privacy analysis resolved a local call by its spelling**, as the
   effect inference did: a local named as a query was read as that query.
3. **A block's name beside a page's binding of its spelling was refused**
   (ADR-0277's limitation, found on the store's page 2026-10-08): `{#each
   cart.lines as line}` beside `let line = query MenuLine(id)` failed with
   "`.name` is read from a value with no declared type". So did a stream
   part's `as={line}`, a loop named like the page's signal, a composed
   view's loop and a layout's loop, each beside a page binding of its
   spelling. The store's binding, renamed `summary` to get by, is `line`
   again. Not a finding: an inner loop that binds the outer one's name was
   planned right (each walk takes the nearest loop); its name is now its
   own as well.

## Alternatives

- **Resolved names in the HIR** (`Expr::Local(Binder)` beside
  `Expr::Name`): every analysis would read the binding without asking. The
  right end state, and a change to every walk of the HIR; `Lexical` already
  is the answer, and asking it is the change that closes these.
- **Skipping every name some binding in the body holds** (`resolve.rs`'s
  flat set): a call outside that binding's scope, to the module's function,
  would be skipped too, and its effects not charged: an under-statement,
  which grants authority. Scopes are `Lexical`'s.
- **Every block's binding renamed** (all binders unique, as a compiler's
  renaming pass makes them): no bookkeeping, and every template's paths,
  plans and recorded evidence changed (`item.name` read as `item~3.name`)
  for no page that is wrong. Renaming where a name is already readable
  changes the pages that shadow alone; a layout's every binding is renamed
  because the names a page binds are unknown to it.
- **The page plan asking `Lexical` for each root**: the plan reads the
  template, not the source, and the speculation backend and the renderer
  read the same paths; a name unique in the template answers all three.

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
  - a shared query that calls a local `mine` beside a `private fn mine`:
    not its reader's; the module's `mine`, called, is (PW5001);
  - a shared query that reads the cart after calling a local `mine`: the
    cause its refusal names is not `mine`;
- pw-core's tests and the corpus, unchanged;
- `names_mean_their_binding.rs`, the page plan:
  - the store's page with `line` beside the cart's loop: built, its part
    reads the menu's line, and the loop binds a name no part outside it
    reads (the control: `summary`);
  - an inner loop that binds the outer one's name: its own, and its row's
    member read is the item's;
  - `<ready as={line}>` beside the page's `line`;
  - a loop named like the page's signal: not the browser's, and no
    element tells a handler in it that the loop's line is the signal;
  - a composed view's loop beside a page binding of its spelling;
  - a layout's loop, and a view's in it, the same on a page that binds
    `line` and on one that does not;
- the store's pages in three engines, the store binding `line` again;
- `scripts/name_binding_mutations.py`: 14 mutants.

## Not claimed

- **The walks over the template still match a root by its spelling**:
  they meet one binding by each because lowering renames, not because they
  ask `Lexical`. A walk over the source (a computed part's input's type)
  asks the checker's types, which are by binding.
- **A member call on a local** is resolved by its member's name among the
  declarations its unit sees, as before.

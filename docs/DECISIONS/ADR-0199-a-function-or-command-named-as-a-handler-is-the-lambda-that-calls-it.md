# ADR-0199: a function or a command named as a handler is the lambda that calls it

Status: accepted under the owner's delegation of 2026-10-02. It builds the
remainder of ADR-0195's ruling 12. Date: 2026-10-05. Milestone: E14.

## Context

- **Every lambda in an `on:` attribute is a resumable handler** (ADR-0134).
  A named function, `on:submit={save}`, was checked against its event's type
  (PW0602, ADR-0131), and the build refused it: "element 0 handles an event
  with no code to run".
- **The owner ruled (ADR-0195, ruling 12):**
  - `on:submit={save}` means `e => save(e)`;
  - it is checked against the event's type, compiled as a lambda is, and
    held to ADR-0157;
  - until it is built, it is refused when the program is checked, not only
    when it is built.
- **Building it found three defects in what already checked:**
  - **The event check read only a bare name, in the page's own module.**
    `on:press={other.rename}`, a command taking an `Int`, checked, and the
    build would have given it the `PressEvent`.
  - **A local's value as a handler checked.** `let save = () => 1`, then
    `on:press={save}`, was refused only by the build. A local that shadowed
    a declaration was checked as the declaration.
  - **A page, a type, a case or a query as a handler checked.**
    `on:press={P}` reported nothing, where `() => P()` is PW0027.
- **Found beside it, and its own ADR (ADR-0200):** `()` written as a value is
  an expression that did not parse, and nothing reports it.
  `fn f() -> Int !{} { () }` checks clean.

## Decision

1. **A function or a command named as a handler is the lambda that calls
   it.** `on:submit={save}` is `(e) => save(e)`, and `() => save()` where
   `save` takes nothing.
   - It is named by its path, `save` or `forms.save`, where no local binds
     the first segment.
   - One function says what an `on:` value names, `Types::named_handler`.
     The checker, the backend and the handler's identity all ask it.
2. **It is held to every rule its lambda form is:**
   - **Its event (PW0602).** It takes nothing, or the event. One that takes
     more is refused: "found `both`, which takes 2 parameters". That holds
     through a module, `forms.save`, too.
   - **Its answer (PW0618).** A command's `Result`, given to the runtime,
     is refused, as `() => add()` is.
   - **Idempotency (PW0338).** A command it names is a command the page
     sends.
   - **The browser (PW5005).** A function it names runs in the browser, and
     performs what a call of it would: its row, or, where it declares none,
     what its body does. A command it names is a request, and the command
     performs it where it runs.
   - **Doing something.** One that calls no command and changes no signal is
     refused when built, as its lambda form is.
3. **It is compiled as its lambda form is.**
   - One handler module, which calls the declaration, with the event where
     it takes one.
   - Its identity hashes its name and the declaration that name resolves
     to, as a lambda's call is hashed. The same text naming another
     module's command is another identity.
   - Its name is its lambda form's: `save`, and none through a module, as
     `(e) => forms.save(e)` has none. The runtime loads a handler by its
     identity (ADR-0132); the name only describes it.
4. **Anything else named in an `on:` attribute is refused when checked**,
   once (PW0614, `event_attribute_names_no_handler`):
   - a local's value that is a function, "a value this declaration binds",
     even where it shadows a declaration of the same name;
   - a page, a view, a component, a query, a subscription, a resource, a
     task, a materialization, an event, a type or a case.

   A value that is no function stays PW0614's "is given `Int`", and a name
   that resolves to nothing stays PW0021.

## Alternatives

- **A local function value as a handler**, React's `onClick={handle}`.
  - Its identity and captures would come from where it is bound, not
    where it is used.
  - A signal it writes would have to count as a handler's write (PW5300)
    where it is bound.
  - A lambda written in the attribute shows both where they are, and a
    declaration is how a behaviour is shared.
- **Keep refusing the named form when built.** The owner ruled it is a
  handler.
- **Name `forms.save` by its path.** The named form would be named where
  its lambda form is not. The two forms stay one, and naming a call through
  a module is a change to both.

## Acceptance

- **`compiler/pw-core/tests/named_handlers.rs`**, 10 tests:
  - a command named as a handler compiles to its lambda form's module, but
    for its identity, and sends it;
  - it is held to PW0618, PW0338 and PW0602;
  - a function that changes nothing is refused, as its lambda form is;
  - it builds with an identity and a module;
  - its identity follows what its name resolves to;
  - it is given its event: its module is `(e) => rename(e)`'s;
  - a local, a page, a type, a case and a query are each refused when
    checked, once;
  - a local that shadows a command is refused as a local;
  - the event is checked through a module's path;
  - its name is its lambda form's.
- **`handlers_in_the_browser.rs`'s
  `a_function_named_as_a_handler_performs_only_what_the_browser_may`.** A
  function's read, with a row or without one, is refused. The controls: its
  lambda form, a command, a function that performs nothing, and a local
  that shadows the function.
- **`every_handler_is_resumable.rs`'s
  `a_handler_named_by_a_local_is_refused_when_checked`.**
- **No program changes.** The store's build is byte-identical, 66 files. The
  corpus checks as before.
- **`scripts/named_handlers_mutations.py`**: 17 mutants (`just
  e14-named-handlers`).

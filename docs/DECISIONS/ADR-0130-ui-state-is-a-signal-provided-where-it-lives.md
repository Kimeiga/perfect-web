# ADR-0130: UI state is a signal, provided where it lives (ADR-0126's rulings)

Status: accepted under the owner's delegation of 2026-10-02 ("I will leave
the decisions to you for now as long as you log them"). Date: 2026-10-02.
Milestone: E14 (T06, T11), toward the DoorDash target. Accepts ADR-0126 with
the four rulings it asked for, and two changes that the prior-art research
behind them argued for. Rules ADR-0072's open question. Nothing here is built
yet: each step under "Order of work" is its own ADR, with tests.

## The four rulings

1. **The keyword is `signal`.**
   - `docs/SEMANTICS.md` already names the primitive `signal`.
   - Solid, Angular, Preact and the TC39 proposal use the word for the same
     thing: a value whose readers update when it changes.
   - `state` would collide with the charter's resource state (§9.1), which
     is a different kind of state with different rules.
2. **Views compose at compile time and are instantiated at run time**
   (ADR-0072's open question), which is Marko 6's model.
   - A view is compiled once, and its markup is spliced into each user's
     template.
   - Each part is addressed by its instance path and its number within the
     view, so two uses of one view never share an address.
   - Props, effect rows, labels, placement and signal requirements are
     resolved per use site, at compile time.
   - A view may contain itself only inside `{#if}`, `{#match}` or `{#each}`.
     There it is an instance made at run time.
   - The alternative ADR-0072 described, rendering a child in place at run
     time, keeps all three defects it found: repeated addresses, lost loop
     identity, and unattached handlers.
3. **A signal is ephemeral, or persisted in the URL; nothing else.**
   - `persist url("tab")` round-trips a value through the address bar. That
     is state worth a shareable link: a tab, a filter, a search.
   - Session storage is not offered. It survives a reload without being
     shareable, and it is one more place a label would have to follow.
   - Revisit when a task needs state that survives a reload and must not be
     shared, such as a draft.
4. **A handler cannot assign a body's `let mut`: state a handler changes is
   a `signal`.** So there is one way to hold UI state.
   - A `let mut` a body uses for its own computation is a local variable
     and stays legal: A-021's paint loop counts with one.
   - What ADR-0126 called "E3's `let mut` view state" is the one use that
     goes, and the error's repair names `signal`.

## Two changes to ADR-0126

- **A shared signal is a declaration, read by its own name** (from the
  research behind this ruling: Koka's `effect val`, Scala 3's givens).
  `signal drawer: Drawer`, at module level, declares the signal. A page or
  view gives it a value for everything it contains: `provide drawer =
  Drawer.Shut`.
  - A view reads and writes `drawer` by the name its module imports, as it
    names anything else. A typo fails where it is written.
  - An unrelated signal that happens to share a name is never picked up.
  - The same view works under any provider.
  - ADR-0126's R2 resolved a bare name against whatever declared it around
    the use, which fails the first two.
- **Shadowing is explicit, and is allowed.** The innermost `provide` wins:
  a dialog inside a dialog provides its own. Two `provide`s of one signal in
  one scope are an error, as two givens of equal rank are in Scala 3.
  ADR-0126 refused every shadowing, which would forbid the nested dialog.

A signal used only by the view that declares it is written in that view's
body: `signal open: Bool = false`.

## Rules carried from ADR-0126

- **R3. Only handlers write.** A render's row is empty (charter §7.4).
- **R4. Updates are fine-grained:** only the parts that read a signal change.
- **R5. Derived values:** a value computed from signals is `derived`
  (ADR-0082), never a second signal kept in step by hand.
- **R6. A signal ends with its scope's instance.** A handler that writes it
  afterwards writes nothing.
- **R7. Types:** a signal's type is any serializable Pleris type, and a
  `match` on it is exhaustive.
- **R8. Signals live in the browser.** Server code cannot read one.
  - A value assigned to a signal carries its label (ADR-0129), so a secret
    in a signal is a secret serialized to the browser, and is refused.
  - A signal initialized from a reader's value makes its page private, by
    the rules that exist.
- **R9. Resumption:** each signal's initial value is in the document, and
  the browser resumes without re-running the page.

## Resolution happens when the instance is made

Each view instance captures its providers' references (a scope id and a
slot), and a handler uses the captured reference. So a handler running
after a navigation never reads another instance's signal, which is the
trap Koka's documentation describes for a function created under one
binding and called under another. This is evidence passing, as Koka
compiles handlers (Xie and Leijen, ICFP 2020), and needs no inlining.

## Order of work

Each is its own ADR, with acceptance tests that fail before it:

1. **Local signals on a page.** `signal x: T = e` in a page body, read in
   text, attribute, `{#if}` and `{#match}` parts, and written by a handler
   that may now call no command. Includes the browser's render of a block,
   the resumption of initial values, and compile errors for a write outside
   a handler, a signal on the server, and a type with no serial form.
2. **View composition** (ruling 2), with handlers and loops inside views.
3. **Provided signals** across sibling views: requirement inference, the
   missing-provider error, and explicit shadowing.
4. **Typed events and `bind:value`** (ADR-0058's ruling, separately
   recorded), which T06 needs.
5. **`persist url`.**

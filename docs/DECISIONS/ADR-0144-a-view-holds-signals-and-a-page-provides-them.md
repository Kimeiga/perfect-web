# ADR-0144: a view holds its own signals, and a page provides signals to the views it composes

Status: accepted under the owner's delegation of 2026-10-02. Date: 2026-10-02.
Milestone: E14, toward the DoorDash target. ADR-0130's third step, built on
view composition (ADR-0136).

## Context

ADR-0130 ruled how UI state is shared:
- A signal one view uses alone is written in that view's body.
- A shared signal is declared at module level, `signal drawer: Drawer`.
- A page or view provides it to everything it contains, with `provide
  drawer = ..`.
- The innermost `provide` wins.
- Two `provide`s of one signal in one scope are an error.
- Each view instance captures its providers' references.

None of it was built. A signal lived in a page's body only, and a view that
held anything but markup was refused at composition (PW5020, ADR-0136). So
state two views shared had to be written in the page. A view whose button
changed it could not be written: the handler had to be the page's, in the
page's markup.

A header's cart button and the drawer it opens could not both be views. In
React this is the "context" problem. State is passed down by hand through
every component between, or put in a global any component can reach by name.

## Decision

1. **A view's own signal.** `signal open: Bool = false` in a view's body,
   beside its markup. Each use of the view holds its own. Views are composed
   at each use (ADR-0136), so each use's signal is an instance of its own.
   It is named so no source can write it: `open~2`.
   - The page's plan holds each instance and its first value.
   - The server renders each instance at its first value.
   - The browser holds each one by its name.
2. **A provided signal is declared at module level, with no value:**
   `signal drawer: Bool`. It is a name and a type. A page's or view's body
   gives it a value, `provide drawer = false`, and that value holds for:
   - the body's own markup and handlers;
   - every view composed in it, through the views those compose, unless a
     nearer body provides it again. That nearer `provide` shadows it.

   A body reads and writes a provided signal by the name its module
   imports, so:
   - a typo fails where it is written;
   - an unrelated signal that happens to share the name is never picked up;
   - one view works under any provider.

   A `provide` is the assignment of the signal, marked as a `provide`. Its
   name resolves as any name does, and its value is checked against the
   signal's type as any assignment is (PW0607). A value written on the
   module's declaration is refused (PW5306): a `provide` gives it.
3. **What is not provided is refused** (PW5305, new). A view needs:
   - each provided signal it names;
   - each one a view it composes needs, unless the view provides it itself.

   A page that needs a signal nothing provides is refused at the use that
   needs it, and the message names the views the need comes through:
   "nothing provides `ui.drawer` to `P`, and `<Wrap>` needs it, through
   `<Open>`".

   A provided signal has no default value. A default would hide the missing
   provider, as React's `createContext(default)` does. A view's own
   template, outside any page, reads a signal nothing provides by its name.
4. **One `provide` of a signal per body, and only of a signal** (PW5306,
   new). Each of these is refused:
   - two `provide`s of one signal in one body;
   - a `provide` of a name that is not a module's signal;
   - a `provide` in a body that is not a page or view.
5. **A handler is compiled once, for every instance.** A handler's module
   names each signal as its body writes it. The element it is attached to
   says which instance each is, in `data-pw-signals`. The compiler writes
   that attribute; the program cannot. The browser's `get` and `set` read
   through it.

   This is ADR-0130's captured reference. Which instance is fixed when the
   element is rendered, so a handler never reaches another instance's
   signal. Two counters share one handler identity and count apart.
6. **A signal ends with its instance** (ADR-0130, R6). An instance inside a
   block is owned by that block, and by every block around it. The plan
   records this as the block's `owns`.

   When a block a signal decides shows another arm, the browser resets what
   the block owns to its first values, and only then renders the block. A
   counter in a drawer that is closed and opened again starts again.

   An `{:else if}` arm is a block of its own here, so an instance in it ends
   when that arm does.
7. **Not inside a loop's row yet** (PW5307, new). A view that holds a
   signal, or provides one, or composes a view that does, is refused inside
   `{#each}`. Each row would need its own instance, kept by the row's key as
   rows are added, removed and reordered.

   A view in a row that reads or sets a signal the page provides holds
   nothing of its own, and is accepted. So a menu's rows can each open the
   page's drawer.
8. **A module signal is read and changed where a page's is.** It is changed
   only in a handler, and given its value by a `provide` (PW5300). It is
   read only in a template part or a handler (PW5301). The rules of
   ADR-0141 and ADR-0142 apply to it as to a page's own signal:
   - a dialog it shows hears its closing;
   - `bind:value` binds one of `String`.
9. **The Marko adapter refuses what it does not model** (ADR-0017's second
   rule). A declaration that provides a signal, or names one a page
   provides, is skipped with its reason. Written first, the adapter dropped
   the `provide`, and wrote a handler setting a name nothing held. A view's
   own signal is Marko's own state, `<let/count=0>`.

## Alternatives

- **Props, by hand.** Rejected as the only way, for ADR-0130's reason.
  Every view between the owner and the user carries a value it does not
  read, and a view that changes it cannot be written.
- **A default value on the declaration**, as React's context and Svelte's
  `getContext` fall back to. Rejected: the missing provider is the defect,
  and a default turns it into a quiet wrong value.
- **Resolve a provided signal by name at run time.** Rejected: a typo, or an
  unrelated signal sharing the name, would be found or missed silently.
  ADR-0126's R2 failed this way.
- **A handler module per instance.** Rejected. It would be the same code
  compiled again for each use, with identities that differ only in a name,
  and the resume manifest would grow with the uses. The element saying which
  instance is one attribute.

## Acceptance

- `compiler/pw-core/tests/provide.rs`: twelve tests, each with controls:
  - the declaration;
  - instances per use, with one handler;
  - siblings sharing a provided signal;
  - shadowing;
  - PW5305, PW5306 and PW5307;
  - reads and writes;
  - the `provide`'s type;
  - the blocks that own an instance;
  - binding and dialogs;
  - the Marko adapter's refusals.
- `spikes/own-renderer/e2e/provide.spec.mjs` over `examples/demo/provide.pw`
  and `examples/demo/drawer.pw`, in Chromium, Firefox and WebKit:
  - the server's first values, with no script;
  - two counters count apart, through one compiled handler;
  - a button in one view opens a drawer another view shows;
  - a panel that provides its own drawer keeps it apart;
  - a counter in a closed drawer starts again.
- `scripts/provide_mutations.py`: compiler mutants against the tests, and
  browser mutants against the spec in Chromium.

## Not claimed

- **Per-row instances** (7).
- **A first value computed from props**: a first value is a literal, as a
  page signal's is.
- **A block rendered again on the arm it showed.** The browser compares
  arms so that instances survive this, and no page here does it yet: a
  block is rendered again only for reads it cannot set in place
  (ADR-0142).
- **`persist url`** (ADR-0130, step 5).
- **The Marko adapter** models no `provide` (9).

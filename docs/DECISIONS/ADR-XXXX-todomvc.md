# ADR-XXXX: TodoMVC in Pleris, held to its own spec and its own tests

Status: accepted under the owner's delegation of 2026-10-02 (a worker's, the
integrator's to number and index). Date: 2026-10-14. Milestone: E14, track
`todomvc` (W7), its first milestone. Builds on the integrator's rulings in
docs/PARALLEL.md, "2026-10-09, W7's plan: TodoMVC", and of 2026-10-14 on the
fragment (below).

## Context

- **The owner's reference app, familiar to everyone** ("something people are
  familiar with", relayed 2026-10-08): TodoMVC, with an objective suite, its
  spec (`app-spec.md`) and its official behavioural tests (one Cypress spec),
  and, for the comparison and the tutorial, a first lesson.
- **What it asks of Pleris that no app has asked**: the filters' routes,
  `#/`, `#/active` and `#/completed`, which reach no server; editing a todo on
  a double press, with the field focused, saved on Enter and on blur and
  discarded on Escape; a counter that says "1 item left" and "2 items left".
- **The plan's ruling on the first of these** (docs/PARALLEL.md, 2026-10-09):
  "a page binds a signal to its fragment, whose type names the forms it takes;
  the runtime keeps it to `location.hash` at start and on each `hashchange`;
  the server renders its declared default", and the integrator's of
  2026-10-14: build it as its own commit, with its own checker and runtime
  tests and mutation controls, minimal where the language needs more, and
  write down each choice the ruling did not specify.

## Decision

### 1. A page binds a signal to its address's fragment

```
session page Todos() {
    route    "/"
    fragment filter "/" => "all", "/active" => "active", "/completed" => "completed"

    signal filter: String = "all"
    ..
```

1. **A page clause**, `fragment <signal> "<form>" => "<value>", ..`: a signal
   of the page, and each form the fragment takes, as `location.hash` reports
   it without its `#`, with the value the signal takes then. It is a policy
   head (`pw_syntax::POLICY_KEYWORDS`), a page's alone (`policy::declared_by`), read
   from the clause's text as a `route` is (`address_fragment::read`), and
   carried to the browser in the page's plan and its document's manifest
   (`fragment: { signal, forms: [{ form, value }] }`).
2. **The runtime keeps the signal in step with the address**
   (`pw-runtime.mjs`, `bindFragment`): it reads `location.hash` once when the
   page starts, after the document's subscription, and again on each
   `hashchange`, the one a link makes, the back and forward buttons, and a
   script's `location.hash = ..`. The signal is set as any signal's change is:
   what reads it is set or rendered again, and a query it keys is read again
   for the new key (ADR-0152), which is how TodoMVC's model filters.
3. **The server renders the declared default**: a request carries no fragment,
   so the document is the same bytes whatever the address's fragment was. A
   page opened at `#/completed` shows its first value until the runtime has
   read the address and the page's keyed read has answered (the deep-link
   test), which is a flash of the default, and the price of a fragment that
   no server sees.
4. **A fragment that is no form, or none, is the signal's first value**, the
   declared default; the first value is held to be one a form gives (PW6104),
   so the default is a page the address can reach. A signal already at the
   value an address gives is not set again, so a page opened at its default
   renders nothing more.
5. **The address is the signal's only writer** (PW6105): no handler assigns
   the signal, so the signal and `location.hash` cannot differ. A page changes
   it by a link, `<a href="#/active">`, which the browser follows, in place,
   and whose history entry is the back button's.
6. **A link to a fragment the page does not take is refused** (PW6106): on a
   page that binds its fragment, a static `href="#..."` in its own view names
   a form it lists, so a reader who follows one is never set back to the
   default by a typo.

**Checks** (`compiler/pw-core/src/address_fragment.rs`, owner `Todomvc`, the
block PW61): PW6101 the clause names a signal the page declares; PW6102 the
signal is a `String`; PW6103 the forms are `"form" => "value"` pairs, each form
once, of the characters a fragment holds; PW6104 the signal starts at a value a
form gives; PW6105 no handler assigns it; PW6106 a link names a form. A clause
written twice, and one outside a page, are the generic rules'.

### The choices the ruling did not specify

Each is the integrator's to confirm or reverse; none is built past what the
ruling says, and each is cheap to change now.

- **The forms are named by the clause, and the signal is a `String`, not a sum
  type.** The ruling reads "the signal's type names the forms". Two limits of
  the language today stopped that: a query's key must be a `String`, an `Int`
  or a `Bool` (PW5308), and TodoMVC's filter keys its read; and a signal of any
  other type needs a codec, which is not built (ADR-0131). So a form's value is
  the text the signal holds, and the clause is where the closed set is written.
  When keys and signals may be a sum type, the clause can name its cases
  instead of texts, and nothing else changes.
- **A clause on the page, not a modifier of the signal, and not the route's.**
  The signal's declaration stays the one every signal has. RFC 6570 has a
  fragment expansion, `{#var}`, which would have put it in the route; a route's
  parameter is read once per request, by a server that never sees a fragment,
  and a signal is what the browser changes, so the route is the wrong place.
- **One-way: the address sets the signal, and nothing sets the address.** "At
  start and on each `hashchange`" is what the ruling asks. Writing the address
  back from a handler (`history.pushState`) would make two writers; PW6105
  refuses it instead, and a link is how a page moves.
- **A form is compared exactly**: the text after `#`, as `location.hash`
  reports it, case-sensitive, not decoded. A form is written in the characters a
  fragment holds as they are (RFC 3986 §3.5's `pchar`, `/` and `?`, less `%`),
  so a form is what the browser reports and never a percent-encoding of it.
- **Several forms may give one value, and the empty fragment is a form a page
  may list**: aliases (`"!/" => "all"`, as TodoMVC's spec allows `#!/`) and
  the address with no fragment (`"" => "all"`). A form written twice is
  refused. A value is any text; a first value is a literal.
- **One binding a page**, and the page's own view only: PW6106 reads the links
  of the page's template, not of the views it composes.
- **The compiler's change is one policy head and one module.** No syntax form
  was added: a policy's value is the text of its line, and the clause's
  grammar is read from it, as `route`'s and `says`'s are.

### Alternatives

- **A signal modifier** (`signal filter: String = "all" in fragment ..`): a new
  form of the statement, whose forms would have no place to be written.
- **A sum-typed signal with its cases named by the type**: not today, as above.
- **Functions** (`fn filter_of(fragment: String) -> Option<Filter>`) written in
  Pleris and called by the runtime: a codec, ADR-0131's question, for one page.
- **A route's `{#filter}`**: above.
- **Writing the address back** from handlers: two writers, above.

### 2. A double press and a control's losing focus are events the platform declares

TodoMVC begins an edit on a double press of a todo's label, and saves it on the
field's blur. `events.pw` declared `press`, `input`, `keydown`, `change`,
`submit` and `close`; ADR-0138 listed `focus` and `blur` among the "events the
platform does not declare", and a double press was not thought of. "Declaring a
new event is a change to this file rather than to a checker" (`events.pw`), so:

- **`dblclick`**, whose handler is given a `PressEvent`, where the pointer was:
  a double press is read as a press is. The two presses of the pair are
  presses still, and run the handlers an element has for them.
- **`blur`**, whose handler is given a `BlurEvent { value }`, the value the
  control held as it lost focus: the field's edit is saved from it, as an
  `input`'s record is read. It does not bubble, so it is the element's own.
- **The runtime** maps each to the browser's event of the name and reads its
  record in the listener, before any code loads (ADR-0138, `DOM_EVENTS`,
  `eventRecord`). The platform contract's hash moves
  (`tests/platform_contracts.rs`), for `events.pw` alone.

The names are the browser's, `dblclick` and `blur`, not `doublepress`: a double
press has no other form (no key, no touch), and a reader of the program meets
the name they know. A choice for the integrator to confirm or reverse.

Not built, and not needed: `focus` (the pair of `blur`), `focusin` and
`focusout`, a refinement of a checkbox's `change` to `checked` (ADR-0131), and a
keyboard's way to begin an edit (TodoMVC's own is the double press, which the
spec names, and a keyboard user cannot make one: a gap the spec has).

### 3. TodoMVC, built

A fourth program on the development server, beside the store, the feed and
kiokun: `examples/todomvc/app.pw`, built by `spikes/own-renderer/todomvc.sh`
into `dist-todomvc`, served on a host per engine (`TODOMVC_PORTS`, `PORT+140`
to `PORT+142`, the plan's ports).

- **Its data layer is a seam of its own** (`TRACK SEAM (todomvc)` in
  `from_build_layers`): chosen where the program imports `todomvc:data/…`, as
  kiokun's is. `todomvc.rs` keeps a session's todos in memory, oldest first,
  under one lock from a command's call to its commit, as the feed's layer
  does. Every operation names its session, and one that names another than
  the command's or the query's own is refused, so no write or read reaches
  another session's list. A session holds at most 500 todos (`full`), which
  TodoMVC's spec does not state and a host guests write to needs.
- **Its markup and class names are the template's**, and its stylesheets are
  TodoMVC's own: `todomvc-common`'s `base.css` and `todomvc-app-css`'s
  `index.css`, from the npm packages the owner approved on 2026-10-08, copied
  as they are by `todomvc.sh`. The official sheet draws a todo's checkbox, a
  circle and a ticked circle, as `data:` images, which a page's generated
  policy refuses (`img-src 'self'`, ADR-0333). The spec allows an app's own
  sheet over the official ("if you need to change some styles, use the app.css
  file"), so `todomvc-styles.mjs` reads the two drawings out of the official
  sheet as it ships, writes each as an SVG file of this origin, and writes
  `todomvc-pleris.css`, the two rules that point at them, loaded after.
  Nothing is drawn by hand; the host serves `.svg` as `image/svg+xml` (a
  one-line change at its static route, marked as the track's), which a browser
  asks for under `nosniff`.
- **The licences.** `todomvc-common` is MIT. `todomvc-app-css` is **CC BY 4.0**,
  at its source and on npm, not MIT as the plan says: it asks for credit, which
  the page gives in its footer and the licence file served beside the sheet
  (`/todomvc-app-css.license.txt`) carries, and nothing in it is changed. The
  owner approved the package; whether CC BY 4.0 is what they meant is the
  integrator's to confirm (a question below).
- **The filters are the model's.** The page's read, `Board(session, filter,
  editing)`, is keyed by the signals `filter`, which the fragment sets (§1), and
  `editing`; the program's own pure functions filter (`shown`) and count
  (`board_of`), and a change of the address is a read for a new key (ADR-0152).
  The footer, the counter, the toggle-all and the clear button read counts of
  every todo, which no filter changes.
- **The editing mode is the page's signal, and the model says which row shows
  its field.** TodoMVC's spec: "Editing mode should not be persisted", so the
  mode is UI state (charter §9.1's first kind): the signal `editing` names the
  todo, `edit_text` holds the field's text, and a double press sets both. A
  signal read inside a query's loop, `{#if editing == row.id}` in a row, is not
  built (ADR-0137 refuses it; ADR-0144's per-row instances, PW5307, are not
  built either), so `editing` keys the page's read too and the model returns
  each row with `editing: true` for the one: the row is shown again, with its
  field. The cost is a read for each begin and end of an edit, and a row
  re-rendered whole, which a change to its text by another tab of the session
  would take the field from. A question for the integrator, below: this is
  what the missing piece costs, and the proper fix is a signal a row can read.
- **The new todo is cleared at Enter, and given back if it was not added.** The
  spec says Enter "creates the todo, appends it to the todo list, and clears
  the input". Cleared when the command answers, what a person typed while the
  command was in flight was lost (found in WebKit, where the answer is slower);
  cleared at once, it is not, and a todo the host refuses (`full`, a network
  failure) is given back to the field where nothing has been typed since.
- **A save ends only its own edit.** A blur saves the todo being edited and,
  once the host answers, ends the edit unless another todo has been begun
  since: a double press on a second todo begins it by leaving the first's
  field, and the first's answer must not end the second's edit (found by the
  suite: "editing another todo saves the first").
- **Persistence is the framework's own**, the spec's allowance in place of
  `localStorage`: a session's todos, kept by the host, outliving a reload, not a
  restart of the host, and not a device that clears its cookies. **The version
  on a device's own storage waits for charter §9.1's fourth kind of state**, which
  starts with the Docs app (docs/NEXT.md), and is TodoMVC's second version then.
  What the official Cypress suite says of an app that does not use
  `localStorage` is the next section's.

### 4. Two things TodoMVC needed of the runtime

Building the page found two things the runtime did not do, which no page had
asked of it. Each is its own commit in the shared runtime, with its tests and
its mutants.

- **A patch that checks or unchecks a box sets the box's state.** The runtime's
  `setAttributeAt` set the `checked` attribute and nothing else, which moves a
  checkbox only until a person has pressed it: from then the property is the
  box's own. The toggle-all, pressed, stayed checked when one todo was cleared
  (the official suite asks it: "complete all checkbox should update state when
  items are completed / cleared"), and a row another tab cleared stayed ticked in
  the tab that had ticked it. A signal's attribute already set the property
  (`setAttributePart`); a patch's now does, for `checked`. `selected` is the same
  and is not done: no page needs it, and none tests it. Three tests in three
  engines, each failing without the change, and two mutants.
- **An element that arrives with `autofocus` takes focus.** HTML runs
  `autofocus` once, when a page loads, and ignores it on an element put in
  later. TodoMVC's edit field appears when a person asks, and "the edit field
  should be focused": the field they asked for is the one they type in. The
  runtime watches what is put into the page from its start (`followAutofocus`,
  a `MutationObserver` on the body, which sees a block's rendering and a
  server's patch alike) and gives the first element that asks for focus, if it
  does not have it. What the page was served with is left alone (the browser
  focused it), and a modal dialog's `autofocus` element is the one `showModal`
  focuses. A runtime behaviour, not a language rule: the attribute is HTML's
  and the page says nothing new. `examples/demo/focus.pw` and
  `e2e/focus.spec.mjs` show it in three engines (7 of the 12 fail without the
  change; WebKit focuses the first field by itself, and not the second), and
  TodoMVC's own test holds it on the edit field a patch inserts.

## Acceptance

### 1. The fragment

- `compiler/pw-core/tests/address_fragment.rs`, 8 tests: the plan carries the
  binding and a page that states none has none; each of PW6101 to PW6106 with
  its control (the same page written well checks clean); a clause written twice
  and a clause on a query are refused. The policy table's own tests hold the
  clause to a domain and a place (`policy::tests`, `clause_places`), and
  `codes::tests` hold PW61 to its owner.
- `spikes/own-renderer/server/src/tests/address_fragment.rs`, 3 tests: the
  document of a page that reads queries and of a page of signals alone name
  the binding, a page that binds none says none, and the host renders the first
  value whatever the address's fragment.
- `spikes/own-renderer/e2e/fragment.spec.mjs` over `examples/demo/fragment.pw`,
  10 tests in Chromium, Firefox and WebKit: the document names the binding; an
  address with a form shows it at start, and what reads the signal follows (a
  text, an attribute, a block); the default is no change; a link, the back and
  forward buttons and a script each change it; a fragment that is no form, and
  the empty one, are the default; a form is its characters exactly; the
  signals it does not bind keep their values; a page that binds none ignores
  its address's.
- `scripts/address_fragment_mutations.py`, 23 mutants, `just
  e14-address-fragment`.

### 2. The two events

- `compiler/pw-core/tests/declared_events.rs` (PW5022 lists both, and each is
  accepted on an element) and `events.rs`
  (`a_double_press_and_a_blur_are_given_their_records`: each handler given its
  record, run under Node, and another event's record refused, PW0602).
- `spikes/own-renderer/e2e/events.spec.mjs` over `examples/demo/events.pw`, 2
  new tests in three engines: a single press runs the press's handler and
  not the double press's, a pair runs the double press's once, given where
  the pointer was; a control losing focus runs its handler, given its value,
  typing is not leaving, and one with no handler for it runs none.
- `scripts/dom_events_mutations.py`, 8 mutants, `just e14-dom-events`.

### 3. TodoMVC

- `spikes/own-renderer/server/src/tests/todomvc.rs`, 14 tests of the host:
  `.main` and `.footer` hidden, not absent, with no todos, and shown with one;
  todos appended to the bottom, ids never reused; the host holding a title to
  its invariant (empty and over 1000 refused, 1000 added); the counter's words
  and its `<strong>`; toggling one and every todo, and the toggle-all's checked
  state following them; clear completed, and its button hidden with none; an
  edit saved trimmed and one emptied destroying the todo; a todo that is not
  there not found; a session's todos its own, by the page, by a command and by
  the layer; the bound; the model filtering by the page's key, and only what
  changes sent; the key naming the row being edited; the template's classes,
  the new-todo field's `autofocus`, the filters' links and TodoMVC's
  stylesheets.
- `spikes/own-renderer/e2e/todomvc.spec.mjs`, 43 tests in Chromium, Firefox
  and WebKit, in the spec's own order: no todos; the new todo (Enter, the
  field cleared and focused, appended, trimmed, never empty, bounded, markup
  shown as text); mark all as complete; an item (its checkbox, hover for
  `.destroy`); editing (a double press, Enter, blur, clicking away, trimmed,
  emptied, Escape, not persisted, another todo's begin saving the first,
  nothing changed, a completed todo being edited); the counter; clear
  completed; persistence (a reload, another visitor, a second tab live);
  routing (each filter and its `selected` link, the model filtering, an item
  updated under a filter, the back button, a page opened at a route and kept
  on reload, a fragment that is no route); the template's markup; and
  TodoMVC's stylesheets, served and applied, and the licence beside them.
- `scripts/todomvc_mutations.py`, 49 mutants: 38 of the program, each
  checked to be a well-typed program before it runs, 10 of the layer and 1
  of the host; `just e14-todomvc`.

## Not claimed

- **A signal of another type than `String`**, or a form that names a case.
- **The signal writing the address**, or a handler that moves the page by
  setting a form (PW6105 refuses it).
- **A page shown in a layout** (ADR-0303): a navigation that keeps the layout
  hands the document to the next runtime, whose listener replaces this one's
  (`life.signal`); no test holds a fragment-bound page inside one.
- **The links of the views a page composes** (PW6106).

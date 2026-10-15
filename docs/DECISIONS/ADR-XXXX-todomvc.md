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

## Acceptance (the fragment)

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

## Not claimed

- **A signal of another type than `String`**, or a form that names a case.
- **The signal writing the address**, or a handler that moves the page by
  setting a form (PW6105 refuses it).
- **A page shown in a layout** (ADR-0303): a navigation that keeps the layout
  hands the document to the next runtime, whose listener replaces this one's
  (`life.signal`); no test holds a fragment-bound page inside one.
- **The links of the views a page composes** (PW6106).

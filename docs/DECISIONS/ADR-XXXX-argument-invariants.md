# ADR-XXXX: a value from outside is held to its type's invariant at every boundary it crosses

Status: accepted under the owner's delegation of 2026-10-02 (the
integrator's ruling of 2026-10-15, on track `reviews`' finding of
2026-10-14). Date: 2026-10-15. Milestone: E14. Track
`argument-invariants`. Builds on ADR-0179 (an opaque type states its
invariant, and a host holds an argument and an answer to it), ADR-0225 (a
`String`'s length), ADR-0331 (a case from a browser), ADR-0309 (a page's
query parameters), ADR-0194 (a type that contains itself crosses as its
nodes) and ADR-0302 (a refusal is told where the press was). Charter §7.10:
"Every value entering from JSON, forms, headers, cookies, browser storage,
databases, third-party APIs, or message queues starts as `Unknown` until
decoded."

## Context

- **The finding** (track `reviews`, 2026-10-14): `rate_order(store, Stars
  6 / 0, ..)` and a 501-character `Comment`, opaque types whose `where`
  invariants are 1 to 5 and at most 500 code points, reached the data
  layer (`store:data/ratings#rate`), which refused them only by its own
  check and PostgreSQL's CHECK.
- **Correction, first** (this track's reading, then a failing test): the
  values went in through `Server::command_answered(.., &[Val..])`, the
  server's entry for values already decoded, which `reviews`' tests call
  (`tests/reviews.rs`, `rate()`). A browser's own path, `POST /command/<id>`
  to `command_answer` to `Prepared::arguments_for`, held both kinds on
  master: ADR-0179's `e2e/compiled-handler.spec.mjs` refuses a quantity of
  0, and ADR-0225's `a_posts_text_is_held_to_its_length_where_it_arrives` a
  281-character post, 400 and why, before the command runs. What did not
  hold was everything else:
  - **the component's call boundary**: `Prepared::call_within`,
    `call_authorized_within` and `call_measured` took the values they were
    given. A quantity of 0 ran `add_to_cart` through them and reached
    `store:data/user-carts#add` (`3164d601`, failing on master by design);
  - **a page's parameters**: its route's path and query (ADR-0309), and an
    unrouted page's `/page/<page>?n=v`, were given to the page and its
    queries as the address said them. ADR-0179 did not claim them;
  - **a map's keys and values and a set's items**: the build wrote no path
    into a `Map` or a `Set`, so an invariant there was held nowhere.
- **The ruling** (the integrator, 2026-10-15): a value of an opaque type
  with a `where` invariant exists only if it passed the invariant. A
  command's arguments come from the browser: external input. They are
  decoded through the same checks at the boundary (parse, don't validate),
  and a command called with an argument that breaks its type's invariant is
  refused before it runs; neither the command's code nor the data layer
  ever sees such a value. Where the decode lives, component or host, is the
  track's to decide; the component and the host are trusted, the browser
  is not.

## Decision

1. **A component's call boundary holds its export's invariants, whatever
   built the values** (`runtime/pw-host`, `Prepared::hold_arguments`).
   - Each call API holds the arguments to the export's `bounded` before
     anything else: before a `requires` predicate reads them (a predicate
     reads through the data layer, as `OwnsPost(post)` does), before the
     component is instantiated. The free functions delegate to them, so no
     call into a compiled component passes a value its type does not hold.
   - Held as the component is passed them: a value of a type that contains
     itself as its nodes, where the contract's paths are (ADR-0194), however
     the host gave it (`graph::tangle`, which leaves nodes as they are).
   - In a decoder's words, as ADR-0179's: "argument refused: value 2 is 0,
     and `domain.PositiveInt` holds `value >= 1`".
   - The browser's path keeps ADR-0179's decode at arrival
     (`arguments_for`): refused there, a request keeps nothing (its
     interaction is free), and is answered 400.
   - The call's check is the last of them, not the one a reader meets: a
     value it refuses is one no boundary held, a host's own error. A
     command so refused fails its press as a command that did not commit
     (202 and no value, the press told in the platform's words); a query,
     as a query that failed. No value from outside reaches either today:
     each enters through a boundary that holds it first (the audit).
2. **A page's parameters are held where its address gives them.**
   - The build states each parameter's invariants in the page's plan
     (`PageValues.bounded`), by the function an export's contract uses
     (`contract::bounded_params`, from each type's declaration), at the
     parameter's position. Not written where none is stated: every plan
     that holds nothing is as it was.
   - The development server holds a routed page's path and query
     parameters, and an unrouted page's `/page/` ones, before the page is
     rendered or a query reads them (`held_params`): one that breaks them is
     refused 400 and why, "parameter refused: value 1 is 1 code points long,
     and `demo.invariants.Code` holds ..", as ADR-0309 refuses a query that
     is no UTF-8. A query's parameter is held inside its `Option` (`some`).
3. **The build reaches a map's entries and a set's items**: a map crosses
   as `list<tuple<k, v>>` and a set as `list<t>` (ADR-0057), so a path
   steps into each entry (`*`) and its key (`0`) or value (`1`), and into a
   set's items as a list's.
4. **The decode lives in the host.**
   - The host is where a browser's bytes become values. Refused there, a
     request is refused before the host's own work on it (an interaction's
     record, a command's order, ADR-0305) and before a `requires` predicate
     reads the data layer with it; and it is refused as a request, 400 and
     why.
   - One mechanism for every boundary: the host reads the contract's paths
     and the plan's, and holds them with `engine::holds`, without the
     program's types (ADR-0179, ADR-0018), at the arrival of a command's
     arguments, a page's parameters and a data layer's answer, and at every
     call.
   - The component and the host are trusted (the ruling). A check compiled
     into the component's export would trap, which a host could only tell
     from any other trap by its words; it would run after the predicates;
     and the browser's backend would run it where nothing is trusted.
5. **A reader's page sends only what its own checked constructors build**
   (ADR-0179), so a value that breaks its type arrives only from a request
   forged or altered on the way, or from a page another build made. The
   press it came from is told in the platform's words (ADR-0302, "This did
   not work. Try again.", `data-pw-handler-error="failed"`): no predicate
   refused it, and a type has no words of its own a reader could be told.

## The audit

Every place a value enters a program from outside, held before this track
(master 5da9c055, merged 22a0508e) and after it:

| boundary | where | held before | held after |
|---|---|---|---|
| a command's arguments from a browser, `POST /command/<id>` | `command_answer`, `Prepared::arguments_for` | yes (ADR-0179, ADR-0225), 400 | yes, 400; and again at the call |
| a call into a component, whatever built its values (a test, `command_answered`, a host's own) | pw-host's three call APIs | **no** | yes, before `requires` and instantiation |
| a route's path parameters | `Server::routed` | **no** (ADR-0179, not claimed) | yes, 400 |
| a route's query parameters, a GET form's fields (ADR-0309) | `Server::routed` | **no** | yes, inside the `Option`, 400 |
| an unrouted page's parameters, `/page/<page>?n=v` | the `/page/` route | **no** | yes, 400 |
| a form's fields otherwise | | a POST to a page is 405, and a command is a JSON request (ADR-0302); a control's value reaches a command through its handler | as a command's arguments |
| a keyed read's key, `/pw-read` (ADR-0152) | `read_keyed`, `key_val` | no; PW5308 holds a key to a `String`, an `Int` or a `Bool`, so no invariant is there | at the call |
| the payment provider's word (ADR-0332) | `settle_payment`, `command_answer` | yes | yes |
| a scheduled run's stored arguments (ADR-0334) | `run_scheduled`, `command_answer` | yes | yes |
| a data layer's answer, every host operation, both store layers | pw-host's linker, `Import.bounded` | yes (ADR-0179); untested on PostgreSQL, whose CHECK refused the row | yes; on PostgreSQL with the CHECK dropped |
| a value kept in a session: its id, its principal | `pw:host/session#read`, the principal's operation | no type there states an invariant (`SessionId`, `UserId`) | as a data layer's answer, were one to |
| "the browser's reply to a speculation" | | no such message: a speculation is the browser's own; what comes back is a handler's captures, re-rendered from a speculated value (ADR-0217) and sent as a command's arguments | as a command's arguments |
| a materialization's read of a resource (ADR-0277) | `derive_value`, `read_value` | no `bounded`: the value is the program's own, from the host's memory or a query's answer, each held where it entered | not claimed |
| inside a value: a record's field, a list's item, an `Option`, a `Result`, a case's payload | `bounded_in`, `held` | yes | yes |
| inside a value: a map's key and value, a set's item | | **no** | yes |
| a type that contains itself, at the call | | no call held anything | yes, as its nodes |

## Alternatives

- **Decode in the component**, a check compiled at each export's entry,
  trapping. Rejected (decision 4): after the predicates, a trap and not a
  refusal, and in the browser's backend where nothing is trusted.
- **Hold each entry the host has today, and not the call.** The next entry
  a host grows (a stored run, a test's helper, the second host's) would be
  unheld again, as `command_answered` was.
- **Refuse a broken argument as a predicate is refused**, 403 with
  `refused` and `says`. Nothing refused it that has words a reader could be
  told; a request that breaks its arguments' types is malformed, which 400
  says, as ADR-0179 and ADR-0309 answer it.
- **422 Unprocessable Content** (RFC 9110 §15.5.21), for a request well
  formed and not processable. An argument that breaks its type's invariant
  is no value of its type: the request does not decode, as a number with a
  fraction for an `Int` does not, which ADR-0179 answers 400.
- **Hold a materialization's reads.** Its values are the program's own,
  from the host's memory; a host that keeps them in a database makes them a
  data layer's answer, and states its `bounded` then.

## Acceptance

Recorded by `just e14-argument-invariants` in
`docs/evidence/E14/argument-invariants.txt`:

- `compiler/pw-core/tests/argument_invariants.rs`: a page's plan states its
  path's `Code` and its query's `Note` inside the `Option`, and nothing
  where nothing is stated; a command's contract states each place inside its
  arguments; a map's key and value and a set's item are stated.
- `runtime/pw-host/tests/pleris_component.rs`,
  `an_argument_that_breaks_its_invariant_never_reaches_the_component`: 0,
  -3 and the least `s64` refused by each of the four call APIs, no operation
  called and no predicate asked; 1 runs each way, and its predicate is asked.
- `compiler/pw-conformance/tests/argument_invariants.rs`: a record and a
  case that contain themselves, an invariant inside each, given nested and
  as nodes, held and refused alike; a map's value, a map's key and a set's
  item refused by place.
- The development server (`tests/argument_invariants.rs`, the fixture
  `examples/demo/invariants.pw`): each invariant held at both its ends on a
  command, an `Int` from 1 to 5, a `String` of 2 to 8 code points and one
  of at most 20, 20 emoji taken and 21 refused; refused before anything is
  kept; each nested place named; the server's entry for values it built
  refused as the browser's is (`reviews`' path); a page's parameters at both
  ends, routed and by name, 400 and why; and on PostgreSQL a row written
  before its CHECK existed is refused where it is read.
- `e2e/argument-invariants.spec.mjs`, in Chromium, Firefox and WebKit: a
  page's address at both ends; forged commands refused 400 with their
  words, nested places named, and values just inside committed; a press
  sends only what the page's constructors build; a press whose request is
  altered on the way is refused and told where it was.
- `scripts/argument_invariants_mutations.py`: each mutant removes one
  boundary's check or one piece of how it reads; each `.pw` mutant is
  checked to be a program first.

Re-anchored, mutants unchanged: `invariants_mutations.py`'s "an export
states no argument's invariant" (the line now assigns `bounded_params`).

## Corrections found on the way

1. **The finding's path** (Context): the browser's path held the
   arguments on master; the call boundary and a page's address did not.
2. **This track's first call check held a type that contains itself by the
   wrong form** (found by its own audit): it held the arguments as given,
   and the contract's paths are the nodes', so a valid nested record was
   refused ("has no field `*`") and a nested case passed over unread.
   `hold_arguments` makes them the component's form first (`2c5e8af9`).
3. **ADR-0179's data-layer test proved nothing on PostgreSQL**: the
   database's CHECK refused the row, and the test returned before reading.
   This track's drops the constraint in the test's own schema.
4. **The host's memory test called `add_to_cart` with a quantity of 0**,
   its first of 20,000; the call now refuses it, and the test counts from 1.

## Not claimed

- **A materialization's reads** (the audit): the program's own values.
- **A world's export** (a hand-written guest's) whose contract states an
  invariant: only the compiler states one, and its exports are an
  interface's; such a call is refused for its path, never passed unheld.
- **The browser's own decoders** read an opaque value as its
  representation and trust it; the browser is not trusted, and what it
  sends is held where it arrives.
- **Words a type gives a reader** of its own, as a predicate's `says`.
- **A path segment that is no UTF-8** is answered 404 (no route matches),
  where ADR-0309 says 400: found by this track's audit, not changed here.

## Questions for the integrator

1. The `mutant-programs` gate (train-7) maps no program for
   `examples/demo/`, so the fixture's three `.pw` mutants are not checked
   by it; this track's script checks each itself before its tests run.
   Should the gate's programs include the demos?

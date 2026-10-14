# ADR-0331: a case crosses from a browser

Status: accepted under the owner's delegation of 2026-10-02 (the
integrator's). Date: 2026-10-13. Milestone: E14. Found by W9 in NEXT 6b
(the reader's tip). Builds on ADR-0058 (a value a handler sends a command),
ADR-0172 (a record and a list from a browser), ADR-0179 (an argument's
invariant held by the host), ADR-0059 and ADR-0061 (a declared case and its
fields, as the browser holds them).

## Context

- **A page could not send a command a value of a sum type.** The handler's
  compiler refused it ("a command parameter a browser cannot send": a
  handler sent primitives, opaque types over them, lists and records), and
  the browser's wire and the host refused it too ("is not accepted from a
  browser").
- **So a choice among cases was sent as a number.** W9's tip became
  `choose_tip(percent: Int)`, taking 0, 15, 18 or 20 and refusing any other
  at run time (`TipError.NotOffered`), where `choose_tip(Tip)` would refuse
  it at check.
- **The browser already holds a case as one**: `{ $case, value }`, its WIT
  name and its payload (ADR-0059), `Option`'s `some` and `none`, `Result`'s
  `ok` and `err`, as a signal's value is written; and a host already holds
  an argument's invariant inside a case (`some`, `ok`, `err`, a declared
  case's name, a field's place).

## Decision

1. **A handler sends a case of what it sends**: `Option<T>`, `Result<T, E>`
   and a declared sum type whose every case's fields a browser can send, as
   it holds it, `{ "$case": <its WIT name>, "value": <its payload> }`, a
   case of several fields an array of them in order.
2. **A host reads it by the parameter's own type**: a WIT variant (every
   Pleris sum type is one, its cases with no payload included; no
   program's component exports a WIT enum, and a host reads none from a
   browser), an option and a result, case by case, and a case's several
   fields as a tuple. It refuses, naming
   the argument, a case the type does not declare, a payload a case has
   none of, one it lacks or of another type, a key a case does not carry
   (only `$case` and `value`), and a value that is no case. ADR-0179's
   invariants hold inside a case as before.
3. **A case of a type that contains itself is not sent**: a host reads none
   from a browser, so the handler is refused where it is compiled, by name.

## Correction, 2026-10-13 (found by train-4's run)

Its change to `compiler/pw-core/tests/handlers.rs` replaced the test's one
refused record, which held a case and is now sent, with one refused for a
field's name, and left no test of a record holding what a browser cannot
send: `cart_lines_mutations.py`'s "a record holding what cannot be sent is
sent" survived (verify 38451028940, 40 of 41 killed). A `Grove { tree: Tree }`,
its field's type one that contains itself, is refused by its name
(`e9e6a49f`); the recipes that read the test ran again by dispatch
(38483504682).

## Alternatives

- **A case's index, or its name as a bare string.** The browser already
  holds a case as `{ $case, value }`; another form for the wire would be a
  second encoding to keep in step with it.
- **Keep W9's two commands.** A command whose argument is a number refuses
  the wrong number at run time; one whose argument is the choice cannot be
  given a wrong one.

## Acceptance

- `compiler/pw-conformance/tests/case_arguments.rs`: a declared case read
  by its name and payload, one of several fields among them, and the
  component given the case it was sent; one whose cases carry nothing;
  `Option`'s and
  `Result`'s; what is not a case of the type refused by the host, naming the
  argument; a page's handler pressing a command with a declared case, an
  `Option` and a `Result`, each encoded with `$case`; a case of a type that
  contains itself refused where the handler is compiled.
- Every program and every other test as before (`just ci`).
- `scripts/case_arguments_mutations.py`, each mutant killed (`just
  e14-case-arguments`).

## Not claimed

- **A case of a type that contains itself**, refused by name.
- **A generic sum type's case** whose payload is its parameter: a handler
  sends the types it can name.
- **A `Map` or a `Set`**: not sent, as before.

# ADR-0058: handlers that compute

Status: accepted under the owner's instruction of 2026-09-25 ("ok fix all the
known gaps"). Decisions marked **(ruling needed)** were made without a ruling
and are offered for reversal. Date: 2026-09-25. Milestone: E10 (charter §14
M10 task 2, "generated modern JavaScript modules for handlers").

## Context

ADR-0033 compiled a resumable handler's body to an ES module, and only one
shape of body: one command call, whose arguments were captures, fields,
literals and opaque constructors. Everything else was refused by name:
- an operator;
- a call to the program's own function;
- a branch, a binding or a loop;
- a second command.

KNOWN_LIMITATIONS: "One command call per handler."

The handler backend wrote JavaScript straight from the HIR, and wrote an
`Int` as a JavaScript number. The pure-computation modules (ADR-0044) write
it as a `BigInt`, exact to 64 bits. So two meanings of `Int` would have met
in the browser as soon as a handler computed.

## Decision

### 1. A handler's body is lowered as a query's is

`lower::handler` lowers the body into the backend IR:
- **Parameters:** its parameters are the paths it captured, each the value
  the document carries for it, in `resume::capture_paths`' order.
- **Captured reads:** a read of a captured path is that value, unless a
  binding in the body shadows its root.
- **Commands:** a call to a command is `Instr::Command`, by the contract's
  component id.
- **Everything else** lowers as a query's body does: bindings, arithmetic,
  strings, lists, maps, branches, matches, `for` loops, early returns, and
  calls to the program's own functions.

The pure-computation emitter writes the module, in a handler mode:
- its `run(context)` is async, and each command is awaited in order;
- a command that is refused rejects its promise, so the handler stops there
  and the runtime marks the element (ADR-0033 §4).

### 2. The two representations meet at the handler's edges

- **In:** a captured value arrives as JSON. An `Int` there is a JavaScript
  number, exact within ±2^53 (ADR-0033 §7), and is read into a `BigInt`. A
  record is read field by field, a list element by element.
- **Out:** an `Int` sent to a command is sent as a number. Past ±2^53 the
  handler traps before sending, rather than sending a different number.
  ADR-0033 refused such a literal when compiling; a computed value can pass
  the bound too, so the check is where the value is sent. An opaque type is
  sent as its representation.

### 3. What is refused, by name

- **A command called inside a function value**, or in a function compiled
  beside the handler. It would not be awaited by the handler.
- **A function value that reads what the handler captured.** Its code does
  not receive the captures.
- **A query called from a handler.** A query answers on the server.
- **A handler that calls no command.** Pressing it would change nothing.
- **A command parameter that is not a primitive** or an opaque type over
  one (ADR-0033 §4).
- **A handler with parameters.** No syntax binds one to a resumable
  handler, and the runtime listens for a click only.

**(ruling needed)**: a command's answer is not read by the handler; its
value is the unit value. The runtime ignores `run`'s result, and nothing
decodes a command's result in the browser.

### 4. What changed for the store

The store's two handlers compile through the new path.
- **Identities unchanged:** an identity hashes the body's tokens.
- **Text changed:** the module text has changed.
- **Same requests:** they send what they sent. `add_to_cart` reads the
  captured `item.id`, builds `PositiveInt(1)`, and sends `["<id>", 1]`.

The committed modules are regenerated. The e2e spec's check of the module
text now checks what the module reads and awaits, and its request check is
unchanged.

## Acceptance

- `compiler/pw-core/tests/handlers.rs`, 16 tests. Each module runs under
  Node against a context that records the commands it sends:
  - the store's two;
  - computed arguments, a branch between two commands, and a loop sending
    three;
  - a refused command stopping the next;
  - an `Int` past ±2^53 trapping before it is sent;
  - an overflow trapping;
  - escaped strings;
  - each refusal.
- `compiler/pw-core/tests/evidence_is_current.rs`: the committed modules
  are the compiler's output.
- Mutation controls: `scripts/handler_mutations.py`,
  `just e10-handlers-compute`, 10 mutants. ADR-0049's control "a handler
  sends the token" now anchors in the lowering, where a handler's strings
  are made.

## Not done

- **The event as a parameter.** It needs a syntax that binds one to a
  resumable handler **(ruling needed)**, a runtime that listens for the
  part's own event (it listens for a click), and the event's fields carried
  to the module. The platform declares the event types (`events.pw`).
- **A command's answer**, read by the handler.
- **The development server hosts the store's two commands only.** A handler
  that calls another command compiles and is tested under Node, not served.

# ADR-0113: a resumable handler runs in the browser

Status: accepted under the owner's instruction of 2026-09-26 ("keep working
until its perfect"). Date: 2026-09-26. Milestone: E10 (charter §7.7, §8,
ADR-0033).

## Context

A resumable handler is loaded and run in the browser when its element is
pressed (ADR-0033). It reaches the origin through a command, and a command is
its own component with its own contract. So the page's contract leaves its
handlers out (`contract.rs`): "the handler's own authority is not lost: the
command it calls is itself a component with its own contract". That holds
while a handler only calls commands, and nothing required it to.

On 2026-09-26, at 389c41b, the store page's Add handler passed `pw check`
when written `resumable(captures = { item }) => Carts.add(current_session(),
item.id, PositiveInt(1))`. It performs `database.write<Carts>` itself, in the
browser. `pw emit-handlers` refused it: "a handler reaches the server through
a command, and this one calls none". The effect had no contract, and nothing
asked where it could be performed.

## Decision

**A resumable handler runs in the browser.** This is PW5005: a declared
placement must be able to grant every effect it requires, and a handler's
placement is the browser. The effects a handler performs itself are inferred
from its body. What reaches it through a call to a command is left out,
because the command performs that. Each remaining effect the browser cannot
grant is refused, once per handler, at the call that performs it.

A handler that is not resumable is not asked. Its element is left inert
(KNOWN_LIMITATIONS), and what it compiles to awaits a ruling. Once it runs,
it runs where a resumable one does, and this rule is its rule.

## Corpus change

Two fixtures' handlers called `connection.flush()`: the rejected R-010 and the
witness `generality/unserializable_capture/direct.pw`. That call is
`Database.flush`, which performs `database.connect` in the browser. It is a
second defect beside the one each fixture is about, and each now reported two
diagnostics. Each handler now calls a local function that performs nothing,
as the sibling witness `via-field.pw` does. Each capture, the fixture's
subject, is unchanged, and each fixture emits its invariant alone, before
the change and after it. R-037 was corrected the same way (ADR-0074).

## Acceptance

- **`compiler/pw-core/tests/handlers_in_the_browser.rs`**, 1 test, failing at
  389c41b: a handler writing the cart itself. Its controls are the store's
  handler calling the command, and one calling a function that performs
  nothing.
- **Corpus.** Beyond the two fixtures above, no rejected, rule or generality
  fixture's diagnostics change in the harnesses. The store, kiokun and the
  accepted corpus check clean.
- **Mutation controls:** `scripts/handler_placement_mutations.py`,
  `just e10-handlers-in-the-browser`, 4 mutants.

# ADR-0086: a function crosses no boundary

Status: accepted under the owner's instruction of 2026-09-26 ("keep working
until its perfect"). Date: 2026-09-26. Milestone: E10 (charter §8.5,
resumption).

## Context

A resumable handler's captures are written onto the element as data, and
read back when the handler runs (ADR-0033). One decision procedure,
`boundary::can_cross`, says what may cross: it refused a resource, whose
value is the thing held open (PW5008), and a private value bound for a
public place. A function was neither.

On 2026-09-26, at e273c60, a handler that captured a function checked:
- `f`, a view's parameter of type `fn(Int) -> ()`;
- `f`, a local declared `let f: fn(Int) -> () = go`.

The build refused the handler for a different reason: "`f` names no
declaration this program contains".

## Decision

**A function crosses no boundary.** A value whose type is, or holds, a
function type cannot cross, because a function is code, not data.
- A resumable handler that captures one is refused (PW5008, "handler
  captures `f: fn(Int) -> ()`, which is not serializable"). The repair is to
  call the function by name and capture the data it would have been given.
- `binding.rs`, which reads the same verdict for a call to another
  placement, refuses it as untransferable.

A local whose type nothing states, `let f = go`, is refused as before, as a
capture with no nameable type (PW5016).

## Acceptance

- **`compiler/pw-core/tests/function_captures.rs`**, 1 test with a
  control. It fails at e273c60, the commit before.
- **No rejected, rule or generality fixture's diagnostics change.** The
  store, kiokun and the accepted corpus check clean.
- **Mutation controls:** `scripts/function_capture_mutations.py`,
  `just e10-function-captures`, 2 mutants.

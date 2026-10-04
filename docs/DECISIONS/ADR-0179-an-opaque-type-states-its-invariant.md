# ADR-0179: an opaque type states its invariant, and every construction and every boundary holds it

Status: accepted under the owner's delegation of 2026-10-02. Date: 2026-10-04.
Milestone: E14, the audit's eighth gap: charter §15.1's `quantity:
PositiveInt`, "checked at the boundary". Charter §7.1 asks for "explicit
decoding at every external boundary", and for no "unvalidated cast in normal
code".

## Context

- **`PositiveInt` promised what nothing kept.** `opaque type PositiveInt =
  Int` stated no invariant, so none was checked. ADR-0054 left it a "(ruling
  needed)": building an opaque value checked nothing. KNOWN_LIMITATIONS
  listed it: "Opaque invariants are not checked at the boundary".
- **Measured on 2026-10-04**, through the development server's command path:
  - a forged `add_to_cart` with a quantity of 0 committed, and the cart held
    `espresso × 0`;
  - a second, with −3, committed, and the cart held `espresso × −3`.

  The host decoded each as an `s64`, and the data layer added it to the line.
- **A construction inside the program could break it too.** `fewer(n)` built
  `PositiveInt(n.value - 1)`, which is 0 for a line of one. The cart's
  `with_fewer` filtered such lines out first, a fact in another function
  that nothing checked.

## Decision

1. **An opaque type may state its invariant**: `opaque type PositiveInt =
   Int where value >= 1`.
   - The invariant is bounds on `value`: each compares `value` with an
     integer by `<`, `<=`, `>` or `>=`, and bounds are joined by `&`, the
     language's and. `0 < value` reads as `value > 0`.
   - The representation is an `Int`.
   - PW0623 refuses anything else by name: another predicate, another
     representation, bounds no value holds, a bound beyond what an `Int`
     holds.
2. **Every construction is shown to hold it, at build** (PW0622).
   - The value analysis bounds the `Int` a construction is given. A literal
     is itself. A bounded value's `.value` is its bounds. `+`, `-` and `*` are
     exact. A `let`'s local is its initializer, where the `let` is. An `if`
     or a `match` is either branch.
   - A test narrows what it tests: a local, or a field read through one,
     compared with an integer. It narrows in the `if`'s branch, its negation
     in the `else`, and on the right of `&` and `|`.
   - Anything else is any `Int`, so a construction from it is refused, and
     the diagnostic says what the value may be: "it is at least 0".
   - Sound because both backends trap where an `Int` operation overflows: a
     value computed at all is the integer the expression means.
   - The way to build from a value the build cannot bound is to test it:
     `if n >= 1 { Some(PositiveInt(n)) } else { None }`. That is the
     program's own checked constructor, and the language keeps one way to
     build a value.
3. **Every external boundary checks it.**
   - The contract states where each must hold: an export's argument, and a
     host import's answer. Each is a path from the value, as the component
     names it: `ok.lines.*.quantity`.
   - The host checks a command's arguments as it decodes them
     (`arguments_for`): a forged quantity of 0 is refused before the
     component runs, as a number with a fraction is.
   - The host checks a data layer's answer before the component reads it: a
     cart line of 0 is a failed call, never a value.
   - An export's checks are part of its `abi_schema`, as its authorization
     is.
4. **The store's `PositiveInt` states `value >= 1`.** `fewer` answers
   `Option<PositiveInt>`: one fewer than one is none.

## Alternatives

- **Check at run time, and trap.** Ada's `subtype Positive is Integer range
  1 .. Integer'Last` is checked so, raising `Constraint_Error`. A broken
  invariant would be found by a reader, not the build, and charter §7.1 keeps
  unchecked exceptions out of ordinary control flow.
- **A checked constructor built into the language.** Rust's `NonZero::new`
  answers an `Option`, and its `new_unchecked` is `unsafe`. The program's own
  function, an `if` and a construction the build can show, says the same.
- **Refinement types over any predicate, decided by a solver** (Liquid
  Haskell, F*). The build would depend on an SMT solver, and a host could not
  check an argument without running the program's code.
- **The invariant as a function the program writes.** The build could not
  decide a construction, and the host would run program code to decode.
- **Check at the boundary alone.** The program could still build
  `PositiveInt(0)` itself.
- **The host reads the program's types.** The contract states paths
  instead: the host stays without the program's type system (ADR-0018).

Alexis King's "Parse, don't validate" (2019) puts the principle this
follows: a value of the type is evidence that the check was made, once,
where the value entered.

## Acceptance

Recorded by `just e14-invariants` in `docs/evidence/E14/invariants.txt`:

- `pw-syntax`: an opaque type's `where` and its predicate are inside its
  declaration, after its representation, losslessly.
- `pw-core` (`tests/invariants.rs`):
  - a literal is itself, and 0 and −5 are refused;
  - the sum and a multiple of bounded values hold, and one fewer may be 0,
    refused, "it is at least 0";
  - a test narrows in its branch and its negation in the `else`; a test of
    another value narrows nothing; a conjunction narrows where it holds and
    only there, and a disjunction where it fails;
  - a `let` is its initializer; a branch is either of its values; a piped
    value is held too;
  - an upper bound, `<`, `>` and a bound written the other way round are
    each read as they mean;
  - an opaque type that states nothing checks nothing;
  - PW0623 for another representation, `==`, a call, another name, an empty
    range, and a bound beyond an `Int`;
  - the store builds every count it can show;
  - the store's contracts state `add_to_cart`'s second argument and each
    cart a data layer answers, `ok.lines.*.quantity`.
- The conformance oracle: generated arguments and answers are fitted to the
  contract, and the component and its reference still agree.
- `pw-host`:
  - `tests/bounded.rs`: both ends; each item of a list and each field; a
    case the path does not name holds nothing to check; a value of another
    shape than the contract says is refused;
  - the store's compiled `add_to_cart`: 0, −3 and the least `s64` refused as
    they are decoded, by name, and 1, 2 and the greatest taken; a data
    layer's cart line of 0 is the command's failure.
- The development server: a forged quantity of 0 or −3 is refused before the
  command runs, and the cart is as it was; a stored line of 0 fails the
  cart's read, by name.
- `e2e/compiled-handler.spec.mjs`, in Chromium, Firefox and WebKit: a request
  with a quantity of 0 or −3 is answered 400 with why, and the cart does not
  move; 2 adds 2.
- `scripts/invariants_mutations.py`: 25 mutants.

Also:
- **The store's contracts change** (`docs/evidence/E8`, `just
  e8-contracts`): each states its boundary's checks, and the five exports
  that take a count have a new `abi_schema`.
- **The trusted platform contract's hash changes**: `examples/domain.pw`'s
  `PositiveInt` states its invariant, and `fewer` answers an `Option`.
- **The conformance oracle's generator** fits each value to the contract, as
  a host passes nothing else.

## Not claimed

- **A test after an early `return`** does not narrow what follows it; an
  `if` with an `else` does.
- **A keyed read's key and a route's parameter** are not checked against an
  invariant: none of the store's is an opaque type that states one.
- **The benchmark's store** is its own copy (ADR-0156) and states no
  invariant.
- **An invariant over another representation**, a `String`'s length, is
  refused until a program needs it.

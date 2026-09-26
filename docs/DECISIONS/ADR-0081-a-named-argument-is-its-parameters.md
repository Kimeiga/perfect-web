# ADR-0081: a named argument is given to the parameter of its name

Status: accepted under the owner's instruction of 2026-09-26 ("keep working
until its perfect"). Date: 2026-09-26. Milestone: E10 (charter §7.1; NEXT's
"named-argument calls").

## Context

A call may name its arguments: `g(b = 1, a = n)`. A signature carried the
parameters' types and not their names, so nothing could say which parameter
a named argument meant. A probe on 2026-09-26, at 7f1e977, found:
- **A silent miscompile.** The backend passed arguments in written order.
  `g(b = 1, a = n)`, with `fn g(a: Int, b: Int) -> Int { a - b }`, compiled
  as `g(1, n)`: `Named(10)` answered `-9` through the E8 host, where it
  means `10 - 1`.
- **Named arguments checked by nothing.** The value relations left every
  argument of a call that named one undecided. So `f(n = "a", s = "b")`
  passed with `n: Int`. So did a name the callee has no parameter for, a
  parameter given twice, and a positional argument after a named one.
- **A label carried from the wrong argument.** A generic result carries the
  label of the argument its type comes from (ADR-0064), found by position.
  `pick(x = secrets.payments(), n = 0)`, with `fn pick<T>(n: Int, x: T) ->
  T`, carried `n`'s label, public, and a secret logged publicly passed.

No example calls a declaration with named arguments; named arguments appear
only in policy clauses and language forms, which this does not change.

## Decision

- **A signature carries its parameters' names.**
- **One arrangement says which parameter each argument is given to**
  (`signatures::arrange`), and the checker, the labels and the backend all
  read it:
  - a method call's receiver or a piped value takes the first parameter;
  - positional arguments fill the parameters after it in order;
  - a named argument takes the parameter of its name.
- **An arrangement that fails is refused** (PW0617, "a named argument is
  given to the parameter of its name, once, after the positional ones"):
  - a name the callee has no parameter for;
  - a parameter given twice;
  - a positional argument after a named one;
  - a named argument to a function value, whose parameters have no names.
- **The backend lowers the arguments in the order written**, each against
  its parameter's type, and passes them in the order declared. An argument
  with an effect keeps its place in the program's order.
- **A named argument to a standard-library operation** is checked, and
  refused by the backend by name: an operation takes its arguments in
  order.

## Acceptance

- **Tests, each with controls.** All five fail at 7f1e977, the commit
  before, the conformance test on `Named(0)`:
  - `compiler/pw-core/tests/named_arguments.rs`, 4 tests;
  - `compiler/pw-conformance/tests/named_arguments.rs`, which runs a named,
    a mixed and a positional call through the E8 host.
- **No rejected, rule or generality fixture's diagnostics change.** The
  store, kiokun and the accepted corpus check clean, and their artifacts
  are byte-identical, apart from ADR-0058's two handlers.
- **Mutation controls:** `scripts/named_argument_mutations.py`,
  `just e10-named-arguments`, 6 mutants.

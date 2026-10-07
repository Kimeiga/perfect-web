# ADR-0250: `let _` discards a value, and an acquisition is held or refused

Status: accepted under the owner's delegation of 2026-10-02; it builds the
owner's ruling 0099-a (ADR-0210), and closes the hole in PW2005 its probes
found. Date: 2026-10-07. Milestone: E14, the owner's Twitter list, item 2.

## Context

- **`let _ = e` did not parse**: "expected a binding name". A program
  discarded a `Result` by naming a binding it never read, `let _ignored =
  e`, as PW0618's repair told it to (ADR-0099), and the corpus holds
  eighteen such names.
- **Ruling 0099-a** (ADR-0210): "`let _ = e` is an explicit discard and
  satisfies PW0618 for a `Result`. An affine resource bound to `_` is
  refused as never consumed."
- **PW2005 followed a binding, and nothing else.** It found `let x =
  acquire()` and `use x = acquire()`, whose value was the acquiring call
  itself, and followed each name to its releases. Probing `let _` found
  every other place an acquisition can stand, each unread and each
  leaking what it acquires: a statement, `Database.begin()`; an argument to
  a call that does not end it, `audit(Database.begin())`; an `if`
  statement's branch; `Some(Database.begin())`; a function value's result,
  `() => Database.begin()`; and `let h = Maps.create(..)?`, the natural way
  to hold a resource a `Result` carries.
- **A body declaring `()` gave its caller its last value.** `fn open() ->
  () { let tx = Database.begin(); tx }` passed: `tx` moved to a caller that
  never has it. The value checker reads such a body's last statement as a
  statement (`values::returns`, recorded in `docs/ASSUMPTIONS.md`); the
  affine check did not.
- **The repair named the function being checked**: "end it with `destroy`
  or `f`". It listed every declaration whose row releases the type, and
  `f`'s row releases one because its body calls `destroy`.

## Research

- **Rust** refuses `let mut _ = 0`: "`mut` must be followed by a named
  binding" (`compiler/rustc_parse/src/diagnostics.rs`,
  `tests/ui/parser/mut-patterns.rs`). Its `let_underscore_lock` lint, deny
  by default, is the trap the ruling names: binding to `_` "causes the
  expression to immediately drop"
  ([lints](https://doc.rust-lang.org/rustc/lints/listing/deny-by-default.html)).
- **Swift**: "A *wildcard pattern* matches and ignores any value" (*The
  Swift Programming Language*, Patterns). **Go**: "The blank identifier
  provides a way to ignore right-hand side values in an assignment" ([the
  specification](https://go.dev/ref/spec#Assignment_statements)).
- **Koka 3.2.3**'s local `val` takes a pattern, `decl : VAL apattern '='
  blockexpr`, and `wildcard : WILDCARDID | '_'` is one
  (`doc/spec/grammar/parser.y` at `v3.2.3`): `let _ = e` is `val _ = e`.

## Decision

1. **`let _ = e` parses and binds nothing** (`grammar::let_stmt`, a
   `WildcardPat`; `lower`, `Pattern::Wild`). `e` is computed: checked,
   held to an annotation (`let _: Int = e`), and run by the backend, which
   already bound nothing for `_` (ADR-0159). A `Result` so discarded is
   handled: PW0618 says nothing, and its three repairs name `let _ = ..`.
   The Koka translation writes `val _ = e`.
2. **`_` binds nothing elsewhere either.** `let mut _` is PW0001, "`let
   mut` binds a name to change, and `_` binds nothing", as Rust refuses
   it. `use _ = e` is PW0001, "`use` binds a name, and `_` binds nothing";
   it read as `use` and a second statement, PW0030. `_` read as a value is
   PW0021, "`_` binds nothing, and is no value", whose repair binds a name:
   it told the program to bind `_`.
3. **An acquisition is held, or refused** (`affine::acquisitions`). From
   each call whose row declares `resource.acquire<T>`, the check walks up
   through what passes a value on, a block's last statement, a branch, an
   arm, `?`, `Ok(..)`, `Err(..)` and `Some(..)`, to what holds it:
   - **a binding to a name**, `let x = ..` or `use x = ..`, which PW2005
     follows as it did; now also `let h = Maps.create(..)?`, and a binding
     whose value is an `if`'s or a `match`'s;
   - **a call that releases `T`**, given it as an argument or the
     receiver: ended where it is made, `Database.begin().commit()`;
   - **the declaration's caller**, as its body's value or a `return`'s,
     where the declaration gives one (4);
   - **a resource's `acquire` clause**, whose value the resource holds and
     its `release` clause ends.

   Anywhere else it is PW2005, saying what drops it: `_`, "bound to `_`,
   and nothing can release it", the ruling's case; a statement; a call
   that does not release it; a `match` on it, whose arms' bindings are not
   followed; a function value's result; another value, a list or a record.
   The repair: bind it to a name, with `?` where a `Result` carries it, and
   end it.
4. **A declaration gives its caller its body's value only where it
   declares one** (`affine::gives_its_value`): a function, a query, a
   command or a task whose result is not `()`. A `return` in a function
   value leaves the function value.
5. **The inference types `e?`, a block, and branches or arms that agree**
   (`infer::Types::of`), so a binding the check now follows names its
   members: `h.destroy()` after `let h = Maps.create(..)?`.
6. **The repair names only what takes a `T` to end it**
   (`affine::release_functions`), as `released_parameters` reads a row.
7. **A discard by name stays**: `let _voted = upvote(..)` names what it
   drops. The corpus keeps its eighteen, among them A-014's handler, whose
   content address is its text.

## Acceptance

- **`compiler/pw-core/tests/let_discard.rs`, 11 tests**, each with its
  control: a `Result` discarded, and dropped (PW0618 and its repair); an
  annotation still held; `_` read; a transaction bound to `_`, and `let _ =
  tx` moving nothing, as in Rust; a statement, an `if` statement's
  branches and `Some(..)`, and the same branches bound and ended; a call
  that does not release it, and one that does, as an argument and as the
  receiver; a list, and a function value's body and `return`; what a
  declaration gives its caller, four ways and through `Ok(..)`, and a body
  declaring `()`; `let h = Maps.create(..)?` never destroyed and
  destroyed, `let tx = open()?` ended as a member, a `match` on the
  acquisition, whose repair names `?` and only `destroy`, and `let _ =
  ..?`; a binding from branches and from arms, ended as a member; a
  component's `resource` statement, its `acquire` clause holding the
  handle and its `release` clause dropping one.
- **`compiler/pw-syntax`, 2 tests**: `let _` parses; `let mut _` and `use
  _` are each one error, where written.
- **`koka_backend.rs`**: `let _ = e` is `val _ = e`. **`handlers.rs`**: a
  handler discarding its command's answer with `_` sends the command.
- **Eight generality witnesses** (`examples/generality/
  affine_not_consumed_once`, its DIMENSIONS.md): six caught that passed
  before, and two neighbours that stay clean.
- **The whole workspace's tests**, the corpus among them: nothing else
  moved.
- **`scripts/let_discard_mutations.py`: 29 mutants**, recorded by `just
  e14-let-discard`. Its first run left one, "a `return`'s value is not the
  body's": each `return` the tests wrote was the body's last statement too,
  given to the caller either way. One inside a branch holds it, and 29 of 29
  are killed.
- **The chain runs on the push** (ADR-0245), and with it the eight scripts
  with a mutant within thirty lines of a change, each run whole: two of
  PW2005's, and those of command answers, function values, handler
  failures, lexical scope, named handlers and separated statements. PW2005's
  third, `affine_loops_mutations.py`, has none so near, and was run whole
  here at `f32d114`: 3 of 3 killed. (Corrected 2026-10-07: this said nine,
  "the three of PW2005", and the push's plan holds eight.)

## Found

Writing the tests found two more, each the next ADR:

- **A `resource` declaration's clauses are walked by no affine rule.** Its
  `acquire` and `release` clauses are terms, lowered outside the body PW2005
  walks (`lower::policy_terms`), as a policy's are: an acquisition in a
  `release` clause, or a `release` that never ends what it is given, passes.
  The same clauses as a component's `resource` statement are walked.
- **A member names whichever declaration registered last.** A member is
  keyed by its receiver's type and its name alone (`signatures::by_member`),
  so `h.destroy()` in a module importing only `Maps` named
  `VendorSdk.destroy`, which releases nothing, and not `Maps.destroy`.

## Not claimed

- **A resource matched where it is made**, `match Maps.create(..) { Ok(h)
  => Maps.destroy(h), .. }`, is refused, though its arm may end it; so is
  `let r = Maps.create(..)` then `match r`, as before. What an arm binds is
  not followed. `let h = Maps.create(..)?` is the form followed.
- **A resource kept in a value**, `let o = Some(Database.begin())`, is
  followed as `o`, which nothing releases: refused, not followed into the
  case.
- **`use _`**: a `use` holding a value it never names, to its block's end,
  is refused rather than admitted.
- **An acquisition in a keyword's block**, `unsafe.imperative {
  Maps.create(..) }`, is refused unless a name inside the block holds it:
  what a keyword's block gives the keyword is ruled nowhere, and is not
  guessed here.
- **A `resource` declaration's clauses, and a member two modules declare**
  (Found, above): the next ADRs, the first ADR-0251.
- **A borrow**: ADR-0045's "Not claimed" stands.

# ADR-0064: a label carried through a call

Status: accepted under the owner's instruction of 2026-09-26 ("keep working
until its perfect"). Decisions marked **(ruling needed)** were made without a
ruling and are offered for reversal. Date: 2026-09-26. Milestone: E10
(charter §7.8, privacy labels on values).

## Context

ADR-0063 found that a declared function laundered a label. `labels.rs`
labelled a call to a declared function by the declaration's label alone: "a
declaration's label is its contract", a rule from E2C. So each of these
passed `pw check`, logging a `Secret<Payments>` publicly (PW5006 when
written directly):
- `List.get(tokens, 0)`, over a list of secrets;
- `List.map(tokens, t => "with {t}")`'s result;
- `List.fold(tokens, "", (acc, t) => "{acc}{t}")`;
- a program's own `fn first<T>(xs: List<T>) -> Option<T>`;
- a public list mapped through a lambda that reads a secret.

None of those declarations says anything about labels. The rule read their
silence as "public", and that is what made them launder.

Writing the tests found one more. A call that resolves to nothing, such as
`tokens.get(0)` where nothing typed `tokens`, is labelled by what goes into
it, and its receiver was left out, so `tokens.get(0)` was public too.

## Decision

### 1. A result made of its arguments carries their labels

A declared call's result is labelled by the declaration's label, joined with
the label of each argument whose declared type mentions a type parameter the
result mentions:
- `get<T>(items: List<T>, index: Int) -> Option<T>` carries `items`' label,
  not `index`'s;
- `map<T, U>(items: List<T>, f: fn(T) -> U) -> List<U>` carries `f`'s;
- `length<T>(items: List<T>) -> Int` mentions no parameter in its result, and
  keeps its contract, so the length of a list of secrets is public.

**Corrected by ADR-0085:** a function over plain values mentions no type
parameter either, so `String.trim` laundered a secret string. A declared
call carries every argument given to a parameter that states no label now,
and the length of a list of secrets carries their label.

A join never loses a restriction, so a declared label is never weakened.
**(ruling needed)**: the alternative is label polymorphism in the
signatures, where a declaration states how each result's label is made.

### 2. What each parameter is given

The parameters are given, in order:
- a piped value, for `xs |> List.map(f)`;
- a method call's receiver, for `tokens.get(0)`;
- then the arguments.

### 3. A function is labelled by what it computes

A lambda's label is its body's, so `List.map`'s `f` brings in what the
function makes from its captures and its parameters. Its parameters carry
the call's other arguments' labels, per ADR-0063.

### 4. An undeclared call's receiver goes into it

A call no declaration answers carries its arguments' labels, as before, and
now its receiver's and a piped value's too.

## Acceptance

- **`compiler/pw-core/tests/labels_through_calls.rs`**, 7 tests:
  - `List.get`;
  - `List.map`, over secrets and through a lambda reading one;
  - `List.fold`;
  - a program's own generic function;
  - a method call, typed and untyped;
  - a piped argument;
  - the control that `List.length`'s result keeps its contract.

  Each has a control over a public list. All but the last fail at 3987866,
  the commit before.
- **No rejected, rule or generality fixture's diagnostics change.** The
  store, kiokun and the accepted corpus check clean.
- **The store's and kiokun's artifacts are byte-identical,** apart from
  ADR-0058's two handlers.
- **Mutation controls:** `scripts/label_call_mutations.py`, `just e10-labels`,
  7 mutants.

## Not done

- **An implicit flow.** `List.get(words, i)` with a secret `i` is public: a
  label is a value's, not the control flow's (charter §7.8).
- **A declaration that states how a result's label is made.** There is none;
  §1's rule stands in for it.

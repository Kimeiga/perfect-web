# E9-V1..V6: the ordinary value relations, 2026-09-24

Raw output: [value-relations.txt](value-relations.txt), produced by
`just e9-values`. Decision record: [ADR-0031](../../DECISIONS/ADR-0031-value-relations.md).
Environment: Darwin arm64 (Apple M2 Pro), rust 1.97.1, locked registry.

## Claim

Each E9-V closing gate is met by a check that **refuses** programs, not by a
representation that could express one:

| gate | refused (and its accepted neighbour) | mutation that must fail it |
|---|---|---|
| V1 arity + arguments | `takes_str(42)`; `takes_store(makes_cart())`; member, piped, query, construction, policy-term calls | primitives always agree; arity never compared |
| V2 generic calls | `identity(s)` where `s: Store` returning `Cart`; `pair(1, "x")`; `List.map` typed through its callback | no instantiation; bound variables not followed |
| V3 nominal identity | `alpha.Tag` where `beta.Tag` is required; `Tag` and `String` in either direction | nominal comparison ignores the declaration |
| V4 results | `fn wrong_return() -> String { 42 }`; every branch, arm, `return` and `?` | no result sites; `?` returns nothing |
| V5 unresolved types | `fn f(x: Stroe)` reported once; no relation runs on it; `Box` / `Box<Int, Int>` for `Box<T>` | unresolved accepted; declared arity unchecked |
| V6 ABI-equal types | `OrderId` where `StoreId` is required (both `string` on the wire); `Money<EUR>` where `Money<USD>` is required | nominal types compared by representation |

`compiler/pw-core/tests/value_relations.rs`: 38 tests. Mutation controls: 10 of
10 mutants killed, each by failing tests and none by a build failure.

## Decided, not merely silent

A checker that never runs is silent too. `pw audit-values` counts what was
decided:

```text
store program (38 files)          accepted corpus (60 files)
Arity       agree        17       Arity       agree        46
Argument    agree        16       Argument    agree        56    undecided 11
Field       agree        11       Field       agree        17    undecided  1
Return      agree        25       Return      agree        25    undecided 54
            undecided    41       Binding                         undecided  3
Annotation  agree       303       Annotation  agree       343
```

Every relation written in `examples/store/app.pw` agrees, including
`add_to_cart`'s call to `Carts.add` and the optimistic transition's
`Carts.with_line(cart, item, quantity)`. `the_store_program_is_decided_not_merely_silent`
holds that with a floor.

The undecided returns are library stubs whose body is `todo`, whose value no
declaration states. The undecided arguments are values whose type nothing
states: a lambda with no expected function type, `4.px`, or a binding from
`measure { .. }`. They are counted, not reported, and never read as
agreement.

## What the first run found

These are defects, not test adjustments. Each was silent before 2026-09-24.

**The milestone demo (examples/store):**
- `query Menu` declared `List<MenuItem>` and returned `Menus.for_store`'s
  `List<MenuItemId>`. The page renders `item.name`, so the library was wrong.
  The WIT for `store:data/menus#for-store` changed accordingly.
- `Carts.add/current/clear` and `query Cart` took `SessionId` and were given
  `current_session()`'s `Session<SessionId>`. They now declare the label
  (ADR-0031 §7, ruling needed).

**The language and its compiler:**
- `?` had no HIR node, so `let x = f()?` bound a `Result`.
- `type Box<T>` parsed and discarded `<T>`.
- No callable could be generic, and no callback could be typed.
- Written types that named nothing (`LatLng`, `Stream`, `LayoutSnapshot`,
  `PaymentReceipt`, `InventoryRow`, `BadgeId`) were accepted without a word.
- `Money<USD>` was written against a non-generic `Money`, and the argument was
  dropped.
- `import domain` beside `import domain.{ X }` made `X` ambiguous with itself.
- `TryExpr` initially missed `lower.rs`'s `is_expr` list, which silently dropped
  every `let x = f()?` initialiser. Found by the checker's own tests and now
  guarded by `every_expression_kind_is_lowered_as_an_expression`.

**The accepted corpus and its libraries:**
- A-012 propagated a `StoreError` with `?` out of a function failing with
  `DecodeError`, and passed an undecoded `decode.Unknown` where a `String` was
  required. `decode.field` now returns what its decoder produces.
- A-010 passed `Money<USD>` and `Secret<Payments>` to `Payments.capture(key:
  Int, .., amount: Int)`.
- A-019 and A-024 passed a component's `self` (`ElementRef`) to map APIs
  declared over `Element`.
- A-016 summed `List<LayoutSnapshot<Float>>` with an `Int` `List.sum`.
- A-022 and R-042 passed a `StoreId` to a telemetry parameter declared
  `String`.

**The rejected corpus and witnesses:**
- 25 rejected fixtures, 55 generality witnesses and 7 rule fixtures named domain
  types they never imported.
- Several passed `Int` amounts to `Money<USD>` APIs, or declared results the
  called library did not produce.
- `log.public(message: String)` made logging a `Secret` a type error as well
  as the privacy violation the witness exists for. Sinks are now generic.
- `Database.rollback` returned `()` where `commit` returned
  `Result<(), CartError>`, so every "rollback on one branch, commit on the
  other" body had two types.

All fixture repairs are listed in `docs/CORPUS.md` §C8, with their pre-change
text in `examples/history/C8/`. Every rejected fixture is still rejected for
exactly its declared rule (`every_rejected_fixture_emits_only_its_own_defect`).

## Reproduce

```sh
just e9-values
cargo test --locked -p pw-core --test value_relations
python3 scripts/e9_value_mutations.py
just ci
```

## Not claimed

- **Member existence.** `box.x` on a `Rect` with no `x` is unknown, not
  refused. It is the next relation and is not one of V1..V6.
- **Sum-type variant constructors.** `Circle(3)` has no type, because variant
  names are not resolved by the workspace.
- **Named-argument calls.** These are Undecided, because signatures do not
  carry parameter names. None exists in the corpus.
- **Branch agreement in statement position.** `if c { 1 } else { "s" }` as a
  statement is not refused. As a result it is.
- **Specialization of non-phantom generic layouts at the boundary.**
- **E10-I**, which consumes these checked signatures and is not affected by
  this change beyond needing them.

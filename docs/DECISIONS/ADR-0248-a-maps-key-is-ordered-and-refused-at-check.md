# ADR-0248: a map's key is an `Int`, a `String`, a `Bool`, or an opaque type over one, refused at check where it is not

Status: accepted under the owner's delegation of 2026-10-02; it builds the
owner's ruling 0057-a (ADR-0210). Date: 2026-10-07. Milestone: E14, the
owner's Twitter list, item 2.

## Context

- **ADR-0057 keyed a map by an `Int` or a `String`** (ruling needed): no
  other type had an order the component and the browser's module shared.
- **Another key was refused only by the backend.** `fn f(m: Map<Float,
  Int>)` checked; `pw build` refused it. So `pw check` passed programs
  `pw build` could not build, and an agent learnt of the defect a step
  late.
- **`Map<ProductId, V>` could not be written.** A program's own key types
  are opaque (`ProductId`, `Sku`), and the backend refused every nominal
  key, though an opaque type crosses and compares as its representation
  (ADR-0033 §4, ADR-0228).
- **Ruling 0057-a** (ADR-0210): "A map key may be an `Int`, `String`, `Bool`
  (`false < true`), or an opaque type over one, ordered as its
  representation. Others are refused at **check**, not only at build."

## Decision

1. **A key is an `Int`, a `String`, a `Bool`, or an opaque type over one**,
   in the checker (`values::map_key`) and the backend
   (`backend::lower::ordered_key`): an `Int` by value, a `String` by code
   point, `false` before `true`, and an opaque type as its representation.
   The component orders a `Bool` as its `i32`, the browser's module as
   JavaScript's booleans, the same way.
2. **Another is refused at check, PW0627 `map_key`**: "… keys a map or a
   set by `Float`, and a key is an `Int`, a `String`, a `Bool`, or an
   opaque type over one". Refused
   - where a type is written (a parameter, a result, a field, a binding's
     annotation, a cast, a lambda's parameter), at the annotation, and
     inside another type (`List<Set<List<Int>>>`);
   - where a call instantiates a generic map or set, at the call:
     `Set.from_list([1.5])`, which no annotation wrote.

   A type parameter's key is not refused where it is written; each call
   that instantiates it is held to it.
3. **The backend keeps its own guard**, for a program that reaches it
   unchecked.

## Acceptance

- **`compiler/pw-core/tests/map_keys.rs`, 5 tests**, each with its control:
  a written `Float` key; a `Bool` and an opaque type over a `String` are
  keys, and an opaque type over a `Float` is not; a key inside another
  type, and in a binding's annotation; a key refused once, where it is
  written, and not again at each call to a function that returns it; a key
  a call instantiates, and a generic function's own.
- **`compiler/pw-conformance/tests/maps.rs`**: a map keyed by a `Bool` and
  sets of `Bool`s and of opaque types over a `Bool`, an `Int` and a
  `String`, through the host, against Rust's `BTreeMap` and `BTreeSet`; a
  `Bool` map out of order, or with a key twice, stops the invocation.
- **`compiler/pw-conformance/tests/javascript.rs`**: four queries keyed by a
  `Bool` and by opaque types, the browser's module against the component
  under Node: 91 queries, 18,200 calls, agreeing.
- **`scripts/map_keys_mutations.py`: 11 mutants**, recorded by `just
  e14-map-keys`. Its first run left one, "every call's result is taken for
  a generic map's": nothing held a call to a function whose own result is
  refused to say nothing more. The fifth test holds it, and 11 of 11 are
  killed.
- **The chain runs on the push** (ADR-0245).

## Not claimed

- **Ruling 0057-c**, a map or set arriving out of order sorted rather than
  refused. Next; built by ADR-0259.
- **A record, a list or a sum type as a key.** None has an order the
  component and the module share, and the ruling admits none.

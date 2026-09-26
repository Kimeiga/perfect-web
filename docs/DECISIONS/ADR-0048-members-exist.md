# ADR-0048: a read names a member its value's type has

Status: accepted under the owner's instruction of 2026-09-25 ("ok fix all the
known gaps"). Decisions marked **(ruling needed)** were made without a ruling
and are offered for reversal. Date: 2026-09-25. Milestone: E10 (a new E9
value relation; charter §7.1).

## Context

KNOWN_LIMITATIONS listed member existence as the next value relation after
E9-V: "`box.x` on a `Rect` with no `x`, or `{row.status}` on a type without
it, is unknown rather than refused." The value relations typed such a read as
unknown, and every analysis after them read nothing. A-015 was the named
example.

## Decision

### 1. The member relation (PW0610)

`value.name`, read or called, is related to the members of the value's type:
- the type's record fields;
- every declaration whose first parameter takes the type. This is how
  `Signatures` already resolves `snapshot.value` and `self.style`: read as a
  property when it takes only the receiver, called as a method otherwise.

When the type is known and has no such member, the read is refused:
"`m.Point` has no member `z`". When the type is not known, the relation is
undecided, and the audit counts it.

Two reads are not members:
- A path through a module, like `List.map`, is a name, and ADR-0047 resolves
  it.
- A unit on a numeric literal, like `900.px` or `30.seconds`, is a
  dimensioned literal.

### 2. An opaque type's representation is `.value`, in its own module

`opaque type PositiveInt = Int` had no way to read its `Int`. A-001 wrote
`quantity.value` from another module, which is the opacity the type exists
to give.
- In the module that declares the type, `.value` is the representation, typed
  under the receiver's arguments.
- Anywhere else it is refused (PW0610, "opaque here"). The module declares
  what other modules may read.
- A declared member named `value` takes precedence. `LayoutSnapshot<T>`'s
  `value` gives its `T` everywhere.

**(ruling needed)**: `.value` is the spelling, not a constructor pattern
(`match q { PositiveInt(n) => n }`); and the boundary is the declaring
module.

### 3. An optional's contents are PW0600's

A member that an `Option` or `Result` lacks but its `Some` or `Ok` side has
is reported as PW0600, "an optional used as its value". That case already had
a rule, which saw only a `let`-bound `Option`. The member relation also sees:
- a parameter;
- a call's result;
- a `Result`.

Where both rules reach one defect, the checker keeps one diagnostic per code
and span. The more specific rule, whose wording names the `let`, runs first.

### 4. What the relation found

Eight reads of members no type has, in code that checked clean:

| where | read | now |
|---|---|---|
| the store's page | `{cart.line_count}`; `Cart` has only `lines` | `domain` declares `line_count(cart: Cart)`, the quantity across the lines, which the dev server was computing |
| A-001 | `quantity.value` on another module's opaque `PositiveInt` | `domain` declares `count(n: PositiveInt)` |
| A-015 | `anchor.bounds()`, `box.x`, `box.bottom` on a `LayoutSnapshot<Rect>`, and `self.style.transform = ..` | `getBoundingClientRect()`, `box.value.left`/`.top`/`.height`, and `set_transform(..)`, all declared by the platform |
| A-016 | `it.width()` on an `ElementRef` | `it.offsetWidth()` |
| A-018 | `{item.name}` over a `List<MenuItemId>` | `List<MenuItem>`, keyed by `item.id` |

Rejected fixtures read absent members beside the defect each is about. Each
now fails only for its own reason:
- R-001 and R-018 read absent members;
- R-010 and R-035 call absent members;
- R-030 and two witnesses called `checkout(cart.id)`;
- `interpolation-hole.pw` read a `Secret`'s `.value` outside `capability`.

A-015's claim, "a measure is scheduled before a mutate", is witnessed now:
- evidence reachability moves it from Missing to Witnessed;
- its phases call declared operations, whose rows the effect analysis reads.

## Acceptance

- `compiler/pw-core/tests/members.rs`: 11 tests, each with its control.
- The accepted corpus, the store and kiokun check clean. Every other fixture
  reports what it did before this change, compared against a build of
  `d45fc23`.
- Mutation controls: `scripts/member_mutations.py`, `just e10-members`, 10
  mutants.

## Not done

- **An opaque value is not built or read inside compiled code.** The Wasm
  encoder refuses to construct one (`PositiveInt(1)`), and so refuses
  `.value`. They cross the component boundary as their representations. The
  backend work of the same instruction takes this up.

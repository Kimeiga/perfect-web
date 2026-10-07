# ADR-0254: a member names the declaration its module sees

Status: accepted under the owner's delegation of 2026-10-02; a correction
ADR-0250's tests found. Date: 2026-10-07. Milestone: E14.

## Context

- **A member is its receiver type's field, or a term whose first parameter
  takes the type** (ADR-0048): `h.destroy()` is a call to a `destroy(h:
  MapHandle)`. The table of them was keyed by the receiver's type and the
  member's name (`signatures::by_member`), one declaration per key.
- **A second declaration replaced the first.** The store's library declares
  four members twice or more: `destroy` on a `MapHandle` (`Maps`,
  `VendorSdk`), `is_available` on a `MenuItemId` (`Menu`, `Menus`),
  `current` on a `Session` (`Carts`, `Estimates`, `Orders`) and `for_store`
  on a `StoreId` (`Menus`, `Recommender`). Each named whichever was
  registered last, whatever the module calling it imported: `h.destroy()`
  in a module importing only `Maps` was `VendorSdk.destroy`, which releases
  nothing, and every check read that one's row, result and label.
- **Its own test said otherwise.** `signatures.rs`'s
  `a_member_resolves_by_receiver_type_and_ambiguity_resolves_to_nothing`
  held two receivers apart, and never two declarations on one.

## Decision

1. **Every declaration of a member is kept** (`by_member` holds them all).
2. **One resolves as it did**: a member with one declaration is that one,
   in any module.
3. **Of several, the one the calling module sees** (`Signatures::sees`): a
   field, which is its type's; a term declared beside the receiver's type;
   or one declared in the module, or in a module it imports, whole or by
   the term's name, as a name resolves (PW0021). Each check asks as the
   module its body is in: the value relations, the inference, the labels,
   the effects, and the backend.
4. **A module that sees several, or none, is refused**: PW0628
   `ambiguous_member`, "`destroy` of a `browser.MapHandle` could be
   `Maps.destroy` or `VendorSdk.destroy`, and this module does not say
   which", whose repair is the path, `Maps.destroy(..)`, or importing one.
   No check chooses one meanwhile, and the affine check reads it as no
   function value.

## Acceptance

- **`compiler/pw-core/tests/member_resolution.rs`, 7 tests**, each with its
  control: `destroy` imported from `VendorSdk` and from `Maps`, which
  releases what a row must declare; from both, and from neither, PW0628;
  `is_available`'s effects, pure from `Menu` and a read from `Menus`;
  `current`'s type, a cart from `Carts` and an order's status from
  `Orders`; a member declared beside its type, seen with no import of its
  module; and an ambiguous member, which the affine check reads as no
  function value.
- **The generality witness `held-behind-try-and-destroyed.pw`** destroys its
  handle as a member again, `handle.destroy()`, the `destroy` it imports.
- **The whole workspace's tests**, the corpus among them. Its first run
  found the lookup taking a member's name from its path's last segment,
  which `last_segment.rs` forbids; it is given the name it looks up.
- **`scripts/member_resolution_mutations.py`: 9 mutants**, recorded by
  `just e14-member-resolution`: 9 of 9 killed. `member_mutations.py`'s two
  mutants on the line this changed are re-anchored, and the script, run
  whole, kills 10 of 10.
- **The chain runs on the push** (ADR-0245).

## Not claimed

- **A member with one declaration needs no import**: a function of another
  module taking the type is its member wherever the type is. Requiring the
  import, as a trait method is in Rust, would refuse programs that check
  today; it waits for a ruling.
- **A page's planned reads** (`page_values::steps`) ask as no module: the
  one declaration, or the type's own.

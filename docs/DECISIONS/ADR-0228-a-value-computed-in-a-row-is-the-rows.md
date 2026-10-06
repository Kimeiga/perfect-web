# ADR-0228: a value computed in a row is the row's

Status: accepted under the owner's delegation of 2026-10-02. It is ruling
0073-a in a loop's row and from a speculated value (ADR-0210), which the feed
reference app's likes need (NEXT 24). Date: 2026-10-05. Milestone: E14.

## Context

- **ADR-0226 and ADR-0227 compiled a value a template computes** at the top
  of a page, from one query's value or one signal's. One in a loop's row was
  refused at build. So was one from a value the page speculates on.
- **The feed needs both.**
  - The timeline says each post's likes in words, "2 likes", in the row of
    `{#each feed as p (p.id)}`.
  - A like should show before the server answers, as a post does (ADR-0222).
    The row's count then moves with the speculated timeline.

## Research

- **A row's member read is computed for each row already** (ADR-0169). A
  host runs the member with the row's item and sets the result in the row,
  whole, by its path from the item (ADR-0170): `item.price.display`, inside a
  loop inside a loop too (ADR-0181). The speculation module does the same
  for each row it renders from a speculated value (ADR-0172).
- **A value computed from the row's item is the same thing.** Its function
  takes the item, or what the steps from the item reach, and its value is
  the row's.

## Decision

1. **A text hole, or an attribute's whole value, in a loop's row may compute
   from the row's item**, or from what a view in the row is given of it.
   - The compiler names its path from the innermost loop's item,
     `p.#feed.app.Home~10`, when the item is all it reads. Each row holds its
     own value, as a member read of the item does.
   - The page's plan reads it as a row read: the collection, the item's name,
     that path, and the steps from the item to what the expression reads,
     then its function: `{"derived": "feed.app.Home.derived_10"}`.
   - A host computes it for each row it renders: the page, a list's patch, a
     keyed row's again. Inside a block in the row, and in a loop in the row
     through each outer item (`posts.*.tags`), as well.
2. **The speculation module computes it for each row it renders** from a
   speculated value, from the row's item whole: `rows: {
   "#feed.app.Home~10": f2 }`.
3. **A text part computed from a speculated value whole**, at the top of the
   page, is computed again by the speculation module, as a path part is. The
   count moves with the list it counts.
4. **The feed's rows say their likes in words, and a like is optimistic.**
   - `<span class="likes">{counted(p.likes, "like", "likes")}</span>` beside
     a "Like" button.
   - `like` declares `optimistic Timeline(current_session(), _) as feed =>
     liked(feed, post)`, which counts one more like on the post.
   - Pressed, the row says "3 likes" before the server answers. A like whose
     request fails is taken back.
5. **Still refused at build, by name** (ADR-0229):
   - a row's value from the item and another value;
   - one from a field of a row the page speculates on, which the module
     computes from the item whole;
   - an attribute computed from a speculated value;
   - one in a block or an arm outside a row, in a view that contains
     itself, or in a block a signal decides.

## Found

- **A field of a value was charged a same-named function's effects.** The
  feed's `liked` reads `i.author`, a field of an `Item`. The module also
  declares `fn author(s) -> UserId !{ database.read<User> }`.
  - ADR-0078's rule for a declaration named as a value matched a path's last
    segment against the declarations in scope. So the transition "performed"
    `database.read<User>`, and was refused (PW0330).
  - A field of a value in scope is the value's now, which the member rule
    reads by its type. Only a module's path names a declaration.
- **An opaque value did not compare.** `i.id == post`, two `PostId`s,
  checked and did not build: "`Eq` on a Nominal".
  - An opaque value is its representation, retyped (ADR-0054). A comparison
    retypes both back, and compares those: an `Int`, a `Float`, a `Bool` or
    a `String`.
  - In a component a host runs, and in the browser's code.

## Acceptance

- **`compiler/pw-core/tests/computed_rows.rs`, 3 tests:**
  - the feed's likes in words: one row read, from `p`, by its function; read
    by the template at that path; computed by the speculation module for each
    row; `like` speculates with `post`;
  - a view in the row given a field of the item, a block in the row, and a
    loop in the row, `posts.*.tags`;
  - a row's value from the item and the list, a speculated row's from a
    field of its item, and an attribute from a speculated value, refused by
    name.
- **ADR-0226's `computed_holes.rs`**:
  - `{List.length(feed)} posts` on the home page builds, computed again by
    the speculation module, where it was refused;
  - its refusals name ADR-0229.

  ADR-0227's `computed_signals.rs` says the same. ADR-0222's
  `optimistic_keys.rs` finds `like` beside `post`.
- **ADR-0078's `effects_through_values.rs`**: `a_field_of_a_value_is_no_function_of_the_same_name`.
  A field, in a body and in a callback, is not charged `fn stamp`'s
  `clock.read`. The function named as a value, and a module's path, still
  are.
- **The development server's tests:**
  - `a_value_computed_in_a_row_is_each_rows_and_sent_with_it`: "2 likes" in
    the row, and "3 likes" sent with the row after another session's like;
  - `an_opaque_value_compares_as_its_representation`: `thread.id ==
    PostId("p1")` in a component a host runs, for two threads.
- **`e2e/feed.spec.mjs`, in three engines:**
  - `a like shows before the server answers, and one that fails is taken
    back`;
  - every test that counted a like reads the row's count in words.
- **`scripts/computed_rows_mutations.py`: 12 mutants**, 10 against the
  compiler's and the server's tests and 2 against the feed in three engines,
  recorded by `just e14-computed-rows`.
  - Re-anchored: `cart_lines_mutations.py`'s and
    `optimistic_transitions_mutations.py`'s, which ADR-0228's code repeats
    deeper, and three of `computed_holes_mutations.py`'s and one of
    `computed_signals_mutations.py`'s.
  - Retired: `computed_holes_mutations.py`'s "a value computed from a
    speculated one is built". The speculation computes it now, and "an
    attribute computed from a speculated value is built" stands for what is
    still refused.
- **The workspace, 2,086 tests; the browser suite, 767 in three engines.**

## Not claimed

- **Where else, and from what else** (ADR-0229): a value from several
  values, or a row's from its item and another; one in a block or an arm
  outside a row, in an instance, or in a block a signal decides; a
  condition, `{#if n > 0}`; one from a page's parameter; a hole in an
  attribute's text.
- **Speculated, a value from a field of a row's item, and an attribute's.**

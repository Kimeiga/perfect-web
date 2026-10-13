# ADR-0324: `private` says a declaration is not importable, and nothing more

Status: accepted under the owner's delegation of 2026-10-02 (the
integrator's). Date: 2026-10-12. Milestone: E14 (the language). Takes NEXT
3a, W8's finding of 2026-10-09, which ADR-0318 left the integrator's.
Builds on ADR-0270 (the reader's user), ADR-0318 (`user`, the user's
visibility) and the corpus' resolution rules (PW0023).

## Context

- **`private` meant two things.** A declaration that said it was the
  user's: `check.rs`'s `label_of`, the boundary's and the resume manifest's
  restriction, the manifest's privacy. And it was not importable:
  `resolve.rs`, PW0023.
- **ADR-0318 gave the first a word of its own**, `user`: the user's, and
  importable as `session` is. Whether `private` stayed the user's was left
  here, since it changes every program that says it.
- **The two meanings did not compose.**
  - A module's own helper that said `private` to stay its own was the
    user's data wherever it was read. A public page computing its title
    through one was refused for keeping the user's data in a shared cache
    (PW5001; this ruling's test).
  - PW0023's advice was "make it public", and `public` says whose data a
    declaration holds: a user's query made importable that way would be
    everyone's.
  - A non-importable session or public query could not be said at all.
- **Every other language reads `private` as access.** Rust's `pub`, Java's
  and Kotlin's `private`, Swift's `private`: a declaration's visibility to
  other code, and nothing of its data.
- **What the programs wrote**: five of the feed's queries (notifications
  and messages, each the user's), a generality case's, R-064's, and one
  `private type` in the resolution rules' fixtures, the only one meaning
  "not importable" alone.

## Decision

1. **`private` says a declaration is not importable.** Nothing reads it as
   the user's any more: the user's scope is `user`'s alone, in the checker's
   labels, the boundary, the resume manifest and the manifest's privacy.
2. **A declaration that holds data says whose** with `user`, `session` or
   `public`: a query, a page, a layout, a command, a subscription, a
   resource and a materialization. `private` on one is refused by name,
   PW0029, "the query `Mine` is `private`, which says it is not importable,
   and nothing of whose data it holds", with "write `user` for the user's,
   `session` for the session's, or `public` for everyone's". A program
   written when `private` meant the user's is so never read as everyone's
   data.
3. **PW0023 says "drop `private` from it"**, not "make it public".
4. **The programs say `user`**: the feed's five queries, the generality
   case and R-064, whose case (a record carried into the public shell by a
   view's parameter) is unchanged.
5. **Found on the way: `user` after a declaration with no body** was that
   declaration's. `fn owner(..) host ".."` followed by `user query` read
   `user` as the function's unknown policy (PW0005), and then as its `host`
   clause's value: ADR-0318's contextual keyword was tested before a
   declaration's keyword alone. The feed's queries reached it once they
   said `user`. A `user` that opens a declaration ends a clause and is no
   policy.

## Alternatives

- **Keep `private` the user's, and not importable.** Rejected: the double
  meaning is the defect W8 found, a helper's privacy taints what reads it,
  and "not importable" cannot be said of anything else.
- **Make `private` the user's, importable, and add a word for "not
  importable".** Rejected: `private` would mean in this language what it
  means in no other, and `user` says the same.
- **Re-mean `private` silently.** Rejected: a query a program still writes
  `private` would become public data, a privacy defect no check would name.
- **Both words on one declaration** (`private user query`). Not taken here:
  nothing asks for a non-importable user's query yet.

## Acceptance

- `compiler/pw-core/tests/private_is_not_importable.rs`: a public page shows
  what its module's `private` function computes, unrefused, which the old
  label refused (PW5001); another module does not import it, and is told to
  drop `private`; `private` refused on a query, a page, a layout and a
  command, by name, and `user`, `session` and `public` accepted.
- `user_visibility.rs`, `boundary_matrix.rs` and `principal.rs` say `user`
  where they meant the user's; `pw-syntax`'s
  `a_user_declaration_after_one_with_no_body_is_its_own`.
- The feed, the store and every program build as before (`just ci`).
- `scripts/private_module_mutations.py`, each mutant killed.

## Not claimed

- **A non-importable declaration that holds data**: refused, rather than
  written with two words.
- **Programs outside this repository** that say `private query`: each is
  refused with PW0029 and its repair, not migrated.

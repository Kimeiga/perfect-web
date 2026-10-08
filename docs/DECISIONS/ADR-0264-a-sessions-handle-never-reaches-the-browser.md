# ADR-0264: a session's handle never reaches the browser

Status: accepted under the owner's delegation of 2026-10-02; a soundness
defect found writing ADR-0263. Date: 2026-10-08. Milestone: E14.

## Context

- **A session's id is its cookie's value**, and the cookie is `HttpOnly`
  (ADR-0258): the page's scripts never read it, so a script a page runs
  cannot take the session away. A session's handle, `Session<SessionId>`,
  is that id: `current_session()` answers it.
- **A page could print it.** A query answering `Seen { session, .. }`,
  read by a page that printed `{mine.session}`, checked clean, and its
  page carried the session's cookie for any script to read.
- **What reaches the browser**: what markup prints, as text or as an
  attribute's value, a string's holes among it; a view's props, which a
  page's browser renders again (ADR-0203); and an answer: a query's or a
  subscription's that a page shows, speculates on or computes from in the
  browser (ADR-0222, ADR-0227), and a command's.
- **`Session<SessionId>` was also a way to say a value is session-scoped.**
  `capability.pw` declared the labels as types a signature can return, and
  a corpus witness wrote `fn summarize(c: Cart) -> Session<SessionId>` and
  printed it. Since ADR-0263 nothing but the platform makes a `Session`
  value, so every one is the session's id.

## Decision

1. **A session's handle never reaches the browser.** A query's, a
   subscription's or a command's answer whose type holds one, wherever it
   sits in the type, is PW5040; so is anything markup prints that holds one:
   a hole, an attribute's value, a hole in an attribute's string, and a
   view's prop. A handler, `on:..`, prints nothing; what it captures is
   held by PW5007, as before.
2. **A user's or an organization's id may reach it.** Each is a name, and
   the browser holding it opens nothing; the handle's power is on the
   server, where it keys what a query reads (ADR-0263).
3. **A value's scope is where it came from** (ADR-0129), never a wrapper:
   the corpus's `private_in_shared_cache/via-helper.pw` returns a `String`
   computed from the cart, whose label follows it through the call, and is
   still caught for what it is about.

## Acceptance

- **`compiler/pw-core/tests/session_to_browser.rs`, 3 tests**, each with
  its controls: an answer that is a session's handle, holds one in a record
  and in an `Option`, a subscription's and a command's, beside the session's
  data answered and a user's handle; a hole, an attribute's value and a
  hole in a string, beside the session's data printed and a user's id; and
  a view's prop.
- **`scripts/session_mutations.py`, 8 mutants**: a query's and a command's
  answer unchecked, a hole, an attribute's value and a string's holes
  unchecked, and a handle not found in an argument, a record's field, or as
  what is printed. Recorded by `just e14-session-to-browser`. ADR-0263's
  `handle_mutations.py` is run whole again: its type walk now reads a type's
  parts where this ADR's does, `declared_parts`, and one of its mutants is
  re-anchored there.
- **The whole workspace's tests**, the corpus among them: one witness
  changed, as Decision 3 says.

## Not claimed

- **A session's handle in a value no page shows**: a page's binding is
  checked by what it answers, and a function's result by where it is
  printed or answered; one held in a server-side `let` and passed to a
  call reaches nothing, and is fine.

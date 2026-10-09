# ADR-0270: the reader's user is the host's, and a listener's handle binds a user's id

Status: accepted at its merge, 2026-10-08, under the owner's delegation of
2026-10-02; proposed by track `notifications` (W3, `track/notifications`), under
the integrator's rulings of 2026-10-07 (docs/PARALLEL.md, "notifications
(W3)") and its answers below. It amends ADR-0091. Date: 2026-10-08.
Milestone: E14, the owner's Twitter list.

## Context

- **The ruling**: `context.current_user()` becomes a platform operation the
  host answers from the session's principal, labelled `User<UserId>`. A
  program constructs no `User<..>`, and a data layer answers a user's id,
  never another user's handle. An event's value of a user's id reaches a
  query parameter of type `User<..>` over that id, and the host drops the
  entries at that user in every session's partition and tells their open
  pages. That last is a change to ADR-0091, brought to the integrator before
  it landed.
- **The question put first**: which type a user's id is. Either the
  platform's `capability.UserId` throughout the feed, or the program's own
  type, named by the principal it reads.
- **What was there, at 1ac6fb6**:
  - `capability.pw` declares `opaque type User<U> = String` and
    `opaque type UserId = String`. `context.current_user()` had the body
    `User("")`, and no host answered it.
  - The feed declared its own `opaque type UserId = String`. Its
    `UserId("you")`, an optimistic author's, built that type, not the
    platform's. No program used `capability.UserId` but through
    `Menus.for_user(.., user: User<UserId>)`.
  - `fn forged() -> User<UserId> !{} { User("bob") }` checked clean: an
    opaque type is built across modules (`values::named`'s
    `Target::Opaque`), and only its `.value` is its module's (ADR-0054).
  - A listener `invalidates_on Notified(reader)`, `reader` a
    `User<UserId>` and `Notified` carrying a `UserId`, was PW0605:
    "argument 1 of `Notified` is declared `capability.UserId` and this is
    `capability.User<capability.UserId>`".

## Research

Read in the compiler at 1ac6fb6, and measured with `pw check` and `pw
build` on copies of the feed.

- **A privacy qualifier is ABI-transparent.** `wit.rs` gives `User<X>` the
  WIT type of `X` (`declared_within`, and `mapped(0)` for
  `types.qualifiers`). A host operation `-> User<UserId>` builds as
  `read: func() -> capability-user-id`. On the wire a handle is its
  user's id, the same string an event carries.
- **The label.** `Signatures::label` keeps the legacy name for
  `User<capability.UserId>`: `Restriction::User("UserId")`. That is the
  label `private` visibility gives (`check.rs`'s `label_of`, `rules.rs`'s
  `privacy_label`), and the one `resume.rs` and `boundary.rs` use. The
  program's own type would be `User<type:feed.app.UserId>`, which no
  visibility names.
- **The program's own type does not build.** A generic platform operation,
  `fn current_user<U>() -> User<U> host "pw:host/principal#read"`, checks,
  and `pw build` refuses it: "WIT: `U` at `read` has no WIT form". A
  non-generic one naming the program's type would need a declaration of
  the principal in the language, and a host type per program.
- **The platform's type costs the feed nothing.** With the feed's own
  `opaque type UserId` gone and `capability.UserId` imported, `pw check`
  is clean and the build differs only in the WIT name of the type
  (`feed-app-user-id` is `capability-user-id`), both aliases of `string`.
  The feed's own `type User` record keeps its name: a handle is written
  `capability.User<UserId>` beside `import capability`.

## Decision

1. **A user's id is the platform's `capability.UserId`**, throughout the
   feed (the integrator's answer to Q1). One issuer and one `sub` per
   deployment (ADR-0258) is one id type, and the label of a handle over it
   is the one `private` already gives.
2. **A program may construct a `UserId`.** An id names someone and grants
   nothing; a handle grants. `UserId("you")` stays, now of the platform's
   type (the integrator's answer).
3. **The host answers the reader's user** (`notifications.rs`, at lines of
   `main.rs` marked `TRACK SEAM (notifications)`):
   - `pw:host/principal#read`, beside `pw:host/session#read`, for a
     query's and a command's host operations: the session's principal's
     `user`, or the session's own guest where no one signed in;
   - a page's binding argument `current_user()` (or
     `context.current_user()`), from the session's principal, as
     `current_session()` is the request's session; never a value a browser
     sends, so `/pw-read` reads a page's list for its own reader alone.
   The platform's declaration, `host "pw:host/principal#read"`, and the
   checks that a program makes no handle (PW5037), that an operation
   outside `pw:` answers none (PW5038), and that no browser supplies one
   (PW5039) are the integrator's, ADR-0263, which this track's first
   commit had made the same declaration as, and gave way to at the rebase.
4. **ADR-0091, amended: a listener's handle binds a user's id.** In
   `invalidates_on` alone, a parameter whose type is the platform's
   `User<T>` (by `Signatures::privacy_kind`, the outer qualifier only)
   binds an event's value of type `T`. `values.rs`'s argument relation
   agrees where the argument is a listener's root, its type `User<T>`, and
   `T` the event's. The entries at that user are dropped, in every
   session's partition (ADR-0256), and their open pages told (ADR-0219).
   - **The runtime is unchanged.** The event's id meets the entry's key by
     value: `invalidate_queries` binds by name, `names_entry` compares by
     value in every partition, and `drop_entries`' `pinned` is false, a
     user id being no session id.
   - **Nothing else relates an id and a handle.** A listener only drops
     cache entries. An `emits`, a call and an `invalidates` key relate the
     two as two types, a `Session` handle binds no session id, and a
     handle over another type binds nothing.

## Alternatives

- **The program's own `UserId`, named by the principal**: a declaration of
  the principal in the language and a host type per program, for a label
  no visibility names. A generic operation does not build.
- **An event carrying the handle itself**, `Notified(reader:
  User<UserId>)`: an `emits` would need the recipient's handle, which only
  the host makes and only for the session that asks. The recipient of a
  like is someone else.
- **Converting a handle to an id in the program** (`user_id(h) ->
  UserId`): a program could then emit its own id freely, but the reverse
  would be wanted next, and it is the one that must not exist.
- **Telling only the named user's sessions** (precision by principal): see
  Not claimed; the integrator kept the superset.

## Consequences

- **A private query keyed by a handle serves its reader alone**: the key
  is made by the host from the session, and no program or browser can make
  another.
- **An event naming a user reaches each of their sessions** without the
  host knowing which sessions are whose: the entries at that user, in any
  partition, are dropped, and every open page reading the query is read
  again. Another user's page reads its own entry, kept, and is sent
  nothing.
- **Telling costs a derive per open page** of the query, not a data-layer
  read: as ADR-0219's precision by key.

## Questions for the integrator, and its answers

1. **Q1, which type a user's id is** (sent 2026-10-07, with the research
   above). *Answer*: the platform's `capability.UserId`; a program may
   construct a `UserId`; `import capability` and `capability.User<UserId>`
   beside the feed's own `User` record is fine.
2. **The handle checks** (proposed as PW5701-PW5703 in this track's
   block). *Answer*: right, but the hole is general and on master already
   (`Session("someone-elses-session")` checked clean too), so they are the
   integrator's, PW5037-PW5039 for `Session`, `User` and `Organization`
   alike (ADR-0263). Not added here; rebased onto them.
3. **The ADR-0091 change** (sent before it landed, with the affected code
   and tests). *Answer*: approved as written, in `invalidates_on` only, the
   platform's `User<T>` by `privacy_kind`, the outer qualifier only. Keep
   the superset of telling, under Not claimed as "telling by principal".
   One mutant more: the event's user dropping the entries of a user it
   does not name.

## Acceptance

Recorded by `just e14-notifications` in
`docs/evidence/E14/notifications.txt`:

- **`compiler/pw-core/tests/principal.rs`, 3 tests**: a listener's handle
  binds the event's id; the controls, each PW0605: a handle over another
  type, a `Session` handle against a session id, an `emits` given a
  handle, a call given a handle for an id and an id for a handle, and an
  `invalidates` key given an id; and the platform declaring the reader's
  user as the host's operation.
- **The server**: `notifications.rs`'s operation answers the session's
  user and a guest's where none signed in; a plan's argument is the
  reader by its call alone; and `tests/notifications.rs`, in memory and on
  PostgreSQL, an event naming a user drops that user's entries in each of
  their sessions and keeps another user's.
- **`scripts/notifications_mutations.py`**: the four of the listener rule
  (no binding, any qualifier, outside a listener, over any type), the
  principal answered with the session, a plan's `current_user()` given
  another user, and a drop at a user the event does not name. The
  notifications ADR's report gives the counts.

## Not claimed

- **Telling by principal**: an event naming a user reads again every open
  page that reads the query, not only that user's. Another user's page
  derives from its kept entry and is sent nothing: a cost of derivation,
  not of exposure. The integrator queues it.
- **A stream's argument** (`<stream query={Q(current_user())}>`): the
  host's stream runs compute `current_session()` and a page's parameters,
  not the reader's user. No page streams one.
- **`identified_by` gone**: the feed still reads its timelines, its
  authors and its relations by the session, which a layer maps to a user
  through the identity's principals. Moving them to `current_user()` is a
  change of every one of those queries' keys, and is not this track's.

## At the merge

Numbered ADR-0270 by the integrator, on 2026-10-08, with its notifications
beside it numbered ADR-0274 (`ADR-XXXX` in the code read one or the other,
as the code it marks is the principal's or the notifications'). Merged from
the track's tip, f20ed75, rebased onto ADR-0273 without a conflict: W3
stopped at the account's weekly limit after its last push, its work
complete. Telling by principal, `identified_by` gone and a stream's
`current_user()` are queued in NEXT.

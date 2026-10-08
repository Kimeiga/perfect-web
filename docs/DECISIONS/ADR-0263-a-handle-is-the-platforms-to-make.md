# ADR-0263: a session's, a user's or an organization's handle is the platform's to make

Status: accepted under the owner's delegation of 2026-10-02; a soundness
defect W3 (track `notifications`) found researching its first question,
generalized and fixed by the integrator. Date: 2026-10-07. Milestone: E14.

## Context

- **A handle reads the data of the one it names.** `Session<SessionId>`,
  `User<UserId>` and `Organization<OrganizationId>` are the platform's
  scoping labels (charter §7.8, `capability.pw`), opaque over the id each
  names. A query keyed by one is private to it, and a data layer reads with
  it: the feed's `timeline(s, limit)` reads the timeline of whom `s` names.
- **A program could make one.** An opaque type is built wherever `T(x)` is
  written, and only its invariant is checked (ADR-0179). So
  `Session("someone-elses-session")` checked clean, and `User(..)` and
  `Organization(..)` too (W3, 2026-10-07, found `User`'s; checked here for
  each).
- **The browser could supply one.** `command peek(s: Session<SessionId>)`
  checked clean: its session is whatever the browser writes in the
  command's body, and what it reads is that session's.
- **The platform's own accessors were forgeries**: `current_user()`'s body
  was `User("")` and `current_organization()`'s `Organization("")`, so every
  reader was one user, in one organization.
- **And a data layer's operation could answer one**, for whomever it named.

## Decision

1. **No program constructs a handle.** `Session(..)`, `User(..)` and
   `Organization(..)` are PW5037 wherever they are written. The host makes
   each from the request it answers, through the platform's operations:
   `current_session()` (`pw:host/session#read`), `current_user()`
   (`pw:host/principal#read`, which W3 answers from the session's
   principal) and `current_organization()` (`pw:host/organization#read`,
   which no host answers yet: a program that asks is refused where it is
   served).
2. **A data layer answers ids, never handles.** An operation outside the
   platform's (`pw:…`) whose answer holds a handle is PW5038, wherever in
   its type the handle sits: the answer itself, an argument
   (`Option<User<UserId>>`, `List<..>`), a record's field, a case's payload,
   an opaque type's representation.
3. **Nothing the browser supplies holds a handle.** A command's parameter,
   a page's parameter, a module's signal, and a page's or a view's signal,
   whose type holds one, are PW5039.
4. **An id is the program's, and so is a secret.** `UserId("x")` names
   someone and grants nothing; a read by an id is a public read.
   `Secret(token)` restricts a value and grants nothing either.

## Acceptance

- **`compiler/pw-core/tests/handles.rs`, 3 tests**, each with its
  controls: a session's, a user's and an organization's handle constructed,
  one inside another value; an operation answering a handle, in an argument
  and in a record's field, beside an id answered and the platform's own
  operation; a command's parameter, plain and in an `Option`, a page's
  parameter, a module's signal and a view's, beside a command given an id
  and a query keyed by the reader's session.
- **`scripts/handle_mutations.py`, 9 mutants**: a construction not found,
  a user's handle left out, an operation outside the platform's answering
  one, a handle not found in an argument, a record's field or as an
  organization's, and a page's parameter, a module's signal and a view's
  not held. Recorded by `just e14-handles`.
- **The whole workspace's tests**, the corpus among them: no program in the
  repository made or took a handle. The trusted platform contract's hash
  changes for `context.pw`'s two accessors.

## Not claimed

- **A handle sent to the browser.** A page may print its session's handle,
  `{mine.session}`, and a session's id is its cookie's value, which
  `HttpOnly` keeps from scripts and a page that prints it gives back. A
  user's or an organization's id is a name, and the browser holding it
  reads nothing; a session's is a credential. Next, as ADR-0264.

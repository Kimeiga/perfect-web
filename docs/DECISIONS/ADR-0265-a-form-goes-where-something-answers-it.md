# ADR-0265: a form goes where something answers it

Status: accepted under the owner's delegation of 2026-10-02; the identity
track's second question (ADR-0258) and the uploads track's fourth
(ADR-0260). Date: 2026-10-08. Milestone: E14.

## Context

- **A link was checked against the program's pages** (PW5009, charter
  §8.2), and a form was not, but a file's (PW5603, ADR-0260). The feed
  reaches `/sign-in`, `/sign-up` and `/sign-out` by forms, because a link
  to them was PW5009: their routes are the host's, the relying party's
  (ADR-0258), and no page declares them.
- **A form's `action` is a request, sent with the form's `method`**, `get`
  where it states none. A `get` to `/sign-out`, which answers a `post`, or a
  `post` to a page, failed as it was submitted, and checked clean.
- **The relying party is Pleris's** (ADR-0258's first decision), so every
  deployment's host serves its routes, whatever its provider: `GET
  /sign-in`, `GET /sign-up` (the same flow, with a create hint), `GET
  /sign-in/callback` and `POST /sign-out`.

## Decision

1. **The route table states what a request may reach, by method**: each
   page's route, a `get`; the relying party's four,
   `routes::RELYING_PARTY`; and each upload's route a `post`, and a lease
   shown under it and what it serves, `get`s.
2. **A link is a `get`**: it reaches a page, or what the relying party or an
   upload answers so. A link to `/sign-in` is no longer PW5009; one to
   `/sign-out`, which answers a `post` alone, is.
3. **A form's `action`, internal, answers its `method`**, read in any case:
   or PW5041 at the action, saying what the route answers instead. A form
   that sends a file, states an `enctype` or posts to an upload's route
   stays PW5603's; a form with no `action` submits to its own page or to its
   handler, and is not checked; one sent to another origin names what the
   program does not own.
4. **The host answers exactly the four**: the development server's
   identity, held to `routes::RELYING_PARTY` by a test there, so the
   compiler's table and the host's routes are one list.

## Acceptance

- **`compiler/pw-core/tests/form_routes.rs`, 3 tests**, each with its
  controls: a `post` to a page, a `get` to `/sign-out` and a `post` to
  nothing refused at the action, beside forms to each route by its method,
  in any case, one with no `action` and one to another origin; a link to
  `/sign-in` and `/sign-up` taken, and to `/sign-out` and to nothing
  refused; and a form that sends a file left to PW5603, beside one sent to
  its upload and a link to what an upload serves.
- **The server**: `the_relying_party_answers_the_routes_the_compiler_knows`,
  each route answered by its method and not by the other.
- **`scripts/form_route_mutations.py`, 10 mutants**, recorded by `just
  e14-form-routes`.
- **The whole workspace's tests**, the corpus among them: each of the
  feed's forms is answered.

## Not claimed

- **A deployment without accounts** answers none of the relying party's
  routes (the guest model), which a program cannot know where it is
  checked: its links and forms to them check.
- **A button's `formaction` or `formmethod`, a form's `target`, and the
  `dialog` method** are not read.
- **A route the identity answers beyond the four** is not found by its
  test, which holds each of the four answered.

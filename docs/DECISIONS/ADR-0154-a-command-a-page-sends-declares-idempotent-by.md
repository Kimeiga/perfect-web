# ADR-0154: a command a page's handler calls says how a second delivery is answered

Status: accepted under the owner's delegation of 2026-10-02. Date: 2026-10-03.
Milestone: E14, gate item 5's second open form (T08).

## Context

T08's setup is a command a page's Add calls, with no `idempotent_by`, and it
checks clean. A retrying proxy, a flaky network or a client's own retry
delivers its request twice, and the cart gets two items. PW0312 refuses only
`retry` on such a command. ADR-0121's runtime gives every press an
interaction, and only an `idempotent_by` command uses it; ADR-0121 says of the
others "it runs every time, as written".

Primary sources:
- **RFC 9110 §9.2.2.** "A client MUST NOT automatically retry a request with
  a non-idempotent method unless it has some means of knowing that the
  request semantics are actually idempotent". An idempotency key is that
  means.
- **The IETF HTTPAPI working group's `Idempotency-Key` header** (draft 07,
  October 2025) makes a non-idempotent method fault-tolerant by such a key,
  as Stripe's API does on every POST.

## Decision

**A command a page's handler calls declares `idempotent_by`** (PW0339).
- The browser's request can be delivered twice whatever the program does, and
  the platform already gives each press its interaction.
- A command called only by other server code is exempt.
- Naturally idempotent commands, such as `clear_cart`, declare it too. It
  costs one line, and keeps one rule for every command a page sends.

## Acceptance

- `compiler/pw-core/tests/case_and_delivery.rs`: a command a page's handler
  calls, with no `idempotent_by`, is refused, naming the press that sends it;
  the same command declaring it, and a command no handler calls, are the
  controls.
- `scripts/case_and_delivery_mutations.py`.
- T08's controls, at the commit that records this ADR's evidence: its setup
  is refused at `pw check`, and its reference passes.

## Consequences

- T08's setup no longer checks. Its negative control fails at `pw check`, as
  T12's does, and `pw check` names the repair. T08's class, a press's
  request applied twice, is refused whole.
- With every page-sent command keyed, the runtime may retry a failed request
  safely, as RFC 9110 then allows. That is not done here.

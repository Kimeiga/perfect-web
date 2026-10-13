# ADR-XXXX: a payment is the provider's word, held to its signature

Status: draft (the integrator's; NEXT item 6, step 6c). Date: 2026-10-13.
Milestone: E14. Builds on ADR-0325 (an interaction's answer commits with
its writes), ADR-XXXX (money is rounded only where a program names how),
A-010 (an origin-only secret operation, `capture_payment`) and the charter's
`Secret<Payments>` (§7.8, §15.2).

## Context

- **The charter drew payment and nothing ran it.** `secrets.payments()`
  hands an origin-only `Secret<Payments>`; `Payments.capture(key, order,
  amount)` is `todo`; A-010's `capture_payment` is checked and never built.
- **Practice** (each read 2026-10-12; scratchpad research, to be cited in
  full at acceptance):
  - Stripe keeps one PaymentIntent per cart, reused on retry; a decline
    returns it to `requires_payment_method` for another card; requests are
    idempotent by `Idempotency-Key`, a key answering at least a day and
    refused with other parameters.
  - Card details stay in the provider's hosted fields, an iframe on its own
    origin: the merchant's server never sees them, which is PCI DSS's SAQ A
    (the PCI SSC's FAQ 1438; the January 2025 SAQ A adds the criterion that
    the page embedding the iframe is not open to script attacks).
  - Fulfilment comes from the provider's webhook, never from the browser:
    the event is signed (`Stripe-Signature: t=..,v1=..`, HMAC-SHA256 of
    `"t.body"`), checked within a tolerance, may arrive twice or out of
    order, and the intent is read again before acting.

## Decision (so far)

1. **A webhook is the provider's word only as it signed it**
   (`payments.rs`): HMAC-SHA256 (RFC 2104, over the workspace's `sha2`, held
   to RFC 4231's vectors), every `v1` signature tried, five minutes either
   way, compared in constant time.
2. **A development provider** charges nothing and behaves as one does:
   intents idempotent by key for a day; a card's outcome by Stripe's test
   numbers (4242 succeeds; 3220 and 3184 ask to authenticate; 0002 is
   declined; 9995 has insufficient funds; any other is declined); every
   change an event signed for the host.
3. To come: the provider on its own origin (the hosted fields' page and its
   API); the `Payments` capability's host operations; the order awaiting
   payment, placed only from a verified, de-duplicated webhook, the intent
   read again, in one transaction; the page told.

## Not claimed (so far)

- A real provider: a Stripe adapter in test mode waits for the owner's
  approval of each crate it needs (NEXT 6d).
- Refunds, captures held for later (`requires_capture`), disputes.

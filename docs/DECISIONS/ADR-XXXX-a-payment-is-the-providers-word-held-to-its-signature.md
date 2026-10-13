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
3. **The program says which command a payment's outcome invokes**
   (`invoked_by Payments`), and no reader presses it. The host calls it once
   it has its provider's word for the outcome: a signed event, or the
   reader's return, each read again from the provider (Stripe's "Fulfill
   orders": the fulfilment "might be called multiple times, possibly
   concurrently"; retrieve, check the status, record the fulfilment). Its
   export says so (`ComponentExport.invoked_by`, part of its interface's
   hash), and a press naming it is answered as a command that is not there,
   as a Convex client finds no public function for an internal one ("can
   only be called by other functions and cannot be called directly from a
   Convex client"). The checks, PW64xx, the payments track's:
   - PW6401: it declares no `requires`: no reader acts;
   - PW6402: it is given one `payments.Settled` (the payment, the order it
     was begun for, where it now stands), what the host has to give;
   - PW6403: one command answers a payment's outcome in a program;
   - PW6404: no page's handler sends it.
4. **The platform's payments** (`packages/pw-platform-web/payments.pw`):
   `Payment`, `PaymentStatus` (Stripe's statuses), `Settled`, and
   `begin(key, order, amount)`, effect `payments.begin`, at the origin alone
   as the key is. The amount is in minor units: only `domain` reads an
   amount's units (ADR-XXXX, money), so only it begins a payment.
5. To come (step seven, designed 2026-10-13):
   - **On the reader's behalf.** `begin` is called in the reader's command;
     the host keeps who began each payment, with its order and amount, in
     the intent's own record (Stripe's `metadata`), and invokes the command
     `invoked_by Payments` as that reader: its `current_user()` is the
     order's owner, so it reads and writes the reader's order and emits
     `UserOrderChanged(current_user())` as `place_order` does. Still no
     `requires`: no reader acts; the provider's word does.
   - **The amount is the host's to hold**: an intent whose amount is not the
     one begun settles nothing, said.
   - **The order awaits its payment**: where the deployment takes payments,
     `place_order` writes the order `AwaitingPayment` with its charges and
     its lines, reserves its promo code, and begins the payment; the order
     page frames the provider's page (`Payment.page`, the host's) for the
     reader to pay on. The settled command places it on `Succeeded`
     (clearing the cart, the code used), leaves it awaiting on a decline (the
     same intent takes another card), and cancels it on `Canceled` (the code
     released). A deployment that takes no payments places as before.
   - Not claimed (so far): the reader's return as a second trigger (Stripe
     recommends it beside the webhook), and a Content-Security-Policy on the
     store's pages (`frame-src` the provider alone; SAQ A's 2025 criterion
     for the page that frames the hosted fields).

## Not claimed (so far)

- A real provider: a Stripe adapter in test mode waits for the owner's
  approval of each crate it needs (NEXT 6d).
- Refunds, captures held for later (`requires_capture`), disputes.

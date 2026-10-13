# ADR-XXXX: a payment is the provider's word, held to its signature

Status: draft (the integrator's; NEXT item 6, step 6c). Date: 2026-10-13.
Milestone: E14. Builds on ADR-0325 (an interaction's answer commits with
its writes), ADR-0327 (money is rounded only where a program names how),
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
   `Payment` (with the provider's `page` for it), `PaymentStatus` (Stripe's
   statuses), `Settled`, `BeginError` (`TransportFailed`; `NotTaken` where
   the deployment takes no payments), and `begin(order, amount)`, effect
   `payments.begin`, granted at the origin alone. The amount is in minor
   units: only `domain` reads an amount's units (ADR-0327), so only
   it begins a payment.
5. **The key is the host's, never the program's** (2026-10-13). The charter
   maps `secret<Payments>` to a payments-secret handle (§10.2) and gives the
   origin world the payment secret (§7.3): a capability the host exercises,
   not a value a component holds. Until step seven `begin` also took
   `key: Secret<Payments>`, and what a program could give it,
   `secrets.payments()`, is `Secret("")`: no key, which the host ignored. A
   parameter that said a key crossed and carried none is gone; the effect,
   granted at the origin alone, is the authority. A program that never
   holds the key leaks it down no path, whatever a label misses.
6. **An order awaits its payment** (step seven):
   - **On the reader's behalf.** `begin` is called in the reader's command,
     `place_order`. The host keeps who began each payment, with its order
     and amount, in the intent's own record at the provider (Stripe's
     `metadata`), so a host started after another still knows them, and
     invokes the command `invoked_by Payments` as that reader: in a session
     the host opens for it, bound to the reader and closed when the command
     answers. Its `current_user()` is the order's owner, so it reads and
     writes the reader's order and emits `UserOrderChanged(current_user())`
     as `place_order` does. Keyed by the payment and its status (ADR-0325),
     an outcome heard twice commits once (PW6405: such a command declares
     `idempotent_by`).
   - **That session is never a browser's.** Its name is the payment's, no
     secret, so that its interaction's record keys it; every session the
     platform opens on a reader's behalf is named `on-behalf:`, and a cookie
     naming one is a new session. Found in review (2026-10-13): a cookie was
     taken as it came, so a request naming the session while a payment was
     settled was the reader's, and the development provider's payments are
     numbered in order.
   - **An outcome is remembered once it is acted on.** One the host could
     not settle (a trap, a refusal) is answered 500, and acted on when the
     provider sends it again; until step seven it was remembered first, and
     a resend would have been ignored.
   - **The amount is the host's to hold**: an intent whose amount is not the
     one begun settles nothing, said (`AmountChanged`).
   - **The order awaits its payment.** Where the deployment takes payments,
     `place_order` places the reader's cart as their order, charged, its
     code used and the cart emptied, as before, but `AwaitingPayment`, with
     a reference of its own (`o-` and 16 random characters) that the payment
     is begun for; the order page frames the provider's page for it
     (`Payment.page`). The settling command places it on `Succeeded`; a
     decline leaves it awaiting, the same payment taking another card; an
     outcome for another reference changes nothing. A reader whose order
     awaits its payment places no other (`OrderError.AwaitingPayment`), and
     the cart page says so.
   - **A deployment that takes no payments places as before**: `begin`
     answers `NotTaken`, and `place_order` settles its order as paid in the
     same transaction.
   - **A provider that cannot be reached** is `TransportFailed`:
     `place_order` answers `PaymentUnavailable`, and nothing it wrote is
     committed: no order, and the cart as it was.

## Not claimed (so far)

- **Taking an awaiting order back** (step eight, next): nothing in this
  deployment cancels a payment, so a reader whose order awaits a payment
  they will not make can place no other. And a provider cancels one of its
  own accord: Stripe's intents "might also automatically transition to
  `canceled` if they're confirmed too many times" ("How Payment Intents and
  Setup Intents work", read 2026-10-13), and a merchant cancels from its
  dashboard. A `Canceled` outcome still leaves the order awaiting a payment
  that can no longer be made.
- The reader's return as a second trigger (Stripe recommends it beside the
  webhook), and a Content-Security-Policy on the store's pages (`frame-src`
  the provider alone; SAQ A's 2025 criterion for the page that frames the
  hosted fields).
- A real provider: a Stripe adapter in test mode waits for the owner's
  approval of each crate it needs (NEXT 6d).
- Refunds, captures held for later (`requires_capture`), disputes.

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
- **Practice** (each read 2026-10-13):
  - **A payment per order, retried by key.** Stripe's requests are
    idempotent by `Idempotency-Key`; keys may be removed "after they're at
    least 24 hours old", and the idempotency layer "compares incoming
    parameters to those of the original request and errors if they're not
    the same" (Stripe, "Idempotent requests"). A decline returns a payment
    to `requires_payment_method` "so that the payment can be retried"
    (Stripe, "How Payment Intents and Setup Intents work").
  - **Card details in the provider's frame.** Every element of the payment
    form comes from the provider's iframe for a merchant to be SAQ A's
    (PCI SSC FAQ 1438, "How is the payment page determined for SAQ A
    merchants using iframe?"); since 2025 SAQ A asks too that the page
    framing it is not open to script attacks (FAQ 1588), met by the
    merchant's controls or the provider's confirmation.
  - **The provider's word, as it signed it.** A webhook is signed in
    `Stripe-Signature` (`t=…,v1=…`), HMAC-SHA256 of the time, a `.` and
    the body; Stripe's library takes a signature within five minutes
    (`DEFAULT_TOLERANCE: 300, // 5 minutes`), any one matching, compared by
    `secureCompare` (stripe-node, `src/Webhooks.ts`); while a secret rolls,
    "Stripe generates one signature per secret until expiration".
  - **Heard more than once, late, and read again.** An endpoint "might
    occasionally receive the same event more than once"; Stripe retries an
    unanswered one "for up to three days", and one already processed is
    answered 200 "to stop further retries" (Stripe, "Manage webhook
    endpoints"). An event's object "can be stale, fetch the latest resource
    from the API before taking any critical action" (Stripe, "Set up
    events"). Fulfilment "might be called multiple times, possibly
    concurrently", and never from the landing page alone (Stripe, "Fulfill
    orders").
  - **The key on the server.** "Don't embed the API key or the endpoint
    secret in source code or client-side applications" (Stripe, "Set up
    events").

## Decision

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
   it has its provider's word for the outcome: a signed event, read again
   from the provider (the reader's return as a second trigger is not
   claimed). Its export says so (`ComponentExport.invoked_by`, part of its
   interface's hash), and a press naming it is answered as a command that is
   not there, as a Convex client finds no public function for an internal
   one ("Internal functions can only be called by other functions and cannot
   be called directly from a Convex client"; Convex, "Internal Functions").
   The checks, PW64xx, the payments track's:
   - PW6401: it declares no `requires`: no reader acts;
   - PW6402: it is given one `payments.Settled` (the payment, the order it
     was begun for, where it now stands), what the host has to give;
   - PW6403: one command answers a payment's outcome in a program;
   - PW6404: no page's handler sends it;
   - PW6405: it declares `idempotent_by`, as the provider's word arrives
     more than once, possibly at once (step seven).
4. **The platform's payments** (`packages/pw-platform-web/payments.pw`):
   `Payment` (with the provider's `page` for it), `PaymentStatus` (Stripe's
   statuses), `Settled`, `BeginError` (`TransportFailed`; `NotTaken` where
   the deployment takes no payments), and `begin(order, amount)`, effect
   `payments.begin`, granted at the origin alone; and (step eight)
   `cancel(payment)`, effect `payments.cancel`, with its `CancelError`. The
   amount is in minor units: only `domain` reads an amount's units
   (ADR-0327), so only it begins a payment.
5. **The key is the host's, never the program's** (2026-10-13). The charter
   maps `secret<Payments>` to a payments-secret handle (§10.2) and gives the
   origin world the payment secret (§7.3): a capability the host exercises,
   not a value a component holds. Until step seven `begin` also took
   `key: Secret<Payments>`, and what a program could give it,
   `secrets.payments()`, is `Secret("")`: no key, which the host ignored. A
   parameter that said a key crossed and carried none is gone; the effect,
   granted at the origin alone, is the authority. A program that never
   holds the key leaks it down no path, whatever a label misses, as Stripe
   keeps its key out of anything a client runs.
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
7. **An awaiting order is taken back** (step eight). A reader whose order
   awaits a payment they will not make could place no other, and a provider
   cancels a payment of its own accord: Stripe's intents "might also
   automatically transition to `canceled` if they're confirmed too many
   times", and a merchant cancels one from its dashboard ("How Payment
   Intents and Setup Intents work").
   - **The reader's `cancel_order`** cancels the payment at the provider
     (`payments.cancel`, effect `payments.cancel`, at the origin alone), then
     takes the order back in the same transaction; its errors are its own
     (`CancelOrderError`), so no match on why an order was not placed has
     to answer them. Where the provider says the payment is past
     canceling, paid or being paid, the order stands, answered `Paid`, and
     the provider's word places it (Stripe: a payment is cancelable while it
     waits for a payment method, a confirmation, an action or a capture,
     and the request "Returns an error if the PaymentIntent is already
     canceled or isn't in a cancelable state"; "Cancel a PaymentIntent").
     One canceled already is `Canceled`: what was asked holds, so a press
     retried after its first's transaction rolled back takes the order back
     too.
   - **A payment is its reader's to cancel alone**: the host reads the
     payment first and refuses one the command's reader did not begin, as it
     records who began each (decision 6). No program of the reader's could
     name another's payment; the host holds it all the same.
   - **A canceled outcome takes the order back too** (`settle_payment`), so
     a provider's own cancel reaches the order as the reader's does.
   - **Taken back**: the order `Canceled`, its lines back in the cart beside
     what the cart holds now (a line of the same item gains its quantity),
     its code's use released, the payment it awaited forgotten; the order
     page says so, its total "Nothing was charged.", and the cart page
     shows the lines. The order page's button is shown where `SignedIn`
     holds, as the command requires (PW5048).
   - **The provider's events come after its answer**: the host reads an
     answer by its length (RFC 9112 §6.3), not to the connection's close, so
     a provider that sends its event before closing does not hold the
     reader's command, which holds the host's layer, while the event waits
     for that layer.
   - **The host waits for its provider ten seconds at most**, to connect,
     to send and to be answered: a provider that does not answer by then is
     one that cannot be reached (`TransportFailed`), as the host's commands
     wait while it answers one of them. Found by a mutant of the read
     above, which hung the store's tests rather than failing them.

## Not claimed

- **An awaiting order no one pays or takes back** stays awaiting: an intent
  does not expire on its own (a Checkout Session does, in a day), and a
  deployment's sweep of stale ones (`cancellation_reason: abandoned`) is
  not built. A taken-back order's checkout choices, its tip and its code,
  are the reader's to choose again.
- **The provider is asked inside the command's transaction**: a host's
  commands are serialized by its layer, so the host waits while the
  provider answers a payment begun or canceled, and an intent begun for a
  command that then rolls back stays at the provider, unpaid.
- The reader's return as a second trigger (Stripe recommends it beside the
  webhook, as "webhooks can sometimes be delayed"), and a
  Content-Security-Policy on the store's pages (`frame-src` the provider
  alone; SAQ A's 2025 criterion for the page that frames the hosted
  fields).
- **A secret in a built program.** The backend and the WIT lay a privacy
  qualifier out as its argument (the 2026-08-20 ruling; ADR-0031, ADR-0270),
  which is right for `User<UserId>`, whose argument is the identity, and
  wrong for `Secret<C>`, whose argument tags what the secret is for and
  whose value is its material (`opaque type Secret<C> = String`). So
  `secrets.payments()` checks and does not build ("a type parameter no use
  instantiates", found in step seven). No program holds the key (decision
  5); how a secret crosses a component's boundary, as a handle the host
  resolves or not at all, is ruled with the first program that needs one.
- A real provider: a Stripe adapter in test mode waits for the owner's
  approval of each crate it needs (NEXT 6d).
- Refunds, captures held for later (`requires_capture`), disputes.

## Alternatives

- **The key as a value the program passes** (the charter's first sketch,
  `Payments.capture(key, order, amount)`, and this ADR's step six): a
  parameter that said a key crossed and carried none. The effect, granted at
  the origin alone, is the authority (decision 5).
- **An order placed only once it is paid**, its cart kept while the reader
  pays, as a hosted checkout keeps one: the cart can change while the
  reader pays, and the payment's amount is fixed when it begins, so every
  change would amend the payment, a second path for each. The order
  awaiting its payment holds what the payment is for.
- **The reader's return as the outcome**: a page's word is the reader's,
  not the provider's; Stripe fulfils an order from the webhook ("Fulfill
  orders"), and recommends the return beside it only as a second trigger.
- **A new random session for each settling**: an interaction's record is
  keyed by its session, so an outcome heard twice, or by two hosts, would
  commit twice. The session's name is the payment's, and browsers are kept
  from it by its prefix instead (decision 6).
- **Stripe's Checkout Sessions**, which Stripe now recommends for most
  integrations: a real provider's API is 6d's; the development provider
  keeps the PaymentIntent's model, which Checkout Sessions are built on.

## Acceptance

- `spikes/own-renderer/server/src/payments.rs`: HMAC-SHA256 held to RFC
  4231; a webhook signed, stale on either side, forged, unsigned, under a
  rolled secret; signatures compared whole; the development provider's
  intents once per key, its cards by Stripe's test numbers, its events
  signed, sent twice or out of order; the hearing reading the intent again,
  an amount changed, an event heard before, an outcome not settled settled
  when sent again; a payment begun once per interaction (and canceled by its
  reader alone, step eight); a press of the settling command refused.
- `compiler/pw-core/tests/invoked_by.rs`: PW6401 to PW6405.
- `spikes/own-renderer/server/src/tests/store_payments.rs`, the store in
  memory and on PostgreSQL: an order awaiting its payment on the provider's
  page, framed; a card paid placing it and the page open on it told; a
  decline leaving it awaiting; another order's outcome changing nothing; an
  unreachable provider placing nothing; a reader's press of the settling
  command not a command; a session opened on a reader's behalf no browser's
  to present; (step eight) taken back, its code the reader's again, a
  payment paid first standing, a provider's own cancel, an order moved
  along not taken back.
- `spikes/own-renderer/e2e/payments.spec.mjs`, in three engines.
- `scripts/invoked_by_mutations.py` and `scripts/payments_mutations.py`,
  each mutant killed (`just e14-invoked-by`, `just e14-payments`).

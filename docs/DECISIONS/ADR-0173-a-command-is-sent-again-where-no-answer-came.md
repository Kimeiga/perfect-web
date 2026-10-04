# ADR-0173: a command is sent again where no answer came, and a command that is retried is idempotent

Status: accepted under the owner's delegation of 2026-10-02. Date: 2026-10-04.
Milestone: E14, the audit's fifth gap (charter §15.4): `add_to_cart` declares
"bounded retry only for safe transport failures".

## Context

- **A command was sent once.** The runtime's `command()` sent a press's
  request and threw on any failure. A request the network dropped failed the
  press, and its line went away again, though the server might have
  committed it.
- **Sending it again is safe.** Every command a page sends is
  `idempotent_by InteractionId` (ADR-0154, PW0338). The host runs a command
  once for its interaction, and answers a request sent again with the
  first's outcome, waiting for it while it runs (ADR-0121, `pw-resource`'s
  `try_command`). The IETF's idempotency-key draft answers such a request
  with 409; an answer the page can use is better.
- **A-005 and A-010 declare `retry transport_only(max = 2, jitter = true)`,
  and nothing ran it.** The store's commands declared no retry.
- **PW0312 let `transport_only` retry a command that is not idempotent.**
  It refuses a retried mutation without `idempotent_by`, except where the
  policy was `transport_only`, as though a transport failure said the
  request never arrived. A browser's `fetch` fails the same way whether the
  request never left or its answer was lost after the server committed.
  R-014's own expected message states the rule: "only commands with an
  `idempotent_by` key may be retried". ADR-0089 made PW0312 read the
  operator exactly; the exemption was older. PW0312 also refused
  `retry none`, which retries nothing.

## Decision

1. **A command that is retried is idempotent, whatever it retries on**
   (PW0312). `transport_only` is not exempt; `retry none` is not a retry.
2. **A handler sends a command again where no answer came**, as the
   command's `retry` clause says:
   - the compiled handler passes the policy where it sends the command:
     `{ retry: { max, backoff, jitter } }`;
   - the runtime, which alone sees that no answer came, sends the request
     again with the same interaction, at most `max` more times. An answer
     cut off in transit is no answer;
   - an answer of any kind is an outcome, and is never sent again: a
     refusal, a trap, an HTTP error;
   - the line the press made stays shown while the request is sent again,
     and goes only when the last send fails.
3. **The waits are TanStack Query's**: `Math.min(1000 * 2 ** n, 30000)`
   milliseconds before the nth resend, the same each time for `fixed`. With
   `jitter`, each is drawn at random below that (full jitter, as AWS's
   Builders' Library recommends), so that pages which lost one connection do
   not all send again at once.
4. **The store's five commands declare `retry transport_only(max = 2,
   jitter = true)`**, as charter §15.4 requires of `add_to_cart`. A press
   that loses its request once or twice is not lost.

## Alternatives

- **Keep the exemption for requests that were never sent.** A browser cannot
  observe that, so the rule would hold for a case nothing can recognise.
- **Retry on the server.** A retry belongs at one point in the stack, and
  the browser is the one that sees the transport fail (AWS's Builders'
  Library: "Retry at a single point in the stack").
- **Retry a 5xx too.** An answer is an outcome. A retried trap would run the
  same failing body again, and the host answers a repeated interaction with
  the first's outcome anyway.
- **Answer a running interaction with 409**, as the IETF draft does. The
  page would need its own wait and resend for what the host already does.
- **Fixed, short waits.** Pages that lost one connection would send again
  in step. The line is already shown, so a wait of a second costs the user
  nothing they can see.

## Acceptance

Recorded by `just e14-command-retry` in `docs/evidence/E14/command-retry.txt`:

- `policy_values.rs`: `transport_only`, `bounded_exponential` and `fixed`
  each need `idempotent_by`; `retry none` does not.
- `handlers.rs`: the store's handlers pass their policy; `retry fixed(max =
  3)` is passed as `fixed`; a command with no clause is passed none.
- `e2e/retry.spec.mjs`, in Chromium, Firefox and WebKit:
  - a request the network drops is sent again, its line shown meanwhile,
    and it is the server's line;
  - an answer lost after the server committed: sent again, and the line is
    there once;
  - an HTTP error is not sent again, and the press fails;
  - after the request and two more, the press fails and its line goes;
  - `demo.pick`'s `pick`, which declares no retry, is sent once.
- `scripts/command_retry_mutations.py`: 10 mutants.

## Not claimed

- **A request that hangs waits.** A command declares no `timeout`, so a
  connection open with no answer is not a failure.
- **A cut-off answer is untested.** The runtime treats an answer cut off
  after its headers as none, and no test cuts one off.
- **The waits are not measured.** Their bound is the runtime's.
- **A press whose resends all fail says nothing a reader hears.** Its control
  is marked (`data-pw-handler-error`) and its line goes. A handler cannot
  match "not sent", since a command's result type has no such case.
- **Offline is not waited out.** The runtime sends again without asking
  whether the browser is online.

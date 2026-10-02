# ADR-0121: an idempotent command runs once per interaction

Status: accepted under the owner's instruction of 2026-10-02 ("implement
optimistic and idempotency first"). Date: 2026-10-02. Milestones: E14 (it
blocks benchmark task T08), E10's command path.

## Context

The store's commands declare `idempotent_by InteractionId`. `pw check` checks
the clause (PW0312/PW0303 require it beside `retry`, and ADR-0089 types it).
Nothing else read it (ADR-0120's finding): no contract carried it, the
browser runtime sent no interaction, and the development server ran every
request. A request sent twice, by a retrying proxy or a flaky network, added
twice.

`runtime/pw-resource` already had the mechanism, from E4: `try_command`
reserves a key before running, lets a duplicate wait for and share the first
outcome, and never re-executes an outcome a panic left unknown. Its own
documentation says the host must bind the key to authority, operation and
payload. It also kept every key for the life of the process.

## Decision

1. **The clause reaches the host as data.** `ComponentExport.idempotent_by`
   carries the key type, beside ADR-0115's `authorization`, and is part of
   `abi_schema`: a keyed contract and an unkeyed one are different invocation
   contracts even where the Wasm signature is identical. Mirrored in
   `pw-host`.
2. **The runtime makes the interaction.** One per press, and one per command
   the press calls (`<press>-<n>`), sent as the `pw-interaction` header and
   kept unchanged by any retry of the same request. Made by the runtime and
   never by the handler, so a handler cannot forget it or reuse it. Made with
   `crypto.getRandomValues`, which exists outside secure contexts too.
3. **The host runs an idempotent command at most once per interaction.** The
   key is the session, the command and the interaction, so one session's id
   is not another's, and the reservation is `pw-resource`'s `try_command`,
   not a second mechanism. Refused before anything runs:
   - an idempotent command's request with no interaction;
   - a malformed interaction (empty, over 128 bytes, or outside `[A-Za-z0-9_-]`);
   - an interaction sent again with other arguments. The same interaction
     is the same request; one with a different payload is a client bug, and
     answering it with the first outcome would hide it.
4. **What is kept is bounded.** `pw-resource` gains `forget_command`, which
   forgets only a *known* outcome: a reservation still running, and an
   outcome a panic left unknown, are kept, because forgetting either would
   allow a second execution. The server keeps the last 64 interactions per
   session (`INTERACTIONS_PER_SESSION`) and drops a session's with its idle
   subscriber. **The bound's cost, stated:** a retry of an interaction older
   than 64 later ones from the same session, or arriving after the session
   was forgotten, runs again.

## Not claimed

- Exactly-once across processes or restarts. The outcomes are in memory, as
  `pw-resource` says of itself; durable idempotency needs storage the
  development server does not have.
- That the runtime retries. It still sends each request once; this makes a
  retry, by anything, safe.
- Anything for a command that does not declare `idempotent_by`: it runs every
  time, as written.

## Acceptance

- `compiler/pw-core/tests/component_contract.rs`:
  `idempotent_by_is_an_export_property_and_part_of_the_abi`.
- `runtime/pw-resource/tests/concurrency_regressions.rs`:
  `forgetting_a_command_keeps_what_must_not_run_twice`.
- The development server's tests: `a_retried_interaction_runs_its_command_once`
  (three sends, one add; sessions independent; the three refusals) and
  `interactions_kept_are_bounded_per_session`.
- `spikes/own-renderer/e2e/idempotent-command.spec.mjs`: the browser's own
  request sent twice at the network is one add; two presses are two; a
  request without an interaction is refused with 400.
- `docs/evidence/E8/component-contracts.{json,txt}`: both store commands
  record `idempotent_by: InteractionId`, and only their ABI hashes move.

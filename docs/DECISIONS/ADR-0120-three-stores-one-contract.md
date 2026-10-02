# ADR-0120: three stores, one contract (E14-A)

Status: accepted under the owner's instruction of 2026-10-02 ("start on the
Next.js and SvelteKit stores"). Date: 2026-10-02. Milestone: E14 (charter
§14 M14 task 4, §15).

## Context

E14's benchmark compares the same tasks across React/Next, SvelteKit and
Pleris (charter §14 M14 task 4). A comparison is only fair if the three
starting points are the same store, and that has to be checked by tests, not
asserted in prose.

## Decision

1. **Layout is the charter's (§12):** `benchmarks/baselines/next-react`,
   `benchmarks/baselines/sveltekit`, `benchmarks/harness`. The Pleris store
   stays where it is, `examples/store` served by the own-renderer development
   server. Each baseline is self-contained, because an agent will work in an
   isolated copy of exactly one.
2. **Versions**, verified on the npm registry on 2026-10-02 and pinned exactly:
   Next.js 16.3.8 with React 19.3.0; SvelteKit **2.70.3** with Svelte 5.57.1,
   Vite 8.3.2 and adapter-node 5.5.7; TypeScript 6.0.3; Playwright 1.58.0, as
   the own-renderer suite uses.
   - **Not SvelteKit 3.0.0**, released 2026-10-01. A benchmark of coding
     agents on a day-old major measures what the models have not seen, not
     the framework. Revisit when E14-E runs.
   - Next 16 has been released since 2025-10-22. TypeScript 6.0.3 is the
     newest that both SvelteKit 2 and svelte-check accept.
3. **Idiomatic code in each stack, with the same markup contract.** Next
   uses a Server Component page, Server Actions in `<form action>`, and
   `proxy.ts` for the session cookie. SvelteKit uses a `load`, named form
   actions with `use:enhance`, and `hooks.server.ts`. All three render the
   ids, landmarks and text the Pleris store renders (`#store-name`, `#menu`,
   `#cart-count`, `#clear-cart`, the Menu and Cart regions). Data is in
   memory in the server process, as the Pleris development server keeps it.
4. **One behavioural contract, `benchmarks/harness/contract/`**, run against
   built artifacts of all three stores (`next start`, `node build`, the
   compiled Pleris page). A test may know only the store's path and when the
   page is ready. The contract is the store as **all three run it today**:
   the menu, Add, Clear, a cart that survives reload, two quick presses
   making two adds, per-session carts, and a page complete with JavaScript
   disabled.
5. **Negative controls**, `scripts/bench_contract_mutations.py`: a store
   whose count is wrong, whose sessions share a cart, or whose Add adds two
   must fail the contract, each in the stack it is written in.

## Finding: the Pleris store declares what its runtime does not do

`add_to_cart` declares
`optimistic Cart(current_session()) as cart => Carts.with_line(..)` and
`idempotent_by InteractionId`. `pw check` checks both (ADR-0024, ADR-0025,
ADR-0089, ADR-0105). No backend executes either:

- no backend emits code for an `optimistic` clause: the compiled handler
  module, the own-renderer runtime and the Marko adapter contain none, and
  the count moves only when the server's patch arrives;
- the development server's command path takes no interaction id, so a
  retried request adds twice. `runtime/pw-resource` has idempotency keys
  (E4), and the E10 command path does not use them.

So the **shared contract excludes both**, and the Next and SvelteKit stores do
not implement them either. Neither is claimed for any stack. They are what
benchmark tasks T01 (optimistic update with rollback) and T08 (no duplicate
submissions) ask for, and in Pleris those tasks currently need runtime work,
not source changes. Recorded in KNOWN_LIMITATIONS.

## Found while building

The first Next and SvelteKit stores created the session cookie on the first
write. Two presses before any cookie arrived made two sessions, and the
second cookie won: the SvelteKit store counted 1 for two presses. Next
passed only because it runs one Server Action at a time. Both now issue the
session on the first response, as the Pleris server does. This is the class
of defect the benchmark exists to count, and the contract caught it.

## Acceptance

`just e14-contract` (`docs/evidence/E14/contract.txt`): both baselines build
and typecheck, `pw check` passes the Pleris store, the contract passes on all
three stacks three times over, and every mutant is killed.

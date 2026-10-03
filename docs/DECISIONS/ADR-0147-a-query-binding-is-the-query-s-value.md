# ADR-0147: a query binding is the query's value, and a page that cannot be read is answered

Status: accepted under the owner's delegation of 2026-10-02. Date: 2026-10-02.
Milestone: E14, for T04. Two corrections, and what T04's orders needed.

## Context

T04 adds a state to an order, and its store shows the order's state with
`{#match order} {:Some(status)} {#match status} {:Placed} .. {/match}`.
Writing its baseline found two defects.

**A page's query binding had no type.** The typer types a `let` by its
written type, and `let order = query Order(..)` writes none. Every other
reader of such a binding saw its value:
- the page plan reads its parts' fields from the query's `Ok` value;
- the server gives that value;
- a loop over it typed its item through `Result`.

A `{#match}` over it typed nothing below it, so `status` had no type, and
`{:Placed}` was refused: "the subject's type is not known here". Loops were
also typed before arms, so a loop over an arm's binding had no item type
either (PW5016).

**A query that failed stopped the development server.** The server rendered
the store's page while it held the table of subscribers. A query failure
panicked there, which poisoned the table, so every later request failed too.
In T04 that is exactly what happens when the kitchen sets a state the
program's type does not have.

## Decision

1. **A page's query binding is the query's value**: the `Ok` value of its
   declared result, as the host gives it and every template read takes it.
   - A `{#match}` over it takes that value apart, and must cover every case
     (PW0305).
   - Taking its `Result` apart, `{:Ok}`/`{:Err}`, is refused (PW0608) until a
     page can show a query's failure (T10). Accepting it would check and then
     render wrong, since the host gives the `Ok` value.
2. **Loops and arms are typed together, until nothing more is learned.** A
   loop's list can be an arm's binding, and an arm's subject a loop's item,
   so neither order alone types both.
3. **A page that cannot be read is answered, and the server goes on.**
   - A document whose queries fail is answered `503`, with the reason.
   - A served page whose values fail after a command is sent a `reload`
     recovery, and reading it again answers why.
   - Nothing panics while holding the table of subscribers.
4. **The development server holds a session's order**, the benchmark's
   order data interface, as it holds a cart:
   - `store:data/orders#current` answers the order's status by its case;
   - the kitchen's hook `POST /bench/order?status=…` sets it, as every stack
     in the benchmark has one;
   - the development node grants `database.read<Orders>`.
5. **`pw-render --plan` renders nothing for a block a private query
   decides**, when the values do not give the query. That render is no
   session's, as ADR-0145 renders such a list empty. A block a shared query
   decides must be given its value.

## T04

The order feature is T04's setup patch on each stack; the canonical store is
unchanged. In Next.js and SvelteKit the baseline shows the order in each
framework's common idiom, a JSX conditional or an `{#if}` chain per state.
Pleris shows it with `{#match}`.

The plausible wrong fix adds the state, and forgets to show it:
- Next.js and SvelteKit build it, and the page shows nothing for a ready
  order; only the hidden test fails it.
- In Pleris, `pw check` refuses it: `` `{#match status}` does not cover
  `Ready` `` (PW0305).

## Acceptance

- `query_values.rs`:
  - a query binding is its value;
  - a match over it covers every case;
  - a loop over an arm's binding is typed;
  - a query's failure is not taken apart yet.
- Server tests, on the store with T04's own setup patch applied:
  - an order the kitchen sets is the query's value, per session;
  - a page whose values cannot be read is answered, and another session is
    served after it;
  - a served page whose values fail is told to read itself again.
- `plan_lists.rs`: a block a private query decides renders nothing for no
  session, and a shared one must be given.
- T04's four controls on all three stacks.
- `scripts/query_values_mutations.py`.

## Not claimed

- **A page that shows a query's failure** (T10).
- **An order a page changes**: the kitchen's hook is the benchmark's, and no
  command places or changes an order.

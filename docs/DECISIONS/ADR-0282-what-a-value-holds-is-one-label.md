# ADR-0282: what a value holds is one label

Status: accepted under the owner's delegation of 2026-10-02, on four
soundness findings of 2026-10-03, relayed 2026-10-08. Date: 2026-10-08.
Milestone: E14. Amends ADR-0118 (its fifth decision) and corrects ADR-0128.

## Context

- **Four findings, made read-only on 2026-10-03** at `29ebcf9` by the
  session "Project readiness for kiokun.com rewrite". Its message to the
  integrator expired unread. Each was reproduced at `d346c43` on 2026-10-08
  with new probes:
  1. **A secret kept in a cached fragment.** A `public query` answering
     `Secret<Payments>`, kept by a `partition public` materialization placed
     at the edge or in the browser, checked clean. The same value, rendered
     by a page with a shared cache, was refused (PW5001, PW5003).
  2. **A fragment built before any request from a session's data.**
     `placement build` with `depends_on Cart(current_session())` checked
     clean. A page placed at build that read it was refused (PW5002).
  3. **Placement and caching read different labels.** A page placed at
     build that read the session through a query of its own (`query
     MyCart() { Carts.current(current_session()) }`) checked clean. Reading
     `Cart(current_session())` itself was refused. Since ADR-0128 the cache
     rules see the session either way, but placement did not, in the
     checker or in the contract.
  4. **A placement diagnostic cut short.** PW5002 said "cannot run in any
     world: it requires" and nothing after it, when a label alone ruled out
     every world.
- **One cause behind them.** Three derivations of what a value holds fed
  different rules, and a fragment's `depends_on` fed the graph alone:
  - **placement and the contract** read each declaration's declared
    keyword, joined with the keywords of what it reads;
  - **PW5001 and PW5004** read what it observes: that, its result, what it
    reads through what it calls, and what it is given (ADR-0118,
    ADR-0128);
  - **PW5101** read what a resource observes with every secret left out
    (ADR-0118's fifth decision), so a secret a query *answers* went with
    the secrets a query only *uses*.
- **ADR-0128 was wrong on one point.** It said that "effects already keep a
  session's value off the build world". They do not: `session.read`
  declares no placement, so every world grants it, build included. Only a
  label keeps a session off build.

## Decision

1. **A secret a declaration answers is held; a secret it only uses is
   not.** A declaration answers a secret through its result type, or
   through what its body's value carries (ADR-0129's summary). A query that
   uses a secret as a key, to fetch something public, answers a public
   value, as ADR-0085 rules. This amends ADR-0118's fifth decision, which
   left out every secret.
2. **What a value holds** (`Reads::answers`) is everything the declaration
   observes, its secrets left out, joined with each secret it answers. It
   is what PW5101 reads of a dependency, so a shared fragment of a secret is
   refused.
3. **What a declaration holds where it runs** (`Reads::holds`) is what its
   value holds, joined with each secret that what it reads answers.
   Placement reads it, in the checker and in the contract alike, so the
   two cannot disagree.
   - A page placed at build that reads the session through a query of its
     own is refused, and its contract does not allow build.
   - A fragment holding a secret runs only at the origin.
4. **A materialization reads what it depends on.** Its `depends_on` targets
   are among its reads, for every label, so a fragment of a session's cart
   placed at build is refused.
5. **A placement refusal says what rules each world out**, as the solver
   found it: an effect a world cannot grant, as written, and a label a
   world may not hold. For example: "`BuiltSession` cannot run in any
   world: it holds Session<SessionId>".

## Acceptance

- **`compiler/pw-core/tests/held_where_it_runs.rs`**, one test a finding,
  each with its controls:
  - a secret a query answers is kept by no shared fragment; a fragment of
    a query that uses a secret as a key is;
  - a fragment holding a secret runs only at the origin, and one of what a
    key fetched runs at the edge;
  - a fragment built before any request holds no session, and one at the
    origin may;
  - a page reading the session through a query of its own, or itself, is
    not built, and at the origin it is;
  - the contract places it where the checker does;
  - a placement refusal names the label that rules each world out.
- **ADR-0118's control stays clean**, in `reads_through_calls.rs`: a query
  that signs with a key and answers what it fetched, kept by a shared
  fragment at the edge.
- **The rejected corpus gains three exhibits** (C18): the secret kept in a
  shared fragment, the fragment of a session built at build, and the
  session read through a page's own query at build.
- **`scripts/held_labels_mutations.py`**: each piece undone fails the tests.
  Its own run is here; the scripts this touches run on CI (ADR-0281).

## Not claimed

- **A placement listing several worlds pins none.** A declaration's
  `placement` names one world, and a list is read as no pin. No program
  writes one; queued.
- **A fragment's placement is read by no runtime.** The materializer keeps
  it and never reads it, so a fragment runs where the host runs it, at the
  origin. This ruling holds what a program declares to what it holds; it
  does not move work to the edge.
- **A secret laundered through a materialization's body.** ADR-0129's
  summaries cover functions, queries, commands, subscriptions and
  resources, and not a materialization's body.

# ADR-0089: a policy's value is one its domain has

Status: accepted under the owner's instruction of 2026-09-26 ("keep working
until its perfect"). Date: 2026-09-26. Milestone: E10 (charter §9.2, §7.8).

## Context

`crate::policy` says what each policy head's value is: a word from a closed
set, a duration, a world, a parameter, a type, or an operator with a
signature. It says an unlisted word "is an error, not an extension point".
Nothing held a value to the table. Each reader of a clause decided alone what
it meant, by an exact match, a prefix or a substring.

The architect ruled on 2026-08-07, on an unknown policy head: "Pleris may
reject authored semantics; it must never silently erase them." A value its
domain does not have is the same failure one level down. On 2026-09-26, at
a187605, each of these checked:
- **`cache Shared`** on R-004's page, the charter's canonical privacy
  defect: a session's cart in a shared cache. The privacy rule read it as no
  shared cache, and the manifest as no cache.
- **`placement originn`**. `check::declared_world` found no world, and the
  placement was derived from effects instead of pinned.
- **`freshness 30.secondz`**, **`consistency snapshott`** and **`key
  nope`**, a cache key naming no parameter.
- **`retry nope(max = 3)`**, an operator `retry` does not have, and
  **`retry transport_only(maxx = 2)`**, `max = "two"`, and
  `bounded_exponential(jitter = true)` with no bound. The manifest read the
  last three as no retry at all.
- **`idempotent_by InteractionId`** where nothing imports `InteractionId`.
  The store's two commands wrote it, as did A-010 and four generality
  fixtures.

Three readers matched loosely enough to be bypassed:
- **PW0102** refuses a stale session read, and read a freshness by its
  first character: `freshness 05.seconds` began with `0`, and was five
  seconds stale.
- **PW5004** refuses a shared cache keyed without its partition, and looked
  for the partition's name in the key's text. A parameter called `username`
  contained `user`.
- **PW0312** lets only `transport_only` retry a command that is not
  idempotent, and read `transport_onlyish(..)` by its prefix.

The corpus and the table also disagreed. Two witnesses wrote `concurrency
latest_wins`, a word neither the table nor the manifest has. A valid
neighbour wrote `retry fixed(max = 3)`, an operator the table lacked. R-027
expects PW0325's repair to offer `on_key_change supersede` and `keep`,
which the table lacked.

## Decision

- **A policy's value is one its domain has** (PW0335):
  - a word the head lists; `impact`'s may carry a `when` condition, which is
    the ontology's (PW5204);
  - a duration, read once by `policy::duration`, which the manifest reads
    through;
  - a world, or a list of worlds;
  - a parameter of the declaration, or for a cache key a partition it
    separates (`privacy::Label::PARTITIONS`);
  - a type visible here, or a declaration visible here;
  - an operator the head has, with its arguments by name, each once, each
    of its kind, the required ones present. `max` is a count from 1 and
    `jitter` is `true` or `false`. `retry none` and `retry forever` stay
    words, and PW0313 refuses the second;
  - no value for a flag.
- **Each reader reads the value exactly**:
  - PW0102 reads the duration;
  - PW5004 reads the key's items;
  - PW0312 reads which operator is applied;
  - PW0100, PW0101, PW0200, PW0313, PW2004 and the graph's
    `included_in_key` compare whole words;
  - the placement rule reads one world, and a list pins none, as
    `check::declared_world` already read it.
- **The table gains what the corpus writes and the manifest reads**:
  - `concurrency parallel`, the manifest's second mode;
  - `retry fixed`;
  - `on_key_change supersede` and `keep`;
  - `scope application`, which R-028 and R-039 are refused for.
- **The corpus is corrected**. The store, A-010 and four witnesses import
  `InteractionId`. The two `stale_key_policy` witnesses write `concurrency
  parallel`, which keeps the dimension "a different concurrency policy".

**(ruling needed)**
- whether `latest_wins` is a concurrency mode;
- whether `fixed` is a retry strategy (it is registered, because a valid
  program writes it);
- whether `keep` and `supersede` mean anything a runtime does. Nothing reads
  `on_key_change` beyond its presence.

## Acceptance

- **`compiler/pw-core/tests/policy_values.rs`**, 10 tests, each with
  controls. All 10 fail at a187605, the commit before.
- **The corrected fixtures** are the only ones that change; every rejected,
  rule and generality fixture's diagnostics are as before. The store, kiokun
  and the accepted corpus check clean. Their artifacts are byte-identical,
  apart from ADR-0058's two handlers.
- **Mutation controls:** `scripts/policy_value_mutations.py`,
  `just e10-policy-values`, 14 mutants.

## Correction, 2026-09-26

A type a policy names was looked up among the program's declarations alone.
So `idempotent_by Int` was refused as "no type visible here", although
`Int` is a type. It is resolved as a written type is now, so the language's
own types count too. `tests/policy_values.rs` states it (`idempotent_by
Int` checks clean), and the mutation controls gain a fifteenth mutant: a
type that must be a declaration's.

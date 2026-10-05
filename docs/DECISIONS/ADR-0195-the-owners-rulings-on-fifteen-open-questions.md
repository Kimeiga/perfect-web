# ADR-0195: the owner's rulings on fifteen open questions

Status: accepted. The rulings are the owner's, relayed on 2026-10-05 by the
session "Web Pleris capabilities and limitations", which the owner asked to
rule on KNOWN_LIMITATIONS' open "(ruling needed)" items. The record, each
check against a primary source, and the order are this session's, under the
owner's delegation of 2026-10-02. Date: 2026-10-05. Milestone: E14.

## Context

KNOWN_LIMITATIONS and 56 ADRs held 109 marks reading "(ruling needed)" or
"offered for reversal". Most were defaults chosen before the owner delegated
decisions on 2026-10-02. The owner ruled on fifteen of them. The relay asked
for each to be recorded and implemented, and overruled, with the reason
written down, where research finds it wrong.

## Decision

Each ruling is recorded below as given. Where a source refines one, the
refinement is stated. None is overruled. Each is built in its own later
ADR, which carries its own research and acceptance. Until then, the
limitation it rules on stands as KNOWN_LIMITATIONS states it.

### Language

1. **A record literal's shorthand, `P { x }`** (ADR-0063), takes Rust's
   rule. A type name followed by `{` opens a record literal everywhere,
   except in the head of `if`, `while`, `for` and `match`. There a record
   must be parenthesized, and `TypeName {` gets a diagnostic offering the
   parentheses.
   - **Checked:** the Rust Reference's `if` conditions are "Expression
     except StructExpression", for this very ambiguity.
2. **A binding and a constructor are told apart by capitalization**
   (ADR-0038), as in Haskell, OCaml and Elm:
   - a case name starts uppercase;
   - a pattern name starting lowercase, or `_`, is always a binding;
   - one starting uppercase is always a case, resolved in the scrutinee's
     type first, and an error, never a binding, where that type lacks it.

   **Checked:**
   - The Haskell 2010 report (§2.4) separates variable identifiers, which
     begin lowercase, from constructor identifiers, which begin uppercase.
   - rustc's E0170 is the bug class this removes: a pattern named like a
     variant binds a new variable that matches everything.
   - None of the 420 `.pw` files under `examples/` and `compiler/` names a
     case in lowercase.
3. **Only a word that can begin a statement or an expression in a body is
   reserved** (PW0013, ADR-0041): `let`, `if`, `match`, `for`, `return`,
   `use`, `query`, `signal`, and their kind. Every other keyword is
   contextual, as `session` already is as a parameter's name, and the
   diagnostic suggests a name.
   - **Checked:** the Rust Reference's weak keywords (`union`,
     `macro_rules`, `raw`, `safe`) are special only in their context.
4. **A declared function's result that its arguments do not fix** is typed
   bidirectionally, from what is expected of it: an annotation, a
   parameter, a return. If nothing fixes it, it is refused with "type
   annotation needed", as Rust (E0282) and Swift refuse it. It is never left
   undecided.
5. **A bare case with a payload, `Circle(3)`** (PW0021), takes the rule a
   payload-free case has. It is accepted where the expected type has the
   case, or where exactly one visible type does. Otherwise it is refused,
   naming the candidates.
6. **`%` on a `Float`** (ADR-0039) is Euclidean, as an `Int`'s is, and a
   zero divisor traps, as an `Int`'s does.
   - **Refined by the source:** Rust's `f64::rem_euclid` documents that
     rounding can make the result equal `|b|` when `a` is negative and much
     smaller in magnitude, and that the result is the rounded
     infinite-precision one.
   - So Pleris's `%` on a `Float` is exactly that: `r = a % b`, then
     `r + |b|` where `r < 0`. That is one rounding, written alike in Rust
     and in JavaScript, so both backends agree on every input, NaN and the
     infinities included.
   - **A `Float`'s text is ECMAScript's `Number::toString`** (ECMA-262
     §6.1.6.1.20), on the server and in the browser alike, so resumed text
     is byte-identical. It is built from Rust's shortest round-trip digits
     by ECMA-262's algorithm, with no new crate; the relay prefers this.
   - Its acceptance, as the relay set it: a differential test against
     Node, and the three Playwright engines where they can run it, over a
     large random set of doubles. It covers the edges: −0, subnormals, the
     1e21 and 1e−7 boundaries, integers near 2^53, and NaN and the
     infinities where they reach text. A mutant must fail it.
   - Formatting for a locale is a separate `format`, later, with i18n.
7. **`return` as an expression** (ADR-0038) is not built. It stays a
   statement, and the `=> return e` arm and `?` cover its uses.

### Data and the resource graph

8. **A cache key covers every parameter the entry's value depends on**
   (ADR-0107), directly or through control flow.
   - That is computed by the information-flow labels already in the
     compiler (`labels.rs`, ADR-0129): each parameter is given a label, and
     the labels are carried to the result.
   - A host call's result depends on all its arguments, so it stays sound
     at the boundary, and a trace returning the unit value contributes
     nothing.
   - The conservative rule stands until this lands.
9. **A clause's key carries the labels of what built it** (ADR-0088).
   - A secret in `emits` or `invalidates` is refused.
   - A key labelled by the session is allowed only where every listener is
     scoped to the session or private.
   - What a key's expression performs, `current_session()`'s
     `session.read`, is the declaration's effect.
10. **The other two of ADR-0088:**
    - A bare `invalidates Cart` stays refused. Every entry is written out
      as `Cart(_)`, as `InventoryChanged(id, _)` is.
    - A materialization may read another. The build refuses a cycle, and
      invalidation propagates transitively. A feed needs this.
11. **An event's key from a command's own arguments** (ADR-0104). The
    compiled command evaluates its `emits` keys and returns its typed
    events with its result.
    - The host writes the events in the same transaction as the writes: the
      transactional outbox, by which, in microservices.io's words, messages
      are sent "if and only if the database transaction commits". The
      server never evaluates a key's text.
    - For a source without transactions, `emits` is refused unless the
      source declares a change feed. That is a data source's guarantees, the
      owner's priority 21.

### The browser

12. **A handler that is not resumable** is mostly settled by ADR-0134, and
    KNOWN_LIMITATIONS is stale there. What remains: a named function bound
    to `on:`, as in `on:submit={save}`, means `e => save(e)`. It is checked
    against the event's type, compiled as a lambda is, and held to ADR-0157.
    Until it is built, it is refused when the program is checked, not only
    when it is built.
13. **The head's elements** are mostly settled by ADR-0186 (`http-equiv` is
    the host's) and ADR-0189 (relations the body allows), and
    KNOWN_LIMITATIONS is stale there too. What remains:
    - a `<link>`'s `href` is a literal or a build's asset;
    - a cross-origin `href` needs its origin declared by the program;
    - the host's Content-Security-Policy is generated from that same list.
14. **A speculation shows only what the page already knows, and never
    invents a value** (ADR-0122).
    - The speculated new line takes the price the page shows, from the
      captured `MenuItem`. The command still takes no price, and the
      server's answer replaces the line.
    - Where a speculation cannot know a field, its type says so, as an
      `Option`. It is never a made-up zero.

### Evidence

15. **E7's gate item 8 means jank the page causes** (E7-G8).
    - A long animation frame counts against the page when its `scripts`
      name the page's own sources, or its `blockingDuration` comes from the
      page's tasks.
    - A frame with no attribution is recorded and reported beside the count,
      and does not fail it. The recorded ones had `scripts: []`, 4 to 6 ms
      blocking, and WindowServer busy.
    - The gate passes only with no attributed frame in every one of at
      least eight runs, beside a control, a deliberate thrash, that fails.
    - **Checked:** the Long Animation Frames draft gives each frame
      `PerformanceScriptTiming` entries with a `sourceURL`, and a
      `blockingDuration` summed from tasks over 50 ms.
    - Its public wording: "no interaction makes a long frame of its own".

## Order

Kept behind the owner's app layer of 2026-10-04 (NEXT 20 to 22), except
where a ruling fixes a wrong value or belongs to that work:

1. **First, for correctness:**
   - 14, an invented zero;
   - 12 and 13's stale entries;
   - 2, the capitalization rule, which no file breaks;
   - 3, contextual keywords;
   - 5, a bare case with a payload.
2. **With the app layer:**
   - 10's materialization chains, for the feed;
   - 11's outbox, with a data source's guarantees;
   - 8 and 9, with the cache.
3. **Then:** 1, 4, 6, 7 (nothing to build) and 15.
4. **A sweep of the other "(ruling needed)" marks**, each confirmed or
   overruled, with KNOWN_LIMITATIONS cleaned of what is stale.

## Alternatives

- **Rule on them one at a time as each is built.** The owner's rulings
  exist now. Recording them first means a later ADR implements a recorded
  ruling, rather than restating it.
- **Implement before the app layer.** The owner put the app layer first on
  2026-10-04. Only the rulings that fix a wrong value, or belong to that
  work, move ahead of it.

## Acceptance

None of its own. This ADR records rulings. Each implementing ADR has its
own tests, mutation controls and evidence, and names the ruling it builds.

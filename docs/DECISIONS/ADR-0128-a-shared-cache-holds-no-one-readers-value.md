# ADR-0128: a shared cache holds no one reader's value, whatever its declaration says

Status: accepted under the owner's delegation of 2026-10-02. Date: 2026-10-02.
Milestone: E14 (T12); charter §7.8, §14 M5, §15.2. Settles the cache half of
the ruling ADR-0085 left open; the value half is listed under "Found while
building" below.

## Context

Charter §7.8's first must-fail example is `SharedCache<Cart@Session>`. The
charter says it three more times: "A shared cache may contain only `Public`
values" (§17's diagnostic), the cart is "Session/User → private cache" and
device location is "never shared" (§15.2).

At `61438b4` it passed in two shapes.

1. **The keyword said public.** T12's plausible wrong fix (ADR-0124) checked
   clean:

   ```pleris
   public query Cart(session: Session<SessionId>) -> Result<Cart, CartError>
       cache shared
       key   session
   { Carts.current(session) }
   ```

   So did the same query with no visibility keyword at all. The cache rules
   read the declaration's keyword and its callees' keywords (PW5001) and the
   result types of what it reads (PW5004). Nothing read a declaration's own
   parameters. And `Carts.current`'s parameter states its label, which
   ADR-0085 treats as a contract that keeps the label out of the result.
2. **The key named the reader.** PW5004 required a shared cache's key to
   name each partition its value depends on, and accepted the value once it
   did. The current user's menu, keyed by `user`, was the generality matrix's
   *valid neighbour*. PW5001's own repair advised partitioning the shared
   cache by session.

The two NARROW generality witnesses, known gaps since corpus C3
(`private_in_shared_materialization/slips-through-{helper,branch}.pw`), were
the first shape seen from a fragment: a `public query` given the session.

## Decision

1. **A declaration observes what it is given.** Its own parameters' labels
   join what it observes (`Reads`), beside what it reads through what it
   calls (ADR-0118) and its result type. A query is a function of its
   arguments. A secret a parameter states is left out: it is a key the
   declaration uses, as ADR-0118 §5 leaves out a secret a query reads.
2. **Every cache rule reads that whole label** (`Reads::observed`), PW5001
   and PW5004 alike. A keyword can say less than a value holds; it can no
   longer lower what the value holds.
3. **No key makes one reader's value shareable.** A value carrying Session,
   User or Device is refused a shared cache whatever its key
   (`Label::safe_in_shared_cache`).
   - An entry keyed by its reader is hit only by that reader, so the shared
     cache shares nothing.
   - It holds the reader's value and identifier where every reader's is
     held. In a deployment, that is a CDN: someone else's infrastructure and
     logs.
   - Its place is the private partition (charter §15.2), which `pw-resource`
     already scopes per session (ADR-0127).
   - A secret is never cached, as before.
4. **A tenant's value is the one a shared cache may carry by its key** (R-005;
   charter §14 M5's "cross-tenant cache key omission"). A tenant's readers
   share it. PW5004 checks only that now. A key item naming a parameter whose
   type is the tenant counts: `key store, org` with `org:
   Organization<OrganizationId>`, as surely as `key store, organization`.
5. **PW5001's repair is the private cache**, or no cache for a secret. The
   advice to partition a shared cache by session is withdrawn. When a
   parameter is the source, the cause chain names it, and the related span
   is where it is declared.

## Alternatives

- **A declared label as an upper bound (Jif's rule).** Report `public query
  Cart` because its keyword says public and its value is a session's.
  Rejected as this fix. Deleting the keyword evades it. And once the
  use-site rules read the whole label they see every consequence, so a
  declaration-site error would be a second diagnostic for one defect
  (ADR-0112). It stays a candidate for the value half of ADR-0085, where the
  question is a function signature's contract.
- **Keep keyed sharing for every partition**, the reading the generality
  neighbour encoded. Rejected:
  - it contradicts §7.8, §15.2 and §17;
  - T12's wrong fix is exactly a keyed session value. It runs correctly on
    this server only because `pw-resource` keys an entry by its arguments.
    An HTTP cache keyed by URL would serve one session's cart to another;
  - it is also the hidden test's failure mode on both frameworks.
- **Refuse in PW5004 instead.** Rejected: PW5001's invariant is "a value that
  is not public cannot live in a shared cache", and PW5004's is the key's
  completeness. The defect is the first.

## Consequences

- **Generality is 31 of 31 invariants, with no known narrow invariant.**
  Since C3 it was 30 of 31, with 1 known narrow; the test asserts the figure.
  - The two known gaps are closed and promoted (`helper.pw`, `branch.pw`).
    Neither dependency declares a cache, so only what it is given decides
    them.
  - `cache_key_omits_partition`'s witnesses now concern the tenant: a
    helper, a key naming another partition, and an organization-keyed
    neighbour.
  - Its former user-keyed neighbour, and T12's shape, are
    `private_in_shared_cache` witnesses (`keyed-by-reader.pw`,
    `relabelled.pw`).
- No rejected fixture's diagnostics change. The accepted corpus, the store
  and kiokun check clean.
- Four earlier tests expected the reading this overturns, a reader's value
  in a shared cache answered by PW5004's question about the key:
  - ADR-0112's `a_session_read_is_one_however_it_is_imported`;
  - ADR-0118's `a_shared_cache_reads_through_a_helper` and
    `a_shared_page_reads_through_a_public_query`.
  Each expects PW5001 now, for the same declarations.
  ADR-0089's `a_key_separates_a_partition_by_naming_it` moved to the
  tenant: a key item `organization_name` still does not name the
  organization.
- Two mutation controls were re-anchored to the new lines, and each is still
  killed:
  - `policy_value_mutations.py`'s "a key's partition is found in its text";
  - `privacy_resolution_mutations.py`'s "PW5004 speaks where PW5001 does".

## Acceptance

- **`compiler/pw-core/tests/shared_cache_holds_no_readers_value.rs`, 5 tests.**
  - Each states a defect: the session-given cart under `public`, no keyword,
    and `session`; the user keyed by `user`, and a session keyed by
    `session` through a helper; a fragment of a query given the session.
  - Controls: `cache private`; a tenant keyed by `organization` or by its
    parameter; a fragment `partition private`.
  - A fifth test: a session's value in a tenant's shared cache is one defect,
    reported once by PW5001 and not again by PW5004.
  - Four of the five fail at `61438b4`. The third, the tenant's controls,
    holds before and after.
- **The generality witnesses.** All four new or promoted witnesses go
  uncaught at `61438b4`.
- **Mutation controls:** `scripts/shared_cache_mutations.py`, `just
  e14-shared-cache`, 5 mutants, one for each piece of the decision above.
  Evidence: `docs/evidence/E14/shared-cache.txt`.
- **T12's controls on Pleris:** recorded in
  `docs/evidence/E14/harness-T12.txt`.

## Found while building

The ruling's other half is about values. Each of these checks clean at
`61438b4`, written as probes beside the cache shapes above:

- **A labelled parameter launders a secret.** `fn shown(key:
  Secret<Payments>) -> String { "{key}" }`, then `log.public(shown(token))`.
  The result is the declaration's, by ADR-0085's contract.
- **A helper launders what it reads itself.** `fn shown() -> String { let t =
  secrets.payments(); "{t}" }`, logged publicly. A call's value label reads
  the callee's signature, not its body. ADR-0118 fixed this for the coarse
  labels the cache rules read, and not for values.
- **A branch's condition is not joined.** `let bit = if token == token {
  "yes" } else { "no" }`, logged publicly. Neither is a sink inside the
  branch: `if token == token { log.public("yes") }`.
- **`log<Public>` refuses only a secret.** A session id logged publicly
  passes. The platform's own definition says a public log's contents "may be
  read by anyone".
- **PW5003 reads only a label's first restriction.** A value that is both a
  session's and a secret is ordered Session first, and is not reported as a
  secret rendered into markup.

They are the next ruling. Each needs a function's body to be checked against
what its signature says comes out, which is the question ADR-0085 left open.

## Not claimed

- **Placement still reads the declared label**, not the observed one.
  Effects already keep a session's value off the build world, so no case is
  known where they differ.
- **A key may still name a parameter holding a secret**, because PW0336
  requires every argument the body reads to be in the key. Nothing in the
  corpus passes a query a secret.

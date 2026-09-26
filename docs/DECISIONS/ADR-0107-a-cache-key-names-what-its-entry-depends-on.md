# ADR-0107: a cache key names each parameter its entry depends on

Status: accepted under the owner's instruction of 2026-09-26 ("keep working
until its perfect"). Date: 2026-09-26. Milestone: E10 (charter §9.2, §9.5).

## Context

Charter §9.2 lists `key` among what a resource declaration expresses, and
§9.5 deduplicates requests. A query's cached entry is shared by every call
whose `key` is equal. A subscription's stream is shared by every call whose
`dedupe_by` is.

PW5004 holds a shared key to the privacy partitions its value depends on,
such as the user and the tenant (R-005). PW0335 holds each key component to
a parameter or a partition. Nothing held a key to the parameters the entry
depends on. On 2026-09-26, at 745cef0, this checked:

```pleris
public query Other(id: StoreId, other: StoreId) -> Result<Store, StoreError>
    cache shared
    key   id
{
    Stores.get(other)
}
```

`Other(1, 2)` and `Other(1, 3)` have one key, `1`, and share one entry. The
second call is given the first's store.

## Decision

**A cache key names each parameter its entry depends on** (PW0336). A query,
subscription or resource that declares `key` or `dedupe_by` names in it
every parameter its body reads. A parameter is read where a name anywhere
in the body resolves to it lexically (`crate::lexical`), on any branch. A
parameter the body never reads may be left out.

The diagnostic underlines the clause and shows the first read. A clause
PW0335 refuses, one naming neither a parameter nor a partition, is set aside:
what the key meant is not known, and one defect gets one diagnostic.

**(ruling needed)** A read is any use of the parameter. A parameter read
only for an effect, such as a trace, has to be keyed too, although the
entry's value does not depend on it. Telling the two apart needs the flow
of values to the result, which the checker does not trace.

## Acceptance

- **`compiler/pw-core/tests/keys_cover_reads.rs`**, 4 tests. Each fails at
  0cd4165, the code at 745cef0. The cases, each with its control:
  - a key leaving out a parameter the body reads, against a key naming it,
    a body reading only what the key names, and a key naming nothing
    (PW0335 alone);
  - a read on one branch;
  - a subscription's `dedupe_by`;
  - the underline and the read shown.
- **Corpus.** No rejected, rule or generality fixture's diagnostics change.
  The store, kiokun and the accepted corpus check clean: every keyed
  declaration in them keys what its body reads.
- **Mutation controls:** `scripts/key_read_mutations.py`,
  `just e10-keys-cover-reads`, 4 mutants.

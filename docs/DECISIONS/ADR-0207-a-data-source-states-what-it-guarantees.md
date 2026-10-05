# ADR-0207: a data source states what it guarantees, and nothing asks it for more

Status: accepted under the owner's delegation of 2026-10-02. It builds
ADR-0195's ruling 11's half that is a source's guarantees. Date: 2026-10-05.
Milestone: E14, the app layer: the owner's priority 21 (NEXT), "a data
source states what it guarantees".

## Context

- **A program asks things of its data, and nothing held them to it.**
  - A query states `consistency snapshot`, `read_your_writes`, `strong` or
    `eventual`: what a reader sees.
  - A command states `transaction serializable` (A-005, A-010), `emits` events
    "in the same transaction as the state change" (ADR-0007), and is
    `idempotent_by` an interaction (ADR-0121).
  - Each held because the host's one database is SQLite: serializable
    transactions, a read of the latest commit, and the outbox writing a
    command's events with its writes (ADR-0005, ADR-0019). A program whose
    data lives elsewhere would ask the same, and get less, silently.
- **What other databases give**, read in each one's documentation:
  - SQLite: "all transactions in SQLite show 'serializable' isolation"
    between connections; in WAL mode a reader sees a snapshot.
  - PostgreSQL: Read Committed by default; its Repeatable Read is snapshot
    isolation, and its Serializable is Serializable Snapshot Isolation.
  - MongoDB: change streams "are available for replica sets and sharded
    clusters", not a standalone server, which has no multi-document
    transactions either.
  - DynamoDB: `TransactWriteItems` is all or nothing; a `Query` is read
    committed against a transaction; a read is eventually consistent unless
    it asks to be strong; and a stream's records of one transaction "might
    appear at different times".
  - Cloudflare D1 with read replicas: sequential consistency through its
    Sessions API (a reader's own writes, monotonic reads), and eventual
    without a session.
  - So what a source gives depends on the database and on how it is
    deployed. The program has to state it.
- **The vocabulary is the standard one.** Jepsen's models order the
  isolation levels, serializable above snapshot isolation above read
  committed, and put a reader's own writes among session guarantees, beside
  and apart from a snapshot. Berenson et al. (SIGMOD 1995) show snapshot
  isolation allowing write skew, which serializability prevents.
- **The outbox** (microservices.io): messages "are guaranteed to be sent if
  and only if the database transaction commits", relayed from the outbox or
  from the database's own log.

## Decision

### 1. A source

```
source Search
    holds        Listing
    transactions none
    reads        eventual
    changes      none
```

- **`holds`**: the resources it holds, as the program's effects name them:
  `database.read<Listing>` names `Listing`. Each is a type or a module the
  source's module can see, and is held by one source.
- **`transactions`**: the isolation a command's writes commit with:
  `serializable`, `snapshot`, `read_committed`, or `none`, each write alone.
- **`reads`**: what its reads may promise, one or more: `strong`,
  `snapshot`, `read_your_writes`, `eventual`.
- **`changes`**: `feed` where it tells what changed once committed, `none`
  otherwise.
- **A clause left out guarantees nothing.** A source holds nothing without
  `holds`, which is refused.
- **A resource no source holds is the host's own database's**: serializable,
  read strongly, a command's events committed with its writes. Every program
  written so far is unchanged, and the store states its own (`StoreData`).

### 2. Nothing asks a source for more than it gives

- **A query's `consistency`** is given by each source it reads: `strong`
  gives every promise, and every source gives `eventual` (PW0344).
- **A command's writes are in one source**, one transaction. A resource no
  source holds counts as the host's database's (PW0345).
- **A command's `transaction`** is at most its source's `transactions`, in
  the order of what each prevents (PW0346).
- **A command's `emits`** needs a transaction its events commit in, the
  outbox, or the source's feed of its changes (PW0347).
- **A command's `idempotent_by`** needs a transaction its record of the
  interaction commits in. A feed tells of a commit after it, and is none
  (PW0348).
- **A source holds what the program's effects name**, each resource once
  (PW0349).
- **What a declaration reads and writes is its inferred row**, through
  everything it calls, as ADR-0101 reads it.

## Alternatives

- **Infer a source's guarantees from its driver.** A guarantee depends on
  the deployment: one PostgreSQL is serializable and another read
  committed; a MongoDB has change streams on a replica set and none alone.
- **One ordered word for `reads`.** A reader's own writes and a snapshot are
  apart: D1 with sessions gives both and is not strong, and a lagging
  replica gives a snapshot and not a reader's own writes.
- **A `source` named in each effect**, `database.read<Search.Listing>`. The
  resource is what the program reads; which database holds it is the
  deployment's, stated once.
- **Require a source for every resource.** Every program written so far
  runs on the host's database, which gives every guarantee. Stating it
  everywhere adds nothing a reader can check, and the benchmark's store is
  frozen (ADR-0156).

## Acceptance

Recorded by `just e14-data-sources` in `docs/evidence/E14/data-sources.txt`:

- **`compiler/pw-core/tests/data_sources.rs`**, 9 tests. Eight are over a
  search index that commits each write alone and reads eventually: a
  program asking what it gives checks; each rule refused and its control
  accepted; each clause's words.
- **The store states its source**, `examples/lib/StoreData.pw`, and checks
  unchanged. Declared as the search index is, it is refused 21 times: the
  six queries asking more than `eventual` (not `Recommendations`), the
  three commands asking a serializable transaction, and each of the six
  commands for its events and for its idempotency.
- **`scripts/data_sources_mutations.py`**: 17 mutants.

## Not claimed

- **That a deployment gives what its program states.** A host does not yet
  compare a source's clauses with what its database provides; a contract
  carrying them is the next step.
- **A command's events computed by the command**, which ruling 11's other
  half builds: the server evaluates only `current_session()` (ADR-0104).
- **Several writes to a source without transactions**: a command that makes
  two is not atomic, and the compiler does not count them.
- **Reads across sources in one command**, which no transaction makes one
  snapshot.

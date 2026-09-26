# ADR-0112: a call's privacy is the declaration it resolves to

Status: accepted under the owner's instruction of 2026-09-26 ("keep working
until its perfect"). Date: 2026-09-26. Milestone: E10 (charter §7.8;
`docs/RISK_QUEUE.md` 34).

## Context

Two privacy rules read what a call's callee is:
- `body_label` joins the labels of what a body calls. The label is a
  signature's, made from its type: `secrets.payments()` returns
  `Secret<Payments>`, and `context.current_session()` returns
  `Session<SessionId>`. It decides whether PW5003 (a secret rendered into
  markup) is asked at all, and what PW5004 (a shared cache key omitting a
  partition) requires.
- `sink_level` reads the privacy level a logging call declares, for PW5006.

Both looked the callee up by its fully qualified spelling (`sigs.by_path`).
A bare name the unit imports is not a fully qualified spelling, so it found
nothing. On 2026-09-26, at 8992021, each of these passed `pw check`, and
each is caught written the other way:
- R-003 with `import secrets.{ payments }` and `payments()`: a payments key
  rendered into markup (PW5003);
- R-006 with `import log.{ public }` and `public(..)`: a payments token
  written to a public log (PW5006);
- a `public query` cached `shared` and keyed by nothing, reading
  `Carts.current(current_session())`: every session's cart in one entry
  (PW5004).

`sink_level` said why it read by path: resolving `public` as "the one
declaration spelled `public`" would make the rule depend on no other module
having one. That is right, and resolution is not that. The unit's imports say
which `public` a bare `public` is.

## Decision

**A call's privacy is the declaration it resolves to.** Both rules find a
callee's signature through the unit, `Inference::called_from`, which reads
its own declarations and imports and a qualified path whole. They fall back
to the fully qualified spelling. A function of the module's own named
`public` is its own and not the log's sink.

**One defect, one diagnostic.** A page reading a `session query` in a shared
cache is PW5001's, whose repairs include partitioning the cache. Written
with `context.current_session()`, it was PW5004's as well. PW5004 now stays
silent on a value PW5001 refuses. R-004 and four `private_in_shared_cache`
witnesses read the session by a bare name and would have been reported twice.

## Acceptance

- **`compiler/pw-core/tests/privacy_by_resolution.rs`**, 4 tests. Each fails
  at a3ce1ca, the code at 8992021:
  - a secret, a sink and a session read, each written both ways. The sink
    has a control: a module's own `public` is not the sink;
  - a page PW5001 refuses a shared cache, reported once whichever way the
    session is read.
- **Corpus.** No rejected, rule or generality fixture's diagnostics change in
  the harnesses. The store, kiokun and the accepted corpus check clean.
- **Mutation controls:** `scripts/privacy_resolution_mutations.py`,
  `just e10-privacy-by-resolution`, 4 mutants.

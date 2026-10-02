# `private_in_shared_cache` — challenge dimensions

A headline P0 invariant: *private state cannot live in a shared cache*. The
fixture (R-004) puts a `Session`-labelled query result directly into a page
declared `cache shared`. Each file below changes one thing about how the
private value reaches the shared cache.

| dimension | file | why it is relevant |
|---|---|---|
| direct | `direct.pw` | the fixture's shape, as the baseline |
| rebinding | `rebound.pw` | the value is renamed before it is rendered |
| helper extraction | `via-helper.pw` | a declared function returns the private value |
| branch join | `branch-join.pw` | private on one path, public on the other |
| relabelled | `relabelled.pw` | T12's wrong fix: the query is declared `public` and given the session as a parameter. Caught since ADR-0128: a declaration observes what it is given |
| keyed by the reader | `keyed-by-reader.pw` | the user's value with the user in the key. `cache_key_omits_partition`'s valid neighbour until ADR-0128 ruled that no key makes one reader's value shareable |
| valid neighbour | `private-cache-is-fine.pw` | the same private value in a `cache private` page — a rule reacting to the LABEL alone, without the cache, would catch this |
| valid neighbour | `public-in-shared.pw` | the same shared cache with a public value — a rule reacting to the CACHE alone would catch this |

Two neighbours, because this invariant is a *conjunction*: private **and**
shared. A rule that fired on either half alone would pass the fixture and be
wrong, and only one neighbour each way can tell those apart.

**Not relevant.** *Cross-module* is already the fixture's shape — the
`Session` label comes from `cart.queries`, not from R-004's own text.

**What a key can and cannot do (ADR-0128).** A key naming the reader keeps
one reader's entry from another's, and that entry is only ever hit by its own
reader: the cache shares nothing, and holds the reader's value where every
reader's is held. So a session's, a user's or a device's value is refused a
shared cache whatever its key. A tenant's is the exception, because a tenant's
readers share it; keying it by its organization is `cache_key_omits_partition`.

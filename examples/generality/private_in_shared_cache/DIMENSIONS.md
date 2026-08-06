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
| valid neighbour | `private-cache-is-fine.pw` | the same private value in a `cache private` page — a rule reacting to the LABEL alone, without the cache, would catch this |
| valid neighbour | `public-in-shared.pw` | the same shared cache with a public value — a rule reacting to the CACHE alone would catch this |

Two neighbours, because this invariant is a *conjunction*: private **and**
shared. A rule that fired on either half alone would pass the fixture and be
wrong, and only one neighbour each way can tell those apart.

**Not relevant.** *Cross-module* is already the fixture's shape — the
`Session` label comes from `cart.queries`, not from R-004's own text.

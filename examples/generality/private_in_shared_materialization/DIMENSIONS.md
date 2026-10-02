# `private_in_shared_materialization` — challenge dimensions

A headline invariant: *private data can never reach a shared materialization
through a dependency edge.* R-045 is the baseline — a fragment declared
`partition public` naming a `session query` in its `depends_on` list.

What makes it a headline claim rather than a cache rule is where the defect
lives. Every declaration in R-045 is individually correct: `Cart` asks for a
private cache and gets one; a fragment is entitled to a shared entry. **The
defect is the edge between two valid declarations**, which is a shape no
single-declaration rule can see and `PW5001` does not.

| dimension | file | why it is relevant |
|---|---|---|
| direct | `caught.pw` | two fragments in one file, one honest and one not — the private neighbour makes the shared one look like precedent |
| cross-module | `cross-module.pw` | the session label is declared in another module, so nothing in the fragment's own file is declared `session` or `private` |
| `private` not `session` | `user-scoped.pw` | the other restricted visibility; a rule matching the word `session` would miss it |
| partition, not visibility | `private-cache.pw` | the dependency declares no visibility and asks for `cache private`; the restriction is in the caching policy rather than the label |
| helper extraction | `helper.pw` | the dependency is declared `public` and passes the session it is given to a helper. A known gap until ADR-0128 |
| branch join | `branch.pw` | the dependency is public on one path and session-derived on the other. A known gap until ADR-0128 |
| valid neighbour | `neighbour.pw` | the same two fragments, both `partition private` — a rule reacting to the DEPENDENCY alone would catch this |
| valid neighbour | `public-dependency.pw` | the same shared fragment reading a public query — a rule reacting to the PARTITION alone would catch this |

Two neighbours, because the invariant is a **conjunction**: shared **and**
restricted. A rule firing on either half alone passes R-045 and is wrong, and
only one neighbour each way separates those cases.

## Closed gaps

`helper.pw` and `branch.pw` were `slips-through-helper.pw` and
`slips-through-branch.pw`, NARROW witnesses that compiled clean when they
should not. The architect named both when specifying this matrix:

```text
public materialization → private value through helper
public materialization → branch that can become private
```

Each dependency was a `public query` given the session as a parameter, and
nothing read a declaration's own parameters, so its edge was public. ADR-0128
(2026-10-02) closed both without giving a fragment a body: a query is a
function of its arguments, so what its parameters carry is part of what it
observes. Neither dependency declares a cache, so only that can decide them.

**Not relevant.** *Rebinding* — a fragment's `depends_on` names a declaration,
so there is no binding to rename. *Two sessions get distinct entries* is a
runtime property and is tested where it can be observed
(`pw-materialize::a_private_fragment_never_shares_an_entry_between_sessions`);
a compile-time witness could only restate the declaration.

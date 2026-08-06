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
| cross-module | `cross-module.pw` | the session label is declared in another module, so the fragment's own text contains nothing marked private |
| `private` not `session` | `user-scoped.pw` | the other restricted visibility; a rule matching the word `session` would miss it |
| partition, not visibility | `private-cache.pw` | the dependency declares no visibility and asks for `cache private`; the restriction is in the caching policy rather than the label |
| valid neighbour | `neighbour.pw` | the same two fragments, both `partition private` — a rule reacting to the DEPENDENCY alone would catch this |
| valid neighbour | `public-dependency.pw` | the same shared fragment reading a public query — a rule reacting to the PARTITION alone would catch this |

Two neighbours, because the invariant is a **conjunction**: shared **and**
restricted. A rule firing on either half alone passes R-045 and is wrong, and
only one neighbour each way separates those cases.

## Known gaps, executable

`slips-through-helper.pw` and `slips-through-branch.pw` compile clean and they
should not. The architect named both when specifying this matrix:

```text
public materialization → private value through helper
public materialization → branch that can become private
```

`PW5101` reads the graph, and the graph has an edge per `depends_on` target. A
fragment that depends on a **public** query whose body reaches private data has
a public edge, and the analysis that would see through it is the label dataflow
`private_in_shared_cache` already uses for pages — over a fragment's body, which
a `materialize` block does not yet have.

They are written as programs rather than as prose so the gap cannot drift out of
date: `just generality` fails the moment either stops compiling, and the file is
promoted rather than deleted.

**Not relevant.** *Rebinding* — a fragment's `depends_on` names a declaration,
so there is no binding to rename. *Two sessions get distinct entries* is a
runtime property and is tested where it can be observed
(`pw-materialize::a_private_fragment_never_shares_an_entry_between_sessions`);
a compile-time witness could only restate the declaration.

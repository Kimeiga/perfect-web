# `value_exceeds_sink_level` — challenge dimensions

A headline P0 invariant, so it gets a matrix rather than a single
counterexample. Each file changes exactly one thing about how the value
reaches the sink; the value and the sink are the same throughout.

| dimension | file | why it is relevant |
|---|---|---|
| direct | `direct.pw` | the fixture's own shape, as the baseline |
| rebinding | `rebound.pw` | `let same = token` — the case that proved labels were attached to names |
| helper return | `helper-return.pw` | the value leaves and re-enters through a declared function |
| record field | `record-field.pw` | the secret is a field of a value built around it |
| branch join | `branch-join.pw` | secret on one path, public on the other; the join must keep the restriction |
| destructuring | `match-binding.pw` | the payload is named by a pattern rather than by a `let` |
| interpolation | `interpolation-hole.pw` | the value leaves inside a string's `{expr}` hole |
| cross-module | `cross-module.pw` | the secret's declaration is in another module |
| valid neighbour | `valid-public-neighbour.pw` | the same shape with a public value — a rule that banned the SHAPE fails here |

**Not relevant, and why.** *Deferred execution* — a sink call inside an event
handler is still a sink call, and `deferred_spans` is about effect attribution
rather than about flow. *Duplicate siblings* — resolution is by module path
here, not by member name, since the by-name fallback was deleted.

**Known not covered.** A label into a collection and back out
(`List.push(xs, secret)` then `List.first(xs)`) is lost, because element types
are not inferred. No witness is written for it because writing one would
require claiming what the fix is; `labels.rs` records the limit.

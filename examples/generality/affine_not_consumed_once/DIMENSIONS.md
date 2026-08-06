# `affine_not_consumed_once` — challenge dimensions

An affine value is consumed exactly once, in the scope that acquired it.

| axis | file | why |
|---|---|---|
| direct invalid | `caught.pw` | the release is in one branch, the exit in the other — the shape the textual scan missed |
| indirect invalid | `caught-match.pw` | a `match` with one arm of ten leaving without releasing |
| indirect invalid | `escapes.pw` | consumed somewhere the acquiring scope cannot see |
| neighbour | `released-before-return.pw` | the same early exit, with a release before it |
| neighbour | `both-branches-release.pw` | no trailing commit, because both branches already released |
| neighbour | `derived-value-escapes.pw` | module state holds a value DERIVED from the handle, not the handle |

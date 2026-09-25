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
| indirect invalid | `never-released.pw` | no release and no early exit: the end of the scope is the exit |
| indirect invalid | `live-across-try.pw` | the early exit is a failing `?`, not a `return` |
| direct invalid | `released-twice.pw` | released on every path, and twice on one: "exactly once", the other half |
| indirect invalid | `released-in-a-loop.pw` | a release that runs any number of times |
| indirect invalid | `parameter-not-released.pw` | a declaration that promises to release its parameter, and does not on one path |
| neighbour | `parameter-released.pw` | the same helper, ending it once on each path |
| neighbour | `moved-to-the-caller.pw` | the value is the body's result, which moves it to the caller |

The seven rows after the first six were added on 2026-09-25, when the check
began counting releases on every path; each caught one was accepted before.

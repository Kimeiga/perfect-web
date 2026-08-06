# `scope_outlives_owner` — challenge dimensions

A subscription may not request a scope that outlives the declaration owning
it. R-039 asks for `application` scope from inside a component.

| axis | file | why |
|---|---|---|
| direct invalid | `direct.pw` | R-039's shape |
| indirect invalid | `session-in-component.pw` | a different scope pairing — `session` inside a component |
| indirect invalid | `nested.pw` | the request is inside a `frame`, not at the top of the body |
| neighbour | `component-scope.pw` | the same observation scoped to the component that owns it |
| neighbour | `application-at-application.pw` | the same `application` scope requested where it is legitimate |

Two neighbours because the rule is a *comparison* between two scopes: one
proves it does not ban the scope, the other that it does not ban the
construct.

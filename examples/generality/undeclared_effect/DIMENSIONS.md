# `undeclared_effect` — challenge dimensions

The row a declaration writes must name every effect its body performs. RQ-2
measured that Koka propagates effects through unannotated higher-order code;
this is the same property over `pw`'s own source.

| axis | file | why |
|---|---|---|
| direct invalid | `caught.pw` | three helpers deep, across modules, through a lambda |
| indirect invalid | `via-callback.pw` | the effect is only inside a lambda handed to a generic |
| indirect invalid | `partial-row.pw` | the row names one of the two effects performed |
| neighbour | `complete-row.pw` | the same chain with a complete row |
| neighbour | `open-row.pw` | no row written at all — an unstated row is not a claim, and checking it would reject working programs |
| semantic preservation | `family-covers-member.pw` | declaring `database` must cover `database.read` |

**Not relevant.** *Branch join* — every branch's effects are performed, so
there is no join, only a union that the row must already contain.

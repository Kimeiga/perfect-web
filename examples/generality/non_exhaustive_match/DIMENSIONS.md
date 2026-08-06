# `non_exhaustive_match` — challenge dimensions

| axis | file | why |
|---|---|---|
| direct invalid | `caught.pw` | the defect is in a match nested as another match's arm result |
| indirect invalid | `missing-one.pw` | nine of ten variants covered — off by one, not by many |
| neighbour | `wildcard-is-enough.pw` | a wildcard arm makes it exhaustive |
| neighbour | `all-arms.pw` | every variant named, which must not be reported as redundant |
| semantic preservation | `or-pattern.pw` | `a \| b` covers both, so an or-pattern is not a hole |

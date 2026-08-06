# `forbidden_effect` — challenge dimensions

Some effects are not permitted where a declaration runs, whatever it declares.
Declaring the effect is what must *not* help — that is the whole distinction
from `undeclared_effect`.

| axis | file | why |
|---|---|---|
| direct invalid | `caught.pw` | a network fetch two helpers below a view, WITH the row declared |
| indirect invalid | `painter.pw` | a painter reaching the document, identified by its row rather than a declaration kind |
| indirect invalid | `build-page-clock.pw` | a build-placed page reading the wall clock |
| neighbour | `handler-may-fetch.pw` | the same fetch inside an event handler — it does not run during render |
| neighbour | `query-may-fetch.pw` | the same fetch in a query, which is what queries are for |
| neighbour | `page-may-read-clock.pw` | the same clock read in a page rendered per request |

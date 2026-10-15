# `forbidden_effect` — challenge dimensions

Some effects are not permitted where a declaration runs, whatever it declares.
Declaring the effect is what must *not* help — that is the whole distinction
from `undeclared_effect`.

| axis | file | why |
|---|---|---|
| direct invalid | `caught.pw` | a network fetch two helpers below a view, WITH the row declared |
| indirect invalid | `painter.pw` | a painter reaching the document, identified by its row rather than a declaration kind |
| indirect invalid | `build-page-clock.pw` | a build-placed page reading the wall clock |
| indirect invalid | `named-callback.pw` | a geometry read through a declaration named as a value and handed to `List.map`, not a lambda (ADR-0078) |
| equivalent syntax | `each-head-member.pw` | a database read in a loop's list, written as a member of the store rather than a call (ADR-XXXX) |
| nesting | `each-key-nested.pw` | a database read in a loop's key, in a loop inside another (ADR-XXXX) |
| neighbour | `each-pure-member.pw` | the same loop over a pure member: a loop's head performs what its expressions do, and nothing more |
| neighbour | `handler-may-fetch.pw` | the same fetch inside an event handler — it does not run during render |
| neighbour | `query-may-fetch.pw` | the same fetch in a query, which is what queries are for |
| neighbour | `page-may-read-clock.pw` | the same clock read in a page rendered per request |

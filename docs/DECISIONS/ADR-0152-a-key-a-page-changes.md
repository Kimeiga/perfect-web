# ADR-0152: a key a page changes, and what its stale work does

Status: accepted under the owner's delegation of 2026-10-02. Date: 2026-10-03.
Milestone: E14 (T07), and the charter's §15.6 store tests 6-8. Settles
ADR-0089's open ruling on `on_key_change supersede` and `keep`.

## Context

A query's key could not change in the browser. A page's `let` query took its
arguments from the page's parameters and `current_session()`, and PW5301
refused a signal given as one: "`term` is a signal, and this reads it once,
on the server, where it never changes". So a menu search, a category filter
or a page of results, which DoorDash is made of, could not be written. And
`on_key_change`, which PW0325 requires of a keyed query, meant nothing at run
time. ADR-0089 left open "whether `keep` and `supersede` mean anything a
runtime does".

The charter asks for it, in its store's required tests (§15.6):
- 6. Same query key under repeated local recomputation makes one request.
- 7. Changing the query key cancels or supersedes stale work.
- 8. Navigating away cancels unneeded requests.

E14's T07, "cancel a stale navigation request", is the benchmark's version,
and its last unwritten task.

How others answer a key that changes while its read is in flight
(researched 2026-10-03):
- **Phoenix LiveView's `start_async`.** "If there is an in-flight task with
  the same name, the later start_async wins and the previous task's result is
  ignored." Stopping the old task is a separate call, `cancel_async`.
- **TanStack Query.** "When a query becomes out-of-date or inactive, this
  signal will become aborted", if the query function reads the signal.
  Otherwise the old answer is kept in the cache under its own key.
- **RxJS, redux-saga and ember-concurrency** name the shapes. `switchMap`,
  `takeLatest` and `restartable` cancel the old work. ember-concurrency's
  `keepLatest` lets the running task finish, then runs only the latest of
  those that waited.
- **A hand-written fetch in an effect** shows whichever answer arrives last,
  unless its author adds an ignore flag or an `AbortController`. That is the
  stale-overwrite bug PW0325 exists for.

## Decision

1. **A page's query may be given a page signal.** `let found = query
   MenuSearch(id, term)`, where `signal term: String = ""`, is a binding
   *keyed by* `term`.
   - The document is rendered for the signal's first value.
   - When the signal changes, the browser asks for the binding again, for
     the new key.
   - The server reads the query for that key, by its policies, and sends what
     the page shows of it as one patch set, in the session's ordered stream
     of frames.

   PW5301 now counts a page query's argument among the places the browser
   reads a signal again.
2. **A signal that keys a query is a `String`, an `Int` or a `Bool`**
   (PW5308). A key crosses to the server and is compared exactly. A signal
   is the browser's claim, as a request parameter is. A `session` query still
   takes its session from the server, never from the page.
3. **A query a signal keys declares `on_key_change`** (PW5309), whether or
   not it declares a `concurrency`.
4. **What `on_key_change` does.** It applies when a binding's key changes
   while the read for its old key is in flight:
   - **`cancel`**: the old read is stopped. The browser aborts its request.
     The server lets go of the old key's flight, and `pw-resource` stops a
     flight nobody holds (`StopReason::NoSubscribers`). The new read starts
     at once. For work worth nothing once superseded: a search as one types.
   - **`supersede`**: the old read runs to its end, and its answer is not
     shown. The new read starts at once. For work that should not stop
     part way, or whose answer warms a cache.
   - **`keep`**: the old read runs to its end, and the new read waits for
     it. Keys changed meanwhile are dropped but the latest, as
     ember-concurrency's `keepLatest` does. One page never asks a source for
     a binding twice at once. For a source that cannot take concurrent
     reads.

     The browser sends each read at once, and the server runs them one at a
     time. The server therefore knows the latest read while the one before
     it still runs, and never applies that one. A browser that queued the
     new read itself would leave the old read the latest when it ended, and
     its answer would be shown for a key no longer chosen.

   In all three, **an answer is never applied for a key it was not asked
   for.** The server applies a read only if it is the latest the session
   asked for that binding. While a new key's read is pending, the region
   shows the last answer applied, marked `aria-busy`.
5. **A key the browser already has is not asked for again** (test 6). That
   is the key shown, or the key in flight.
6. **Leaving the page stops its reads** (test 8). On `pagehide` the browser
   aborts every read in flight. The server, finding the request gone, lets
   go of a `cancel` read's flight.

   Every read holds its key's flight while it runs, and `pw-resource` stops
   a flight only when nobody holds it. A `cancel` read letting go stops the
   flight only if no other reader shares it: another session, or a document
   being rendered.

## Found on the way: presses ran in the order their code arrived

A handler's module loads on its first press. Two buttons pressed quickly,
Hot then Cold, loaded two modules, and each handler ran when its own module
arrived. If Cold's arrived first, Cold ran first and Hot second, and Hot was
left chosen. Now each handler starts after the press before it has started.
The modules still load at once, and a slow command still does not hold up
the press after it.

## Acceptance

- The compiler's tests: a binding a signal keys is planned, with its
  signals, its policy and the signal's first value. PW5308 and PW5309 refuse,
  and a page query's signal argument passes PW5301.
- The server's tests:
  - a keyed read is applied as a patch set;
  - a superseded read is not applied;
  - under `cancel`, a newer read stops the old key's flight;
  - a later command re-reads the binding for its current key.
- The server's tests, on T07's store, for each policy:
  - Hot is made slow, and Cold asked for while Hot is read;
  - under `cancel`, Hot's read is stopped part way;
  - under `supersede`, Hot's read runs to its end, unshown;
  - under `keep`, Cold's read waits for Hot's to end.

  And a browser that leaves lets go of its read.
- T07's controls on all three stacks, which run the browser half: in
  Chromium, Cold's items show while Hot is read, and Hot's request is
  aborted.
- The charter's store tests 6-8, in Chromium, Firefox and WebKit, on a store
  with T07's setup and reference (`e2e/keyed.spec.mjs`, served from
  `dist-keyed` by `keyed-store.sh`):
  - a changed key's old read is aborted, stopped on the server, and never
    shown;
  - a key the page has is not asked for again;
  - leaving the page stops its read on the server.
- The mutation controls.

## Not claimed

- **A key from a URL.** A page's parameters still come from its address,
  and a new address is a new document.
- **Two tabs of one session.** The key, like what a page shows, is recorded
  per session.
- **A view's signals, and a stream's query, keyed by a signal.** Only a
  page's own `let` query is.

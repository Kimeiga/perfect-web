# ADR-0148: a stream region shows its query's state, and its settled arm comes in the same response

Status: accepted under the owner's delegation of 2026-10-02. Date: 2026-10-03.
Milestone: E14, for T05 and T10. Charter §9.3, §15.3, §15.5, §15.6 (tests 1, 3
and 17).

## Context

A-008 declares a slow public query `delivery streamed`, and shows it with
`<stream query={Recommendations(id)}>`. The stream holds:
- a `<placeholder>`;
- `<ready as={items}>`;
- `<failed as={_error}>`.

It checks. ADR-0075 refused it at build, because the template IR had no
representation for it; only the Marko adapter rendered one. So two of the
benchmark's tasks were not expressible: T05, streaming recommendations, and
T10, a loading and an error state.

Nothing checked what a stream needs either:
- a page could read a streamed query with `let`, and wait for it;
- a stream could leave out the arm for a state its query reaches;
- a streamed query could declare no bound on how long its region waits.

`<failed as={e}>` was typed as the query's declared error (ADR-0066). A query
also fails where it declares nothing: its budget is spent, or the host traps.
There is then no error value for `e` to be.

### What the platform does now

The WHATWG's out-of-order streaming (whatwg/html#11818), from WICG's
declarative partial updates, marks a range with `<?start name="x">` and
`<?end>`. A later `<template for="x">` replaces the range's contents.

Its status:
- Chrome turns it on by default from 150. The Intent to Ship was sent on
  2026-05-26.
- Mozilla's position is "proposed support".
- WebKit's position is "support".

Measured on this host on 2026-10-03, by `just e14-stream-probes`
(`docs/evidence/E14/stream-probes.txt`):
- **Chrome 154** applies the patch while parsing, with JavaScript off too.
  The markers go, and the template is never inserted; comments around the
  range stay.
- **Chromium 145, Firefox 146 and WebKit 26** (Playwright's builds) parse each
  marker as a comment. The template stays as an inert element.
- **When a script starts on a response still open:**
  - a deferred module, which is how the runtime was loaded, starts only after
    the whole response arrives, in every engine;
  - `async` starts early in Chromium and Firefox, and late in WebKit;
  - a classic inline script that calls `import()` starts early in all three.
- **When WebKit first paints:** only once the page holds more than about 200
  characters of text, or has loaded. A smaller shell is shown in Safari only
  when its streams have settled, whatever renders it.

## Decision

1. **A `<stream>` shows its query's state.** Its direct children are:
   - `<placeholder>`, shown while a streamed query is pending;
   - `<ready as={v}>`, given what the query answered;
   - `<failed as={why}>`, given why it did not.

   Its `query` is a call of a query. The arguments a page can give are its
   parameters, or an invocation-context call.
2. **When the region is filled is the query's delivery, as its declaration
   says.**
   - **`delivery streamed`:** the document is sent without waiting, with the
     placeholder inside a `<?start>`/`<?end>` range. In the same response,
     the server writes the arm the query settled to as a `<template for>`.
     Regions settle in the order their queries answer, not the order they
     are written. The response ends with the last one, and closes, which
     tells the browser the document is complete.
   - **Any other delivery:** the page waits for the query, as for a `let`,
     and the region shows the arm it settled to from the start.

   Every stream's query starts before the document is rendered. A streamed
   query that has settled by then is rendered in place, with no placeholder.
3. **The rules, a new family (PW54xx):**
   - **PW5400:** a query declared `delivery streamed` is read only by a
     `<stream>`. A `let` would make the page wait for it, which is what the
     declaration says the page must not do.
   - **PW5401:** a stream shows each state its query can be in, and no
     other:
     - it needs a `<ready>`;
     - it needs a `<failed>`, since every query can fail;
     - it has a `<placeholder>` when its delivery is streamed, and none
       otherwise, since a placeholder for a query the page waits for is
       never shown;
     - each part appears once, and nothing else is in the stream;
     - its parts appear nowhere outside a stream;
     - neither the stream nor its parts write attributes, which would be
       dropped;
     - its `query` calls a query.
   - **PW5402:** a query declared `delivery streamed` declares a `timeout`.
     A region waits for its query that long and no longer; without one, a
     source that never answers would hold the response open forever.
4. **The failed arm is given `Option<E>`.** `Some(e)` is the query's declared
   error. `None` is the host's failure: the budget spent, or a trap. A query
   that declares no error gives the arm nothing, and `as` on it is refused
   (PW5401). Both type passes, the typer and the value relations, give it
   `Option<E>`.
5. **A declared error is an answer that is not kept.** Every reader waiting
   on the flight that answered it is given it, and the next reader asks
   again, as after a host's failure. An `Ok` answer is kept as the query's
   policy says. A `let` that reads an `Err` still makes the page unavailable
   (ADR-0147).
6. **The runtime starts while the response is still open,** from a classic
   inline script that imports it, on every page. Measured: a deferred module
   would have left every handler unbound until the slowest region settled
   (charter §15.6 test 17).
7. **The runtime fills a region the browser does not, and binds what it
   holds.** It applies each `<template for>` still in the document, at boot
   and as the parser adds them. When a region settles, filled by the browser
   or by the runtime, its parts are read again and their handlers bound.

Two findings along the way:
- **The server's handler table, and its part lookup, named the block kinds
  they walked.** A handler in a stream's arm was in no table, so its button
  was refused. Both now walk every part's regions through `nested()`, as the
  IR's own walks do. So does the plan's part lookup.
- **The benchmark hook that sets the recommender now drops the work still in
  flight**, not only what was kept. A request after it had joined a flight
  started under the recommender as it was.

## Alternatives

- **Send the settled arm over the page's subscription**, as `ReplaceRange` in
  a `patch_set` (ADR-0146). Rejected:
  - nothing shows until the runtime has booted and subscribed;
  - it costs a round trip after the document;
  - with no JavaScript, the region is never filled.
- **React's form:** the arm in a hidden element, moved by an inline script.
  Rejected. It needs a script in every browser, where the platform's form
  needs none in Chrome.
- **Stream in order:** write the region when its query answers, and the rest
  of the document after it. That is the waiting `delivery streamed` rules out.
- **`<failed as={e}>` as `E`** (ADR-0066). Rejected: a host's failure has no
  `E`.
- **A `Failure<E>` type** with timeout and unavailability cases. Deferred.
  `Option<E>` is what a page can act on, and a new built-in type touches every
  stage.
- **Keep a declared error, as an answer.** Rejected. A transient refusal would
  then be served for the whole freshness window. HTTP caches, React Query and
  SWR keep no failure either.
- **Start the runtime with `async`.** Rejected: WebKit runs an `async` script
  only after the response ends.

## Acceptance

- `compiler/pw-core/tests/streams.rs`, 11 tests:
  - each rule, with a control that the right program checks;
  - the failed arm's `Some` and `None`, and a match over it that leaves out
    `None` (PW0305).
  Ten fail before the change; the eleventh is the control.
- `compiler/pw-core/tests/stream_plan.rs`, 6 tests:
  - the IR's part, with its query, arguments, delivery and arms;
  - the plan's entry, with the query's policies;
  - a stream inside a block refused at build;
  - a `fallback` and a shown signal refused by the plan.
- `runtime/pw-render/tests/streams.rs`, 4 tests:
  - a pending region in a patchable range;
  - one the page waits for is never pending;
  - each settled arm, with its value bound;
  - the patch holding exactly what the settled region holds.
- The server's tests, over a real socket, on the store with a streamed
  region:
  - the document arrives first and the arm after it, in one response;
  - a region past its budget is given the host's failure when its budget is
    spent;
  - a declared error and a host's failure are told apart;
  - a declared error is given and not kept;
  - a page with nothing streamed is one response of known length.
- `e2e/stream.spec.mjs`, over `examples/demo/streamed.pw`, in Chromium,
  Firefox and WebKit:
  - the page is usable while its region is pending;
  - the region fills in place, and its Pick buttons work;
  - each failure shows its arm;
  - a query past its budget ends the response;
  - with JavaScript off, the placeholder stays.

  One more test runs in the host's Chrome where installed: a region filled
  by the browser itself, with JavaScript off.
- The browser suite: 457 tests pass in three engines, with the runtime
  started the new way on every page.
- The accepted corpus, A-008 among it, checks clean.
- `scripts/streams_mutations.py`; `just e14-streams` records it all in
  `docs/evidence/E14/streams.txt`.

## Not claimed

- **With no JavaScript, outside Chrome 150 and later,** a streamed region
  stays its placeholder.
- **WebKit paints a page only once it holds about 200 characters of text,**
  or has loaded. Below that, Safari shows the shell only when its regions
  have settled.
- **A region is rendered once per document.** A change to its query after it
  settles is not sent.
- **A stream sits at the top of a page, or in a view composed there.** One
  inside a block or a loop's row is refused at build. So is a signal shown
  inside one, or a view that holds one.
- **`fallback` is not executed for a stream's query**: a plan with one is
  refused.
- **A page with a stream and a `let` is the store's route's** in the
  development server. Any other page may read queries only through streams.

# ADR-0163: a page says when it is absent

Status: accepted under the owner's delegation of 2026-10-02. Date: 2026-10-03.
Milestone: E14. The last step of the audit's route gap: charter §15.3's
`/stores/:store_id` for a store that is not there.

## Context

- **`/stores/999` was answered 503.** ADR-0162 serves the store page at
  `/stores/{id}`. For an id no store has, `Store(999)` answers
  `Err(StoreError.NotFound)`. A page whose queries fail is answered 503
  (ADR-0147), whatever they answered.
- **503 says the wrong thing.** RFC 9110 §15.6.4 defines 503 as a server
  "currently unable to handle the request due to a temporary overload or
  scheduled maintenance". §15.5.5 defines 404 as finding no current
  representation of the target. A crawler or cache told 503 comes back
  later, and a person is told to wait for a store that will never be there.
- **Which declared error means "absent" is not the server's to guess.**
  - `StoreError.NotFound` from `Store(id)` says the address names nothing.
  - `StoreError.Unavailable` says try again.
  - Another program's `NotFound` could name a missing part of a page that is
    still worth showing.
- **Other frameworks leave it to the page's code.**
  - Next.js 16: a page calls `notFound()`. The call throws, ends the
    segment's rendering, and renders `not-found.js` with 404.
    - A call inside a `<Suspense>` boundary comes after the response began
      as 200, and the status cannot change. The `noindex` tag Next.js adds
      keeps that soft 404 out of search results.
    - Its documentation warns that a call left in an un-awaited promise
      renders no not-found page at all.
  - SvelteKit: `load` calls `error(404, ...)`, which sets the status and
    renders `+error.svelte`. An error thrown any other way is a 500.
  - Rails: the Active Record railtie maps `ActiveRecord::RecordNotFound` to
    `:not_found` for the whole application. A page whose lookup of a
    secondary record raises it is answered 404 too.
  - Nothing in these checks where the call is, or that the error it is
    meant for can reach it.

## Decision

1. **A page declares the error that means it is absent**:
   `not_found_on StoreError.NotFound`. The store page declares it.
2. **PW0342 holds the clause to the page.**
   - It is a page's.
   - It names a case, as `Type.Case`. The type is visible as written:
     `StoreError` imported, or `domain.StoreError`. The case is the type's.
   - A query the page reads with `let` can answer it: a query whose declared
     result is `Result<T, Type>`.
     - A query whose value merely holds the type answers with it, and does
       not fail with it.
     - A `<stream>` region's failure fills its `<failed>` arm (ADR-0148).
     - Neither of those can make the page absent, so neither counts.
3. **The page's plan carries the case**, by its WIT name (`not-found`), on
   each binding whose query can answer it, and on no other.
4. **A host answers 404 when a binding answers the case**, with a page of its
   own that holds nothing of the page's. Any other failure is 503, as
   before.
   - The development server carries the kind of failure as a type, `Unread`,
     not in its text. No other error's message can make it the other.
   - The not-found page has a language, a title and a heading.
5. **The status is a real one.** Every query a document waits for is read
   before its response begins, and a streamed region cannot answer the
   case, by rule 2. The soft 404 that Next.js accepts after streaming starts
   cannot happen here.

## Alternatives

- **Any case named `NotFound` is 404.** This guesses by name: a program's
  `NotFound` can mean something else, and an error with no case of that name
  could never say "absent". It is Rails' global mapping, with the same
  failure on a secondary lookup.
- **The query declares it.** `Store(id)` is read by other pages too. A cart
  page that names each line's store should not be absent because one line's
  store closed. The address is the page's.
- **A call in the page's body**, `not_found()` in a `match`. A Pleris page
  has no imperative render path. A declaration is checked and planned; a call
  would need its own proof of reachability, which is the check PW0342
  already makes.
- **Naming the binding**: `not_found_on store: StoreError.NotFound`. What the
  case means is its type's, not one binding's. `Menu(id)` answering
  `StoreError.NotFound` says the same thing. A page that needs the
  difference declares two error types.
- **410 Gone** for a store that was there and is not now. The data layer
  would have to say so, with a case of its own; see "Not claimed".

## Found on the way

- **Every response said OK.** The development server wrote `HTTP/1.1 {code}
  OK` for every status, a 503 among them. A client ignores the reason
  phrase (RFC 9112 §4), but a person reading the response does not. Each
  status now has its own phrase.
- **A failure's kind was read from its text.** The first version of rule 4
  marked the failure's message and looked for the mark where the page is
  answered. The message arrived there wrapped, so a contains-check was
  needed, and a data-layer error whose text held the mark would have been
  answered 404. The kind is a type now.
- **Three comments still said the server holds one store** (ADR-0162). They
  are corrected.

## Acceptance

- `compiler/pw-core/tests/not_found.rs`:
  - a page that names its absence checks clean, for any case of the type;
  - the clause on a query is refused;
  - a value that is no case is refused: no dot, an unknown type, an unknown
    case, a record, a query;
  - a case no query the page reads fails with is refused, a query whose
    value is `Map<String, StoreError>` among them;
  - an error imported from another module is named as it is imported, and
    by its module's name;
  - the plan carries the case on each binding that can answer it.
- The development server's tests:
  - `/stores/999` is `404 Not Found`, a page of its own, and no document the
    server holds;
  - `/stores/48` is `200 OK`;
  - controls: a clause naming `StoreError.Unavailable`, and no clause, both
    answer `/stores/999` with `503 Service Unavailable`.
- `e2e/stores.spec.mjs`, three engines: a store that is not there is 404,
  titled and headed "Not found", with nothing of the store's page.
- The page plan's evidence, written again by `just e10-component`.
- `scripts/not_found_mutations.py`: 17 mutants, recorded by
  `just e14-not-found`.

## Not claimed

- **A program's own not-found view.** The page is the server's. A declared
  view, both for this and for an address no route names (answered 404 as
  plain text), is a later ruling.
- **More than one case**, or cases of more than one type. The plan's field
  is a list, so a later ruling can widen the clause without changing hosts.
- **410 Gone.**
- **A keyed read that answers the case after the page is shown** (ADR-0152).
  The page is there, and the read fails as any read does.
- **Cache headers on the 404.** RFC 9110 makes a 404 heuristically cacheable.
  The development server sends no `Cache-Control` for any page; a host
  applying a page's `cache` policy to its responses is a deployment's.

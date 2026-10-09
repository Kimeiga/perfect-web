# ADR-XXXX: a page at another address of the page is moved there

Status: accepted under the owner's delegation of 2026-10-02, on track
`kiokun`'s need (W6's milestone 1c, ADR-0289: `KiokunError.Moved` is
declared and not returned "until the integrator's page redirect lands").
Date: 2026-10-09. Milestone: E14, kiokun.com in Pleris.

## Context

- **kiokun.com moves a word's page.** A simplified character that equals
  one traditional form in meaning is answered 308 to that form's page, the
  address's query kept (`[word]/+page.ts:419-425`):
  `redirect(308, '/' + encodeURIComponent(canonicalTarget) + url.search)`.
  A conjugated Japanese verb is answered 307 to its dictionary form, with
  `from`, `conj` and `alt` added to the query (`+page.ts:363-378`).
- **The program could not say so.** A page could say that its address
  names nothing (`not_found_on`, ADR-0163, answered 404), and nothing else
  about its address. W6 declared `KiokunError.Moved(String)` and left its
  query never answering it.
- **What HTTP says** (RFC 9110):
  - 308 Permanent Redirect and 307 Temporary Redirect keep the request's
    method (§15.4.8, §15.4.9); 301 and 302 let a client change a POST into a
    GET.
  - `Location` may be a relative reference, resolved against the address
    asked for (§10.2.2); a fragment the address had is kept when the
    `Location` has none.
  - A 308 is heuristically cacheable unless explicit cache controls say
    otherwise; its content is usually a short note with a link.
- **What Google Search says** ("Redirects and Google Search"): 301 and 308
  are permanent, and the indexing pipeline takes the target as canonical;
  302, 303 and 307 are temporary, and the source stays shown.
- **The status must be decided before the response starts.** ADR-0163's
  rule 5 holds already: every query a document waits for is read before its
  response begins, and a streamed region cannot answer a page's case.

## Decision

1. **A page declares the error that means its address is another address
   of the page, and how it moves**: `redirect_on KiokunError.Moved
   permanent`, or `temporary`. The word is required; neither is assumed.
2. **PW0350 holds the clause to the page.**
   - It is a page's.
   - It names a case as `Type.Case`, the type visible as written, as
     `not_found_on`'s does (ADR-0163).
   - It is not the case the page's `not_found_on` names: a page that is
     absent is not elsewhere.
   - A query the page reads with `let` can answer it.
   - **The case carries one value, of the type of the one parameter the
     page's route carries.** Where the host sends the reader is the page's
     own route, that parameter filled with the value: `/word/{word}` with
     `Moved(String)`. A `String` does not fill a `StoreId`, though one is
     the other underneath: types are compared by their resolved identity
     (`ResolvedType::same_as`); a route with no parameter, or two, has no
     one place for it.
3. **The page's plan carries the case**, by its WIT name, and whether the
   move is permanent, on each binding whose query can answer it, and on no
   other: `"redirect": {"case": "moved", "permanent": true}`.
4. **A host answers the case with the move**:
   - **308 where the move is permanent, 307 where it is not**, each keeping
     the method.
   - **`Location` is a path**: the page's route, its parameter filled with
     the value encoded as a link's hole is (`url_component`, every UTF-8
     byte but RFC 3986's unreserved characters), and the address's query
     kept as it came. No origin is named, so no `Host` header can choose
     one.
   - **Kept by no cache**, as every answer to a session is (ADR-0184): a
     heuristically cacheable 308 would outlive the data that decided it.
   - **Its content is a short page** with a language, a title, a heading
     and a link to the address.
   - **A value no segment can carry** (empty, `.` or `..`, as navigate's
     rule has it, ADR-0280), **or the address asked for itself**, is the
     program's fault: answered 500 and logged, never a loop.
   - **A case the page does not name is a failure like any other**, 503
     (ADR-0147).

## Acceptance

- **`compiler/pw-core/tests/redirects.rs`, 7 tests**: the control (both
  words); a page's alone; the case named and how it moves (no word, three
  words, another word, no type, no case, a type not visible); not the
  absent case; a query the page reads can answer it; one value, of the
  route's parameter's type (nothing, a `String` for a `StoreId`, two
  values, a route with none and with two); the plan carries the move on
  the binding that can answer it, and on no other.
- **The development server's `tests::redirects`, 5 tests**, on kiokun's
  word page, its query made to answer `Moved` for words the tests choose:
  308 with `Location: /word/%E9%AD%9A` (魚), kept by no cache, with a link,
  the query kept, and the address it names served; 307 for a temporary
  move; a move to itself, and to `..`, answered 500; the case unnamed,
  answered 503; the plan carries it on the word's binding.
- **`scripts/redirects_mutations.py`, 15 mutants**: each part above undone
  fails a test.

## Not claimed

- **kiokun's word page does not answer it yet.** Its query must return
  `Moved` where kiokun.com's `equivalentTraditionalTarget` says so; that is
  W6's, held to kiokun.com's own code (the oracle standard, ADR-0289's
  successor).
- **A move to another page**, or one that fills a route of several
  parameters, is refused (PW0350), not expressed: no program needs one yet.
- **A move that adds to the query**, as kiokun.com's deinflection does
  (`from`, `conj`, `alt`), needs a page's typed query parameters, which the
  language does not have.
- **A redirect is the document's.** A keyed read whose query answers the
  case in the browser fails as any read does.
- **Cache policy for a deployment** (a CDN keeping a permanent move) is not
  ruled here; the development server keeps none.

## Alternatives

- **301 and 302**: they let a client change the method; 308 and 307 say
  the same moves without that, and kiokun.com sends 308 and 307.
- **An absolute `Location`**: it needs the deployment's origin, which only
  a canonical link needs (W6's 1e, Q2), and a relative path is resolved by
  every client against the address asked for (RFC 9110 §10.2.2).
- **A redirect written in the view** (`{#match}` arm): the status line is
  sent before any of the view is, so a view cannot decide it; ADR-0163
  chose a clause for the same reason.
- **A default of `permanent`**: a temporary move cached as permanent sticks
  in browsers and search indexes; the word is required.

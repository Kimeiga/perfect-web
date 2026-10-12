# ADR-0309: a page's query parameters are typed, and a GET form makes them

Status: accepted under the owner's delegation of 2026-10-02 (the
integrator's). Date: 2026-10-11. Milestone: E14. Found by W6 (kiokun's
search) and W9 (the store's): a search's text could not reach a page.
Builds on ADR-0160 (a page's route), ADR-0162 (served at it), ADR-0280 (a
navigation after a commit) and the ruling of 2026-10-11 in PARALLEL.md (no
search form "sent by `navigate`").

## Context

- **A route names path segments only** (ADR-0160): `route "/stores/{id}"`,
  each `{name}` one segment and one text parameter, required. A search's
  text belongs in the address's query, where a browser's GET form puts it
  and where a reader can copy, bookmark and share it.
- **No form could reach a search.** W6 found that "a form sent by
  `navigate`", the first ruling for kiokun's search box, cannot be written:
  `navigate` goes only at the end of a command's `Ok` arm (ADR-0280, PW5043),
  and a search commits nothing. The ruling of 2026-10-11: no search box
  until a route reads its query part, then a plain GET form, which needs no
  script.
- **The host already read a query, untyped**: an unrouted page's parameters
  from `?item=cortado` (ADR-0130), each required, percent-decoded, a `+`
  kept as itself.

## Research (primary sources, read 2026-10-11)

- **RFC 6570, URI Template**, §3.2.8 "Form-Style Query Expansion: {?var}":
  "useful for describing an entire optional query component"; `{?who}`
  expands to `?who=fred`, `{?x,y,empty}` to `?x=1024&y=768&empty=`; an
  undefined variable expands to nothing; a Level 3 operator. §1.4: a
  template "can be used in reverse for the purpose of variable matching"
  only where its expressions are delimited.
- **The URL Standard, §5.1, the application/x-www-form-urlencoded parser**:
  the input split on `&`, an empty piece skipped, a piece without `=` a
  name with an empty value, "Replace any 0x2B (+) in name and value with
  0x20 (SP)", then percent-decoded and "UTF-8 decode without BOM".
  `URLSearchParams.get(name)` is "the value of the first tuple whose name is
  name".
- **HTML's form submission**: a GET form's data set is serialized as
  application/x-www-form-urlencoded and replaces its action's query, so a
  space is sent as `+`.
- How others type it: TanStack Router validates a route's search
  parameters with a schema (`validateSearch`); Elm's `Url.Parser.Query`
  reads each as a `Maybe`; SvelteKit's `url.searchParams` and Next.js's
  `searchParams` hand a page untyped strings.

## Decision

1. **A route's query part is RFC 6570's form-style expansion**, at its end
   and once: `route "/search{?q}"`, `route "/stores/{id}{?tab,page}"`. Each
   name in it is a page parameter, as each `{name}` segment is, and names
   one parameter once. A literal query part in a route (`"/search?q=x"`)
   is refused: the expansion says it.
2. **A query parameter is an `Option` of text**: the address may leave it
   out, so the page is given `None`, and an empty field is `Some("")`. Its
   text is a `String` or an opaque type over one, as a path's is (PW0621).
   A path parameter stays required.
3. **The host reads a query as a browser writes one**: the URL Standard's
   form-urlencoded parsing, `+` a space and `%20` a space, the first
   occurrence of a name taken, names the route does not declare ignored
   (an address may carry `utm_source`). Bytes that are no UTF-8 are refused
   (400), as in a path segment, where the URL Standard would replace them:
   the host never shows a reader a value it made up.
4. **An address is made by expansion**: a link or a `navigate` to a page
   with query parameters writes `?name=value` for each one given (`Some`),
   in the route's order, each value percent-encoded outside RFC 3986's
   unreserved set, and nothing for `None`.
5. **A GET form is checked as a link is** (charter §8.2, PW5009): its
   `action` names a declared route with a query part, and each named field
   of the form is one of that route's query names. A field the route does
   not declare would carry its value to nothing; it is refused, said with
   the route's names.
6. **A page is at its route's path.** Two routes that differ only in their
   query part are the same route (PW0341). An upload's paths are compared
   with a page's path (PW5602). A move (ADR-0295) fills the path's one
   parameter and keeps the address's query.

## Consequences

- kiokun's search box (W6) and the store's (W9) are plain GET forms,
  working with script off; `route "/search/{q}"` becomes `route
  "/search{?q}"` and `q: Option<String>`.
- A page's values hold an `Option` from its address.

## Not claimed

- A list parameter (`{?tag*}`, RFC 6570's explode) and a typed number: text
  alone, as a path's.
- `{&name}` continuation, and a literal query part.
- **A navigation to a query's parameter**: `navigate` gives a path's text
  alone (ADR-0280), and is refused for a page whose parameter is a query's;
  a link or a GET form gives it. The runtime expands a route's query part
  for when it does.

## Acceptance

- `compiler/pw-core/tests/routes.rs`: a query's parameter is an `Option` of
  text (PW0630); a query part ends the route and names each parameter once
  (PW0340); two routes that differ only in their query are one (PW0341); a
  link's query and a GET form's fields give only what the route names
  (PW5050), and the controls. `uploads.rs`: a page whose route has a query
  part, at an upload's route, is PW5602.
- The host's `a_query_is_read_as_a_browsers_form_writes_it`: `+` and `%20`
  a space, `%2B` a plus, the first of a name, a foreign name ignored,
  bytes that are no UTF-8 refused, and what a page is given. The host's
  `a_route_with_a_query_part_is_moved_by_its_path`: a move fills the path
  and keeps the query, and a move to itself is still refused, not a loop.
- `examples/demo/search.pw` and `e2e/query.spec.mjs`, in three engines: a
  GET form with scripts on and off, a link, `None` where nothing is given,
  the first of a name, and a 400 for bytes that are no UTF-8.
- Mutation controls: `scripts/query_params_mutations.py`.

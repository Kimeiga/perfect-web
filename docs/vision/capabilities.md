# What complex web apps need, and what Pleris does of it

**The north star** (the owner, relayed 2026-10-08): Pleris should be able to
make any complex web app, as perfectly as possible. Twitter, DoorDash and the
apps after them are instruments for that, not the goal. This matrix makes the
claim measurable. Each row is something complex apps need, with its status,
the ADRs that establish it, and what it does not do yet. The next reference
apps are chosen by the rows they cover, not by their brand.

The integrator owns this file. It is updated with each ADR that moves a row,
in the same commit.

**Statuses:**
- **built**: done, with recorded evidence, and its limits stated.
- **partial**: some of the row works, and what does not is stated.
- **missing**: nothing of it yet, or only pieces that do not add up to it.
- **not built here**: charter §2's "Do not initially build" list, or the
  owner's scope.

Checked against `master` at `5da9c055` (2026-10-14), ADR-0001 to ADR-0334.

## Built

| Capability | ADRs | Limits |
|---|---|---|
| Reading, caching and live updates | 0007, 0100–0103, 0107, 0127, 0128, 0145, 0146, 0177, 0219, 0224, 0271, 0294, 0296, 0297, 0320, 0323, 0330 | One host: a second hears nothing of the first's commits (0246). A commit tells up to eight sessions at once; a document no page has asked for in three seconds is passed by and told when its page asks, so a frozen tab sees a change then, not while it is frozen (0297). A block is told part by part only while it shows the arm it showed; another arm, a view's instance or an unkeyed list renders it again whole (0320). A member read is computed in text, attributes, blocks and titles, and through a match arm's binding, not in a loop's list or a view that contains itself (0323, 0330). A list renders in its own length (0294). A document's change is derived outside the host's table, and the trail kept of it is the development server's, for its tests (0296). |
| Commands: optimistic, idempotent, retried | 0025, 0121, 0122, 0154, 0157, 0159, 0173, 0222, 0236, 0238, 0268, 0275, 0305, 0325, 0331 | An interaction's answer commits with the command's writes, on PostgreSQL and in memory, so a retry after a restart or on another host is answered as the first was; a record answers a day at least, and a declared error records nothing (0325). A page's commands run in the order it sent them, kept by the host that answers (0305). A handler can send an `Option`, a `Result` or a declared sum type; not a case of a type that contains itself, a generic sum type's case whose payload is its parameter, a `Map` or a `Set` (0331). A command declares no timeout (0173). |
| A refusal is never silent | 0302, 0307, 0318 | A predicate has words of its own (PW0351). A refused press shows them beside the control and the page's status region announces them, and every other failed press says so too (0302). A control whose command requires a predicate with no parameters is shown only where the page asks that it holds (PW5048, 0307). A press from a tab whose reader signed in or out elsewhere is refused and told (0318); two windows remain where a retry of a committed press is refused as stale. The words are in one language, the first failing predicate's; a predicate with parameters (`OwnsPost`, `MayMessage`) is not asked of the reader (0302, 0307). |
| Privacy and per-user isolation | 0112, 0128, 0129, 0184, 0252, 0263, 0264, 0270, 0274, 0279, 0282, 0318, 0324 | Held by page, `/pw-read`, cache and stream. A label is not followed through storage or time (0129), nor through a materialization's body (0282). A private entry is told to its user's sessions alone where a binding fills its key with `current_user()`; one keyed otherwise reaches every reader of it (0318). `private` says only that a declaration is not importable; `user`, `session` and `public` say whose data it holds (0324). |
| A typed principal | 0263, 0264, 0270, 0318 | The host answers `current_user()`. A guest is a principal of its own; its cart joins the user's at sign-in, and a join that fails leaves the guest's lines reachable by no one (0318). |
| Streaming, out of order | 0148, 0223, 0272 | A region renders once per document, at a page's top only. Without scripts it fills only in Chrome 150 and later. |
| Static pages and SEO | 0114, 0183, 0186, 0189, 0293, 0311 | A page states its title, description, robots, Open Graph and Twitter metadata and, as a page the build checks, its canonical address, which the host writes absolute from the deployment's `PW_ORIGIN` and never from `Host` (0311). kiokun's title and description match kiokun.com's own code on 5,513 sampled words (0293). No alternate-language, icon or manifest link; no JSON-LD (the integrator ruled a typed `Json`, not built); no preview image (an image renderer waits for the owner). |
| Accessibility | 0141, 0143, 0182, 0185, 0302 | Narrow: only the store's pages are audited; no axe-core in the suite; no screen-reader run (KNOWN_LIMITATIONS). A refusal is announced by the page's `role="status"` region (0302). |
| Performance budgets | 0109, 0187, 0188 | The long-frame instrument is unstable on the development machine (E7-G8, open). |
| Uploads (images) | 0260, 0261 | PNG, JPEG, WebP and GIF; a directory as the blob store; one image a post; attaching loses the draft. |
| Pagination | tested under 0222, 0225, 0271, 0296 | Minimal: "Load more" reads again with a larger limit, with no cursor. WebKit's "Load more" failure on CI was the host's lock, and is fixed (0296). The store's search lists the first 20 stores and says how many more match; it does not page (0308). |
| Notifications | 0270, 0274, 0318 | A like, a reply and a follow notify; a mention does not, nothing is grouped, and there is no push and no email. A reader's mark-read reaches their other sessions and sends another user's open page nothing (0318). |
| Direct messages | 0279 | Not end to end encrypted (Missing, below). No blocking yet (NEXT). |
| Keyed reads and cancellation | 0029, 0151, 0152, 0224 | A view's signals and a stream's query are not keyed. |
| Recovery and reload | 0155, 0175, 0176, 0177 | |
| A command outliving its page | 0268 | Within the Fetch standard's 64 KiB kept alive. |
| Resumability | 0110, 0113, 0132, 0134, 0135, 0137, 0139, 0217, 0300 | A build is named by what it built, and the resume decision holds a document to the schema and scope its build says of its page (0300). Across deployments is risk R5, open: a page open across a deployment keeps its old table and document, and that skew is a deployment's (E8). Entries' generation is still the host's constant. |
| Capabilities, WIT components, Wasm | 0006, 0008, 0020, 0026, 0032, 0033, 0116, 0244, 0262, 0266, 0267, 0283, 0287, 0291, 0317 | A component imports each host operation its compiled code calls, and a handler's work is held where it runs, in the browser (0283, 0287). A call through a local is charged its namesake's effects, which refuses work and grants none (0283). A frame phase's work is still held to a page's declared placement (0287). A component is compiled once a process; a host that loaded many builds would keep each one's, and none reloads a build yet (0317). |
| Routing and not found | 0160, 0162, 0163, 0190, 0231, 0295, 0309 | A route's query part (`/search{?q}`) is typed text, `None` when the address leaves it out, and a plain GET form is checked against the route's names (0309). A page can say its address is another address of the page: 308 or 307 to its own route, carrying the route's one parameter and the address's query (0295). A move to another page, or one that adds to the query, is not expressed (0295); a list parameter and a typed number are not read (0309). A program cannot declare its own 404 view. |
| Navigation and layouts | 0160, 0280, 0300, 0303, 0306, 0312 | Links; `navigate` after a command commits, the next page read after the commit (0280). A page names its layout, and a navigation between two pages of one layout and one build keeps the layout in place and shows the next page in it, for a link, a `navigate` and back and forward (0303, 0306). One layout, unnested, given nothing, with one slot and no title template (0303). A page of another layout or build, or with none, is loaded whole; a loop or a view that contains itself is taken whole; no view transition and no prefetch; the document left stays on the host until it is idle, 120 seconds (0306). A stopped navigation is noticed in Chromium only (0280). A link's path arguments are untyped (0280). |
| XSS and CSRF | 0094–0097, 0258, 0333 | Every HTML answer carries a Content-Security-Policy generated from the build and the deployment (scripts and styles by hash, the payment provider framed only where a page frames it), every answer carries `nosniff`, and the build refuses a load from another origin (PW5052) (0333). A program cannot declare another origin; an address a value chooses loads from this origin alone, and nothing at build says so; no `report-to`, so a deployment hears of no refusal; no Trusted Types; one policy for a build's every page. |
| Time and the clock | 0180, 0304, 0328 | A value reads the wall clock only by comparing it (`passed`, `today_in`); a kept value holds until the earliest instant it compared, and open pages are told when the clock passes it; zones, dates and weekdays come from the host's tz database. A store's hours are its data and whether it is open is the program's: pages show it, and an order is refused while it is closed (0328). No countdown ("closes in 12 minutes"): no value of now is shown. A handler reads no clock; a page built once and a public materialization cannot compare it; one host's clock, moved forward only; hours look a week ahead; `clock.now()` still answers 0. |
| Scheduled commands | 0325, 0334 | `invoked_by Clock`, `schedule` and `unschedule`: a command runs when the clock passes its time, once, as the user who scheduled it, its answer committed beside its writes. Runs are kept in the database (or the materializer); a failed run is retried (two seconds, doubling to a minute, five times), then kept, said and counted in `/metrics`. Its first use: an order unpaid for thirty minutes is taken back. No recurring schedule; a run starts within about a second of its time; a program reads no time to compute one; an effect outside a run's transaction, the provider's cancel, may be asked twice by two hosts. |
| Money | 0169, 0195 (ruling 6), 0327, 0329 | `Money<USD>` is an opaque amount; a part of an amount is taken only at a named rate and rounding (half up, half even, toward zero, away from zero). An order is priced by the program from each store's fee schedule and tax rate, the reader's tip and promo code, and the reader pays the total they were shown or is refused (0329). USD alone; no conversion; `display` is en-US and the store's own code; no discount allocated across stores; a tax rate that changes on a date, or follows a district, is not modelled; each store's tax flags are a deployment's tax adviser's to set. |

## Partial

| Capability | ADRs | What works | What does not |
|---|---|---|---|
| Accounts | 0258, 0263, 0264, 0265, 0270, 0318 | Pleris is the relying party (PKCE, CSRF); a typed principal; the feed and the store both use accounts, and a guest's cart joins the user's at sign-in (0318). | A development provider only, which refuses to run outside a loopback development deployment, and no other provider exists; no OIDC discovery, JWKS or ID-token check; sessions in memory (`track/second-host` is persisting them); no email, recovery, account linking or sign-in rate limits. Google is the owner's choice of first real provider, through a generic OIDC relying party (NEXT, the production path). |
| Forms | 0142, 0143, 0221, 0227, 0265, 0309, 0319, 0331 | `bind:value` for a `String`, a textarea, labels, routes checked; a plain GET form checked against its route (0309); a handler sends a sum type (0331). The store saves, renames, removes and chooses delivery addresses, and checks out, with these (0319, 0329). | No checkbox or `<select>` binding; a form as one typed record waits (0131); nothing multi-step. |
| Multi-tenancy | 0107, 0128, 0263, 0264 | A tenant in a cache key; an `Organization` label. | `current_organization()` is bound to `pw:host/organization#read`, which no host answers; no tenant model. |
| Database independence | 0005, 0019, 0207, 0218, 0246, 0298, 0308, 0318, 0325, 0334 | The feed and the store each run in memory and on PostgreSQL behind one seam (the store's operations are written once over `Rows`); a source states its guarantees and the host holds the layer to them. The store's whole server suite runs on either layer. Search, interaction records, scheduled runs and owners' rows are in both layers. | Each data layer is written by hand in Rust, about 5,800 lines in `store.rs`, `store_pg.rs`, `feed.rs` and `feed_pg.rs`; PostgreSQL is the only database; one host (0246); the browser suite runs the store in memory; the store's reads grow with the data on PostgreSQL until NEXT 4b and 4c land (in flight). |
| Third-party APIs | 0026, 0148, 0262, 0332 | `network.fetch` declared and granted; a host binding's shape checked. The host calls the payment provider over plain HTTP, waiting ten seconds at most, and checks the signature of the provider's webhooks (0332). | The host has no general outbound HTTP client and no TLS, so no real provider (a Stripe test-mode adapter waits for the owner to approve its crates); the recommender is simulated in process; nothing sends a webhook. |
| Payments | 0332, 0334 | A payment flow against a development provider: a command the provider's signed word invokes settles an order (`invoked_by Payments`, HMAC-SHA256, five minutes either way); an order awaits its payment, and is taken back by its reader or by a sweep thirty minutes on; a provider that cannot be reached places nothing; the host holds the key and a program never does. | No real provider; no refunds, held captures or disputes; the reader's return is no second trigger; the provider is asked inside the command's transaction, so an intent begun for a command that then rolls back stays at the provider; `Secret<C>` does not build in a program; `examples/lib/Payments.pw`'s `capture` and `charge` are still `todo` fixtures. |
| Search | 0037, 0041, 0207, 0308, 0309, 0313–0316 | kiokun: a search page over the whole dictionary (834,036 rows; kiokun.com's two FTS5 statements run in SQLite behind host operations, and the term expansion, grouping, scoring and page are Pleris), the reading index and homophones, and deinflection by rules of our own, each held to kiokun.com's own code where it runs alone (for the search, 351 of 355 sampled queries answer the same, and kiokun.com fails the other four). The store: stores and dishes found by name, cuisine and category in the data layer, on memory and PostgreSQL, to one stated meaning, from a plain GET form, filtered to the stores that reach the reader's chosen address (0308). | No search source an app declares: "a program cannot yet declare a full-text index or its ranking" (0314). The store's search folds no accent or stem, corrects nothing, ranks by no distance, rating or popularity, does not page, and reads every row (`LIKE`, no index). kiokun's `/api/search` has no route form (Pleris has none that answers JSON); Korean is not deinflected. |
| Materializations | 0019, 0103, 0176, 0255, 0273, 0277 | A value its body derives, kept, and made again in order. | Public ones only; made again whole; `regenerate on_read` not run. |
| Structured concurrency and affine resources | 0016, 0045, 0211, 0250, 0251, 0269 | Checked statically (PW20xx). | The `pw-tasks` runtime is not wired into the host. |
| Layout and frame phases | 0187; R-034, R-038, R-040, R-043 | Checked at compile time. | No frame scheduler in the runtime: no `requestAnimationFrame` or observers. |
| Developer experience | 0013, 0149, 0276, 0321 | The compiler's feedback: `pw check` (coded diagnostics with spans, and notes shown in the file they belong to), `pw explain` (types, effects, privacy, placement), `pw fmt` (one canonical layout), `pw diff` (what a change means), `pw build`. | No guide or tutorial on `master` (`track/guide` is writing one, with a tutorial app that already checks and is served in a browser test); no editor support or language server ("none exists yet", 0321; the charter asks for one); no `pw new`; no live reload (a host serves one build, 0317); a small standard library (no `split`, no `range`); no debugging aid beyond `PW_TRACE` lines, `GET /metrics` and the development server's per-document trail (0296). |

## Missing

| Capability | What exists | Covered by |
|---|---|---|
| Admin tables: sort, filter, bulk edit | `List.sort_by` and `filter` (0040); a table's nesting checked (0204). No sortable table, no bulk command, no checkbox binding. | The SaaS admin app. |
| Email, outgoing webhooks and recurring jobs | A `task` declaration kind is parsed and does nothing (its signature skipped, Koka refusing it); `task.spawn` is checked statically (PW2001–PW2003); `pw-tasks` (0016) has Rust tests only, no timers, and the host does not use it. A command can be scheduled to run once (Built, 0334); there is no recurring schedule, no email and no outgoing webhook. The host hears one kind of webhook, the payment provider's, signed (0332). | The SaaS admin app. |
| Charts and dashboards | None. | The SaaS admin app. |
| i18n and l10n: locales, currencies, plurals, RTL | A locale as a cache key's dimension only (0056, 0169, 0180, 0186, 0255). A program has one set of words for a refusal (0302); `display` writes US dollars as en-US does (0169). | The SaaS admin app. |
| Real-time collaboration and offline | `replicated` is parsed only (A-011); charter §9.1's kind 4. A design note (`track/docs-design`) is in flight. | The Google Docs-shaped app. |
| Maps, canvas and custom widgets | A mounted resource is refused by the build (0075). | The Google Docs-shaped app's editor-surface ruling. |
| End-to-end encryption | None: a device-only placement label, browser-placed computation and rendering, and device-local storage are each missing. | The end-to-end-encrypted messages after the Docs app. |
| Video and media | Uploads are images only (0260). | No planned app yet. |
| Experiments, feature flags, analytics events | `Analytics.pw` and `Telemetry.pw` are `todo` stubs. | No planned app yet. |
| AI features: a model's tokens streamed into a region | A region renders once per document (0148). | No planned app yet. |
| A second host, horizontal scale | A second host on one database hears nothing of the first's commits (0246). A session and a sign-in under way are in one host's memory (0258), and a page's command order is its host's (0305). An interaction's answer is committed with the writes (0325), and a scheduled run is keyed the same on every host (0334). | `track/second-host` (in flight). |
| Production | Pieces, not a deployment: `PW_DEPLOYMENT` and `PW_ORIGIN`, a generated Content-Security-Policy, SQL migrations applied at start, `GET /metrics`. The host is the development server (notes below). | kiokun.com, the owner's production target (ADR-0285): served first on a preview origin, with no cutover without the owner's go (NEXT). |
| Interop: JavaScript components and npm libraries in a page; incremental adoption into an existing site | Nothing loads JavaScript into a page, and nothing adopts Pleris into an existing site (notes below). | The Google Docs-shaped app's mounted editor (a design in `track/docs-design`) is the first use of a resource. |
| Testing tools for app authors | `pw check` and `pw fmt --check`; no `test` declaration, no `pw test`, no fixtures or browser harness for an author (notes below). | No planned app yet. |
| A data layer declared in Pleris, not hand-written Rust per app | A `source` declares guarantees only (0207); the layer behind it is about 5,800 lines of Rust for the feed and the store (notes below). | The app on three kinds of database (a design). |

### The new rows, in detail

**Production**

- `PW_DEPLOYMENT=production` and `PW_ORIGIN` exist. A production deployment
  whose pages state a canonical address, and states no origin, is refused at
  start (0311).
- A Content-Security-Policy is generated from the build, and every answer
  carries `nosniff` (0333).
- SQL migrations, each layer's own (store 0001 to 0015, feed 0001 to 0009),
  are applied in order at start under an advisory lock, and recorded.
- `GET /metrics` answers how often each data-layer operation ran, how many
  values are kept and how scheduled runs ended (0127, 0334). A webhook's
  signature is checked (0332). A command's body is bounded to 64 KiB.
- The host is the development server, "NOT E8's production host" in its own
  Cargo.toml. There is no container image or deployment target.
- There is no TLS: the host speaks plain HTTP, a thread for each connection.
  It compresses nothing, and sends the runtime as written, unminified (0188).
  There are no causal traces (charter §10.5); logs are lines on stderr. No
  rate limit exists but an upload budget (0260).
- Sessions live in one host's memory (`track/second-host` is moving them),
  and the identity providers are development ones (0258); Google's is queued
  in NEXT.
- Secrets: the payment provider's key is the host's and no program holds one
  (0332), `Secret<C>` does not build in a program, and the development
  provider makes its webhook secret at start. Nothing manages a deployment's
  secrets.
- Queued in NEXT and not built: migrations across live deploys, secrets
  management, rate limiting and backups. Wasmtime's advisories are being
  fixed in `track/wasmtime-48-0-6`.

**Interop**

- A page writes no script (0094), the generated policy allows scripts from
  this origin only (0333), a program cannot declare another origin, and a
  build has no assets. So a JavaScript library cannot be loaded into a page.
- A mounted resource, the charter's boundary for imperative widgets, is
  refused by the build (0075). A custom element can be written in lowercase
  (0072), and nothing loads its code.
- Handlers compile to JavaScript the runtime loads: Pleris out, not
  JavaScript in.
- For adoption into an existing site: every page is a whole document the
  host serves, and no ADR or design names an embedded or partial mode.

**Testing tools for app authors**

- `pw check` and `pw fmt --check` run on an app's program.
- The reference apps' test controls are their own: `/bench/clock` moves the
  host's clock forward (0304), `/bench/records` answers a document's trail
  (0296), and `store::Faults` injects delays and failures into the store.
- The language has no `test` declaration and the CLI no `pw test`. There are
  no fixtures or fake-data helper, no browser harness shipped to authors, and
  no coverage.
- In flight: `track/guide` compiles its examples with a test and drives its
  tutorial's app in browsers, and `track/todomvc` holds TodoMVC to the
  upstream Cypress suite. Each tests Pleris or one app, not a tool an author
  runs.

**A data layer declared in Pleris**

- A `source` declares guarantees only (0207): `FeedData`, `StoreData`,
  `KiokunEntries`, `KiokunSearch`.
- The layer behind it is Rust in the host: about 5,800 lines in `store.rs`,
  `store_pg.rs`, `feed.rs` and `feed_pg.rs`, written once for memory and once
  for PostgreSQL, with SQL files for the schema and the seed data inside
  them.
- Replacing `store:data/carts` with compiled Pleris is E10-S10, open in the
  evidence ledger since E10.
- `track/threedb-design` proposes tables, keys, indexes and references
  declared in Pleris, one generic adapter per database, and migrations
  generated from the declarations. It is a design.

## Not built here

**Charter §2, quoted:** "Do **not** initially build any of the following: a
database engine; a SQL optimizer; a new TCP or QUIC implementation; a
complete browser engine from scratch; a CSS layout engine; a global CDN; a
distributed database; a package registry; a bespoke binary replacement for
HTML; a custom router appliance; a local foundation model; a universal CRDT
system; a full production framework before proving the semantic model."

- "A custom router appliance" is network hardware, not URL routing or
  dispatch.
- `docs/vision/non-goals.md` heads this list "Never build", which is
  stronger than the charter's "initially". That file says a change to it
  needs an ADR and a human decision, so it is recorded here and not edited.

**The owner's scope for the DoorDash app** (relayed 2026-10-08): courier
dispatch, routing and fraud are the backend's services, outside the app
layer. Payment processing is a provider's: Pleris holds a capability, and
never card data.

**Reused, not built:** a search engine (the database's, or a declared search
source), and a CRDT or OT library behind `replicated`.

## In flight

Branches being built now (2026-10-15, about 01:40 UTC). None is on `master`.
A row moves when its branch merges with its ADR and its evidence, and not
before; what a line says here is the branch's aim, and its state as its
commits show it at that hour. The branches move; `git log` on each says
more.

| Branch | What it builds, and where it stands | Rows it moves |
|---|---|---|
| `track/scheduled-orders` | An order placed for a time its reader chooses: a window picked at checkout, the order held and sent to the store ahead of it by a command the clock runs (ADR-0334's `schedule`). Checker and server tests, the store's statement counts re-pinned, a 41-mutant script (not yet run) and its ADR are written; not green. | Scheduled commands (a second use); time and the clock; forms (the picker). |
| `track/reviews` | A rating of one to five stars and an optional comment on each store of a delivered order, once, public without the reader's name. Built on both data layers with its server tests green and its reads' statements pinned on PostgreSQL; work in progress. | Forms; privacy (reviewers unnamed); search (a store's rating). |
| `track/render-merge` | NEXT 4c, first step: one render's identical reads made once, from one snapshot (a read-only repeatable-read transaction on PostgreSQL). Tested on memory and PostgreSQL; its ADR is written. | Database independence; reading and caching. |
| `track/bounded-checker` | NEXT 4c's checker: a read inside a loop is refused by construction (PW0405, with a warning, PW0406), so an N+1 read does not compile. The checker, 23 tests, a corpus of refused loops and a 33-mutant script are written; work in progress. | Database independence (a read's cost bounded by the language); performance. |
| `track/bounded-statements` | NEXT 4b: the store's reads are set reads, and the statements of each of the store's 63 data operations, and of each page and command, are pinned by test and equal at two data sizes. Ready for the next train. | Database independence; performance. |
| `track/todomvc` | TodoMVC in Pleris, held to its spec and to its upstream Cypress suite. So far: a page binds a signal to its address's fragment (PW61xx), the platform declares a double press and a control losing focus, a patch that checks or unchecks a box sets the box's own state, the app, a Playwright spec and mutation controls. The Cypress run and the rest of the runtime fixes are left. | Routing (a fragment); forms; testing tools; the head-to-head comparison. |
| `track/second-host` | Hosts sharing one database: each hears every commit from the outbox, and a session is the database's. Idempotency already commits with the writes (ADR-0325); the track keeps it so across hosts. The design is written and the outbox is built as a log in commit order that every host reads from its cursor; tests on two hosts, persisted sessions and browser tests on two ports are left. | Reading and live updates; accounts (sessions); a second host. |
| `track/docs-design` | A design note, no code: the Google Docs-shaped collaborative app (Automerge in the host and in the editor's page, ProseMirror mounted as a resource, sync and presence down the stream). | Real-time collaboration and offline; maps, canvas and custom widgets. A design only. |
| `track/threedb-design` | A design note, no code (1,303 lines): an app's data declared in Pleris, one generic adapter per database, migrations generated from the declarations, then the store across SQLite, MongoDB and Cassandra. | A data layer declared in Pleris; database independence, past SQL. A design only. |
| `track/guide` | The language guide and tutorial, every example compiled by a test with the real compiler. So far: the example checker, a generated reference of the diagnostic codes, a "Why Pleris" page whose refusals compile, and a tutorial guestbook that checks clean and is served in a browser test; the rest of the guide is not written. | Developer experience. |
| `track/proof` | The bug museum (each exhibit a census leaf with a dated primary source, reproduced in Next.js or SvelteKit, refused or missed in Pleris, with controls) and the head-to-head comparison. So far: the plan, a runner and four exhibits (J08 shown end to end; F01 and G01 written; H07, an N+1 read, written as a miss); no head-to-head measurement yet. | The evidence that answers "is Pleris perfect?", which is not a row. |
| `track/wasmtime-48-0-6` | Wasmtime from 48.0.5 to 48.0.6, for RUSTSEC-2026-0353 to 0364. | Production (the supply chain). |

## The reference apps, by the rows they cover

In the owner's order (NEXT), with where each stands:

1. **The DoorDash customer side**: built on `master` except two pieces.
   - Built: client-side navigation (0306), accounts in an app (0318),
     database independence (the store on PostgreSQL, 0298), search (0308),
     time and the clock (store hours, 0304, 0328), money (0327, 0329),
     payments against a development provider (0332), forms (addresses,
     0319, and checkout), and a command that runs at a time (an unpaid
     order taken back, 0334).
   - In flight: ratings and reviews (`track/reviews`) and orders scheduled
     for a time (`track/scheduled-orders`).
   - Its places are a fixture of eleven, not a geocoder, and its courier
     minutes a model (0319).
2. **The bug museum, extended, and the head-to-head comparison**: the
   evidence that answers "is Pleris perfect?", not breadth. A plan is in
   flight (`track/proof`); nothing is built.
3. **A Google Docs-shaped app**:
   - real-time collaboration and offline;
   - presence;
   - a mounted editor surface;
   - sharing and permissions;
   - version history.

   A design note is in flight (`track/docs-design`).
4. **End-to-end-encrypted messages**:
   - a device-only placement;
   - browser-placed computation;
   - device-local storage.
5. **A multi-tenant SaaS admin**:
   - multi-tenancy and roles;
   - admin tables;
   - charts;
   - i18n;
   - background jobs and email;
   - webhooks;
   - an audit log.
6. **The app on three kinds of database**: database independence, past
   SQL, and a data layer declared in Pleris. The owner approved its
   databases (NEXT): SQLite, libSQL or D1; MongoDB or Firestore; Cassandra
   or ScyllaDB; and, optionally, CockroachDB. A design note is in flight
   (`track/threedb-design`).

Beside that order:

- **kiokun.com** is the owner's production target (ADR-0285). Its word page
  (labels, character header, written forms, examples, pitch accent,
  Contains and Appears in, homophones, deinflection) and its search page are
  built on the development server and held to kiokun.com's own code where
  it runs alone. ADR-0285's inventory counts 102 routes (none built, five
  partial) and 62 features (17 built, 10 partial, 35 missing as of
  2026-10-11). Static-data pages, accounts, in-browser features and outside
  services are not started. Its rows: search, static pages and SEO, routing.
- **TodoMVC** is in flight (`track/todomvc`).

**Rows no planned app covers yet**: video and media; experiments, flags and
analytics; AI token streaming. They decide the app after these. Interop and
testing tools for app authors are exercised by no reference app either.
Production is kiokun.com's, when the owner says go; a declared data layer is
the three-database app's; developer experience is `track/guide`'s.

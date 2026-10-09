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

Checked against `master` at `73e462b` (2026-10-08), ADR-0001 to ADR-0279.

## Built

| Capability | ADRs | Limits |
|---|---|---|
| Reading, caching and live updates | 0007, 0100–0103, 0107, 0127, 0128, 0145, 0146, 0177, 0219, 0224, 0271 | One host: a second hears nothing of the first's commits (0246). |
| Commands: optimistic, idempotent, retried | 0025, 0121, 0122, 0154, 0157, 0159, 0173, 0222, 0236, 0238, 0268, 0275 | Idempotency is kept in the host's memory, not with the writes (0121, 0246). A command declares no timeout (0173). |
| Privacy and per-user isolation | 0112, 0128, 0129, 0184, 0252, 0263, 0264, 0270, 0274, 0279, 0282 | Held by page, `/pw-read`, cache and stream. A label is not followed through storage or time (0129). |
| A typed principal | 0263, 0264, 0270 | The host answers `current_user()`. |
| Streaming, out of order | 0148, 0223, 0272 | A region renders once per document, at a page's top only. Without scripts it fills only in Chrome 150 and later. |
| Static pages and SEO | 0114, 0183, 0186, 0189 | No canonical or alternate-language link, and no JSON-LD. |
| Accessibility | 0141, 0143, 0182, 0185 | Narrow: only the store's pages are audited; no axe-core in the suite; no screen-reader run (KNOWN_LIMITATIONS). |
| Performance budgets | 0109, 0187, 0188 | The long-frame instrument is unstable on the development machine (E7-G8, open). |
| Uploads (images) | 0260, 0261 | PNG, JPEG, WebP and GIF; a directory as the blob store; one image a post; attaching loses the draft. |
| Pagination | tested under 0222, 0225, 0271 | Minimal: "Load more" reads again with a larger limit, with no cursor. WebKit's "Load more" still fails on CI at times (NEXT, open). |
| Notifications | 0270, 0274 | Reads again more pages than it must, a cost, not a leak. No push, and no email. |
| Direct messages | 0279 | Not end to end encrypted (Missing, below). No blocking yet (NEXT). |
| Keyed reads and cancellation | 0029, 0151, 0152, 0224 | A view's signals and a stream's query are not keyed. |
| Recovery and reload | 0155, 0175, 0176, 0177 | |
| A command outliving its page | 0268 | Within the Fetch standard's 64 KiB kept alive. |
| Resumability | 0110, 0113, 0132, 0134, 0135, 0137, 0139, 0217 | Across deployments is risk R5, open. A manifest's document and scope are constants that the decision checks against themselves (found 2026-10-08, NEXT). |
| Capabilities, WIT components, Wasm | 0006, 0008, 0020, 0026, 0032, 0033, 0116, 0244, 0262, 0266, 0267 | |
| Routing and not found | 0160, 0162, 0163, 0190, 0231 | A program cannot declare its own 404 view. |
| XSS and CSRF | 0094–0097, 0258 | No Content-Security-Policy is generated (KNOWN_LIMITATIONS). |

## Partial

| Capability | ADRs | What works | What does not |
|---|---|---|---|
| Accounts | 0258, 0263, 0264, 0265, 0270 | Pleris is the relying party (PKCE, CSRF); a typed principal. | A development provider only, on loopback; no OIDC discovery, JWKS or ID-token check; sessions in memory; no email or recovery. Google is the owner's choice of first real provider, through a generic OIDC relying party (NEXT, the production path). |
| Forms | 0142, 0143, 0221, 0227, 0265 | `bind:value` for a `String`, a textarea, labels, routes checked. | No checkbox or `<select>` binding; a form as one typed record waits (0131); nothing multi-step. |
| Multi-tenancy | 0107, 0128, 0263, 0264 | A tenant in a cache key; an `Organization` label. | `current_organization()` is bound to `pw:host/organization#read`, which no host answers; no tenant model. |
| Database independence | 0005, 0019, 0207, 0218, 0246 | The feed in memory and on PostgreSQL behind one seam; a source states its guarantees. | The store is in memory only; each data layer is written by hand in Rust; one host. |
| Third-party APIs | 0026, 0148, 0262 | `network.fetch` declared and granted; a host binding's shape checked. | The host has no outbound HTTP client; the recommender is simulated in process. |
| Numbers and money | 0169, 0195 (ruling 6) | Exact integers; `Money`'s en-US USD display. | The display is the store's own code, not the platform's; USD only; no locale's format. |
| Search | 0037, 0041, 0207 | Kiokun's search page over one shard (the index in Rust, the ranking in Pleris); a declared `source Search` in tests. | No search over the store or the feed; no declared search source in an app. |
| Materializations | 0019, 0103, 0176, 0255, 0273, 0277 | A value its body derives, kept, and made again in order. | Public ones only; made again whole; `regenerate on_read` not run. |
| Structured concurrency and affine resources | 0016, 0045, 0211, 0250, 0251, 0269 | Checked statically (PW20xx). | The `pw-tasks` runtime is not wired into the host. |
| Time and the clock | 0180 | `Instant`; `clock.read` and `clock.wall` typed. | `clock.now()`'s body is `{ 0 }`, and no host answers it. |
| Layout and frame phases | 0187; R-034, R-038, R-040, R-043 | Checked at compile time. | No frame scheduler in the runtime: no `requestAnimationFrame` or observers. |
| Navigation | 0160, 0280 | Links, each a document load, matched to routes (PW5009); a handler going to a page once its command commits, the page's parameters typed, read after the commit (0280). | No client-side navigation: every navigation is a document load, so the parts two pages share are not kept in place (the next ruling). A link's arguments are untyped. |

## Missing

| Capability | What exists | Covered by |
|---|---|---|
| Client-side navigation, the parts two pages share kept in place | Nothing: every link is a document load. | The DoorDash customer side's first item (NEXT). |
| Payments | A `Payments` capability tag; `examples/lib/Payments.pw`'s functions are `todo`; no payment code in the host (0193 claims none). | DoorDash's checkout. |
| Admin tables: sort, filter, bulk edit | `List.sort_by` and `filter` (0040); a table's nesting checked (0204). No sortable table, no bulk command, no checkbox binding. | The SaaS admin app. |
| Background jobs, schedules, email, webhooks | A `task` declaration kind is parsed and does nothing (its signature skipped, Koka refusing it); `task.spawn` is checked statically (PW2001–PW2003); `pw-tasks` (0016) has Rust tests only, no timers, and the host does not use it. | The SaaS admin app. |
| Charts and dashboards | None. | The SaaS admin app. |
| i18n and l10n: locales, currencies, plurals, RTL | A locale as a cache key's dimension only (0056, 0169, 0180, 0186, 0255). | The SaaS admin app. |
| Real-time collaboration and offline | `replicated` is parsed only (A-011); charter §9.1's kind 4. | The Google Docs-shaped app. |
| Maps, canvas and custom widgets | A mounted resource is refused (0075). | The Google Docs-shaped app's editor-surface ruling. |
| End-to-end encryption | None: a device-only placement label, browser-placed computation and rendering, and device-local storage are each missing. | The end-to-end-encrypted messages after the Docs app. |
| Video and media | Uploads are images only (0260). | No planned app yet. |
| Experiments, feature flags, analytics events | `Analytics.pw` and `Telemetry.pw` are `todo` stubs. | No planned app yet. |
| AI features: a model's tokens streamed into a region | A region renders once per document (0148). | No planned app yet. |
| A second host, horizontal scale | A second host on one database hears nothing of the first's commits (0246). | No planned app yet. |

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

## The reference apps, by the rows they cover

In the owner's order (NEXT):

1. **The DoorDash customer side**:
   - client-side navigation;
   - accounts in an app;
   - database independence (the store on PostgreSQL);
   - search;
   - time and the clock (store hours);
   - money;
   - payments;
   - forms (addresses and checkout).
2. **The bug museum, extended, and the head-to-head comparison**: the
   evidence that answers "is Pleris perfect?", not breadth.
3. **A Google Docs-shaped app**:
   - real-time collaboration and offline;
   - presence;
   - a mounted editor surface;
   - sharing and permissions;
   - version history.
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
   SQL. The owner approved its databases (NEXT): SQLite, libSQL or D1;
   MongoDB or Firestore; Cassandra or ScyllaDB; and, optionally,
   CockroachDB.

**Rows no planned app covers yet**: video and media; experiments, flags and
analytics; AI token streaming; and a second host. They decide the app after
these.

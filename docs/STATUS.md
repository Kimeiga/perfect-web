# STATUS

<!-- Charter §3.4 requires exactly these sections. Keep them. -->

**Reviewed:** 2026-10-03, against master `25e379d`, with ADR-0170.
**Charter:** v2, `PROJECT_CHARTER.md`.
**Numbering:** engineering E0-E15, public proofs P0-P9, risk experiments RQ-*.

**Current milestone:** E14, the AI benchmark, ahead of E11-E13 by the
owner's ruling of 2026-10-02
([ADR-0119](DECISIONS/ADR-0119-e10-closes-and-the-ai-benchmark-comes-next.md),
[plan](milestones/E14.md)). It starts with the Next.js and SvelteKit stores
and an offline harness; no model is called until the owner chooses the models
and budget.

**Correction, ADR-0170, 2026-10-03: a loop over a list inside a query's
value built, and failed at its first change**
([ADR-0170](DECISIONS/ADR-0170-a-list-inside-a-querys-value.md)).
- **What was wrong.**
  - The plan recorded `{#each cart.lines}`'s binding, `cart`, as the list,
    and the development server could not show a session's document once the
    cart changed.
  - A row's member read over such a list was refused. A value computed for a
    row went inside a field, and a number such as `quantity` has none.
  - The speculation module looked at text alone. A loop, a block or an
    attribute that read the speculated cart would have kept its old value
    while the count moved, and nothing said so.
- **Now** a list is recorded by its path, and its rows read members of their
  item. A computed value is set in the row whole, by its path. A part a
  speculation would not reach is refused by name.

6 mutants (`just e14-nested-lists`).

**Correction, ADR-0169, 2026-10-03: a page could build and then fail to
render** ([ADR-0169](DECISIONS/ADR-0169-a-member-read-is-computed-or-refused.md)).
- **What was wrong.**
  - A template value read through a member function, as
    `{item.price.display}` reads `display`, was planned in one place only
    (ADR-0125). Elsewhere it was neither planned nor refused: in a loop's
    row, in an attribute, in what a block decides by, or in a loop's list.
    The page built, and failed when it was rendered. The plan's comment said
    such a read was refused; nothing refused it.
  - The renderer read one field after a name, so `item.price.display`
    named nothing.
  - The data layer priced every cart line at $4.50, whatever its item.
- **Now**
  - a loop's row reads members of its item: for a loop over a query's
    list, a host computes each one for each row;
  - every other member read no host computes is a build refusal, naming
    the part and the path;
  - the renderer reads a path field by field;
  - the store shows each item's price, `$3.50`, as en-US writes dollars,
    exact for every amount, and the data layer prices a line at its item's
    price.
- **Also** a transport test assumed the cart's frame came before the
  menu's, and read one entry under load. It waits for both now.

23 mutants (`just e14-row-reads`).

**Correction, ADR-0168, 2026-10-03: a change did not reach an attribute, and
E7-P could scramble its list**
([ADR-0168](DECISIONS/ADR-0168-a-change-reaches-every-part-that-reads-it.md)).
- **What was wrong.**
  - With each Add named by its item (`aria-label="Add {item.name}"`), a
    renamed item kept its old button name. A host set a changed item's text
    parts in place and no attribute; the runtime applied no `SetAttribute`.
  - E7-P's runtime moved an instance to where it already was, and
    `moveBefore` then put its nodes before their own start. The list's
    anchors no longer nested, and the next patch addressed nothing. A
    single patch that addressed nothing was ignored, which hid it.
- **Now**
  - the renderer says what changed in an instance, part by part;
  - a host sets text with `ReplaceText` and attributes with `SetAttribute`,
    nodes kept;
  - the runtime applies attribute patches, read by the browser's own
    parser;
  - an instance already in place is not moved.
- **Also** each Add is named by its item, and the cart's count is said in a
  polite live region (§15.6 test 14). The menu control decodes `%26`.

Nine mutants (`just e14-instance-changes`).

**Correction, ADR-0167, 2026-10-03: markup was read as code**
([ADR-0167](DECISIONS/ADR-0167-markup-text-is-text.md)).
- **What was wrong.** The lexer does not know markup, and read every `//` in
  it as a comment that ran to the line's end. None of these was reported:
  - a `//` line between elements became page text;
  - `<p>http://example.com</p>` did not parse;
  - `<a href=http://x.y>` built `href="http"` and broke the element after
    it;
  - `<!-- x -->` built a nameless element.
- **Seven rejected-corpus fixtures** (R-004, R-016, R-018, R-019, R-021,
  R-023, R-024) had shown their own `// ERROR:` notes as page text.
- **Now** the parser reads markup text, comments and unquoted attribute
  values from the source, as HTML reads them. A comment in markup is
  `<!-- -->`, and no page renders it. PW5028 refuses a line of text that
  begins with `//` or `/*`.

13 mutants (`just e14-markup-comments`).

**Correction, ADR-0166, 2026-10-03: ADR-0165's browser suite ran on a stale
build** ([ADR-0166](DECISIONS/ADR-0166-the-store-and-its-items-say-what-they-are.md)).
- **What was wrong.** To see whether the estimate's position changed when
  WebKit paints, it was moved below the cart and the page rebuilt. The source
  was put back without rebuilding, and the full suite ran on the experiment's
  build. ADR-0165 reported "518 passed" for a page the sources no longer
  described. Against the committed page, two `store.spec.mjs` tests that
  named the menu loop's part by number fail in every engine. ADR-0165's own
  evidence builds first, and stands.
- **Now** `run.sh` records each source's SHA-256, and the browser suite
  refuses a build whose sources have changed since. The two tests find the
  menu's tokens by where they are.

**ADR-0166, 2026-10-03: the store and its items say what they are**. Charter
§15.1's descriptions:
- `Store` and `MenuItem` declare a `description`, and the store's page shows
  each;
- a host's answer is read through the type the importing component
  declares. Fields it does not name are not passed in, and one it names
  that is missing is refused by name. One data layer serves the canonical
  store and the benchmark's frozen copy.

WebKit now paints the store before its slots are filled: first paint at
16-32 ms, where it was 2527 ms. §15.6 tests 3 and 17 hold in all three
engines. `price` waits for a ruling on showing money as text. Eight mutants
(`just e14-descriptions`).

**ADR-0165, 2026-10-03: the store's delivery estimate and recommendations**
([ADR-0165](DECISIONS/ADR-0165-the-store-s-estimate-and-recommendations.md)).
The audit's third gap, charter §15.3's slots:
- the canonical store streams its delivery estimate, under its name, and its
  recommendations, after its cart, each in a named region;
- the recommendations are public, kept ten minutes, and dropped when the
  store's menu changes, an event that now reaches a stream's kept answer.

§15.6 tests 3 and 17 hold in Chromium and Firefox.
- **Not in WebKit.** WebKit paints a page once it holds about 200 characters
  of text, and the store holds about 90. Measured: the runtime was ready at
  47 ms and the first paint came at 2527 ms, when the slots were filled. A
  Safari user sees nothing until then.
- `e2e/slots.spec.mjs` marks both gaps `fixme`. The fix is §15.1's
  descriptions and prices, the audit's eighth gap, next.
- **Also found:** a `//` line inside markup is rendered as text, as in JSX.

Seven mutants (`just e14-slots`).

**Correction, ADR-0164, 2026-10-03: a menu change drops what declares it, for
its store** ([ADR-0164](DECISIONS/ADR-0164-a-menu-change-drops-what-declares-it.md)).
- **What was wrong.** ADR-0162 recorded §15.6 test 11, "`MenuChanged(store_47)`
  invalidates store 47 only", as met. It was met for pages. But the server's
  query cache was dropped by the query's name: a change to store 47's menu
  dropped every store's kept menu, and store 48's next document read its
  menu again. A server test failed: three reads where two are right.
- **Now** the store's `Menu` declares `invalidates_on MenuChanged(id)`. The
  change is the event `MenuChanged(47)`, and it drops what the program says
  depends on it, for store 47 only. Five mutants (`just e14-menu-changed`).

**ADR-0163, 2026-10-03: a page says when it is absent**
([ADR-0163](DECISIONS/ADR-0163-a-page-says-when-it-is-absent.md)). The last
step of the route gap:
- a page declares the error that means its address names nothing, and the
  store declares `not_found_on StoreError.NotFound`;
- PW0342 holds the clause to a case of a declared error that a query the page
  reads with `let` can fail with;
- the plan carries the case on the bindings that can answer it. The
  development server answers `/stores/999` 404, with a page of its own;
  any other failure is still 503.

**Correction, found on the way:** every response the development server wrote
said `OK`, a 404 and a 503 among them. Each status now has its own reason
phrase. Seventeen mutants (`just e14-not-found`).

**ADR-0162, 2026-10-03: each store at its route**
([ADR-0162](DECISIONS/ADR-0162-each-store-at-its-route.md)). The audit's route
gap, charter §15.3, is closed:
- the development server routes a path by the plans' routes, and serves the
  store at `/stores/{id}`;
- each document reads its own parameters;
- a second store, Harbor Coffee, has a menu fragment of its own.

§15.6 test 11 runs end to end in three engines: a change to store 47's menu
reaches its page, and store 48's page is as it was. An unknown store is
still answered 503, as a page whose queries fail. Answering it 404 needs the
page to say which error means absent, the next ruling. Seven mutants
(`just e14-stores`).

**Correction, ADR-0161, 2026-10-03: each document is its own subscriber**
([ADR-0161](DECISIONS/ADR-0161-each-document-is-its-own-subscriber.md)).
- **What was wrong.** The development server kept one subscriber per session,
  and serving a document cleared it. A change waiting for one tab was lost
  when the session's second tab was served: a server test failed with "the
  first tab was never sent the add".
- **Now each served document has its own subscriber**, record of what it
  shows, and keyed reads. A change reaches every document of its session,
  and two tabs each hear the other's add, in three engines.
- **Found on the way:**
  - the browser suite had served a build made with an older runtime;
  - a mutation run stopped from outside could leave its mutant in the
    source, unseen by the anchor check, which now reports it;
  - two earlier mutants had survived: one since ADR-0146, whose loop was
    redundant and is removed, and one whose test this change made moot,
    rewritten.

Six mutants, and every mutant of six earlier scripts killed again
(`just e14-documents`).

**ADR-0160, 2026-10-03: a page's route**
([ADR-0160](DECISIONS/ADR-0160-a-page-s-route.md)). The compiler's half of the
audit's route gap. A route was read only to check links: nothing tied its
`{id}` to the page's parameter, and the plan did not carry it.
- The plan now carries the page's route.
- A route names each of its page's parameters once (PW0340), each is text
  (PW0621), and one route is one page's (PW0341).
- The canonical store declares `route "/stores/{id}"`.

The server still serves the store at `/StorePage.html`. Each document
becomes its own subscriber first: the session-wide model clears a session's
undelivered frames when another of its documents is served, so one of two
tabs can miss a change. Then the store is served at its route, beside a
second store. Nine mutants (`just e14-routes`).

**ADR-0159, 2026-10-03: a handler handles what its command answers**
([ADR-0159](DECISIONS/ADR-0159-a-handler-handles-what-its-command-answers.md)).
What ADR-0157 left open: a handler could read its command's answer, and
nothing made it.
- **The same dropped failure had been judged three ways.**
  - As a statement in a handler, it was refused.
  - As the handler's last value, `=> clear_cart()`, it compiled, and the
    press looked as though it worked.
  - Handed on by `?` or `return`, it checked, and only the backend refused
    it.
- **Now PW0618 refuses all three.** A handler matches the answer, or
  discards it by name, `let _ignored = ..`.
- **Correction: that discard could not end a handler.** The backend refused
  a block ending in a binding. It compiles now.
- **The canonical store shows every refusal.** Its Add handler's catch-all
  had hidden all but "sold out", and its Clear handler dropped its answer.
- **The benchmark's store discards each answer by name**, its behaviour
  unchanged, with T11's patches re-based. Every task's Pleris patches check
  as before.

Six mutants for the rule, and ADR-0099's seven re-anchored
(`just e14-handler-failures`).

**ADR-0158, 2026-10-03: a test leaves nothing behind**
([ADR-0158](DECISIONS/ADR-0158-a-test-leaves-nothing-behind.md)). The tests
had left 4,809 directories, 673 MB, in the temporary directory, about 500
more each `just ci`, on a disk that runs nearly full. A Rust test's
directory is removed when the test is done with it, and so is each harness
sandbox, however its step ends. A test run now leaves none.

**ADR-0157, 2026-10-03: a handler is answered what its command did, never
its value**
([ADR-0157](DECISIONS/ADR-0157-a-handler-is-answered-what-its-command-did.md)).
The audit's first gap, item availability, end to end (charter §15.4, §15.6
test 10).
- **The store refuses an item sold out since the page was rendered.**
  `add_to_cart` asks whether the item can be ordered before it writes, and
  refuses with `CartError.ItemUnavailable`. The page says "That item just
  sold out." in a live region, and the count goes back. In three engines.
- **The refusal reaches the handler.** A handler's call to a command is
  typed `Result<(), E>`: whether it committed, and its declared error if not.
  It never carries the value, which the page reads from the query the
  command invalidates, so the page has one source for what it shows. The
  server answers `Ok` without the cart, or the error whole.
- **PW0339:** a command is called only by a page's handler. Called from a
  declaration, its body ran without its own policies, so what it invalidates
  stayed stale.
- **PW0620:** a handler that binds a command's value is refused where it is
  written, in the handler's terms.

**Corrections found on the way:**
- **A compiled handler dropped a command whose answer decodes as `()`.** The
  decoder for `()` never read its argument, which was the call itself, so a
  command declaring no `Result` was never sent. `handlers.rs` caught it
  before it was committed.
- **A kept answer lost `null`.** A retry of such a command would have been
  answered nothing.
- **ADR-0154 named its rule PW0339.** It is PW0338 everywhere else.
- **E14-A's contract ran against the canonical store.** ADR-0156 says its
  parity is with the benchmark's store, and it runs against that store now
  (`baseline-store.sh`).
- **The value relations behind PW0605 typed a command's call by its
  declared result.** A surviving mutant found it: `describe(add(1))`
  checked, and only the backend refused it. All three typers agree now.

Tests in the compiler, the oracle, the host, the server and the browser, and
16 mutants (`just e14-command-answers`).

**ADR-0156, 2026-10-03: the benchmark's Pleris store is its own copy**
([ADR-0156](DECISIONS/ADR-0156-the-benchmark-s-pleris-store-is-its-own-copy.md)).
The tasks' Pleris patches were written against `examples/store`, which is
also the canonical store the audit leaves to grow. So it is frozen now as
`benchmarks/baselines/pleris`, as the other two stacks' baselines are. Every
task's sandbox, `pw diff`, the unsafe table and the keyed store build from
it. The canonical store grows toward §15 without re-basing 108 patches.

**Corrections, ADR-0155, 2026-10-03: the page keeps its subscription, and
recovers a refused handler**
([ADR-0155](DECISIONS/ADR-0155-the-page-keeps-its-subscription-and-recovers-a-refused-handler.md)).
An audit of the store against charter §15, requirement by requirement
(`docs/research/charter-15-store-audit.md`), found three runtime defects:
- **One failed request ended a page's subscription for good.** A dropped
  connection, a restart or a blink of the network left the page showing what
  it last heard, without a word. The request is asked again now, from the
  page's cursor, with a growing pause.
- **A press on a handler from another build did nothing** (§15.6 test 16).
  It reads the page again now, once, and never replays the press.
- **Every recovery was read one place off** from `pw-resume-wasm`'s codes.

Browser tests in three engines, and three mutants, each killed.

The audit also counts §15.6's tests: 18, not 17. At the audit, 7 were met,
3 met on T07's store, 7 partial and 1 missing. ADR-0155 meets test 16, and
ADR-0157 test 10, the missing one. The audit's remaining gaps, in order,
lead `docs/NEXT.md`.

**Keyed reads in three engines, 2026-10-03.** The charter's store tests 6-8
(§15.6) run in Chromium, Firefox and WebKit, on a store with T07's category
tabs built into `dist-keyed`:
- a changed key's old read is aborted, stopped on the server, and never
  shown;
- a key the page has is not asked for again;
- leaving the page stops its read on the server.

Found on the way: `run.sh` wrote its template IR into the spike's directory
whatever store it built. So a second store's build replaced the canonical
store's `store-ir.json`, which the server's tests compile in, and they failed
on a binding the canonical store does not have. Each IR's path can be given
now.

**ADR-0153 and ADR-0154, 2026-10-03: the two forms gate item 5 found open
are ruled.**
- **A template tests a case with `{#match}`** (ADR-0153, PW0337). An
  `{#if status == Placed}` chain was held to no case. One that forgot a case
  added later showed nothing for it, which is T04's wrong fix in another
  form. A template's catch-all arm was refused already (PW5019). A
  function's catch-all, outside the template, is what remains.
- **A command a page's handler calls declares `idempotent_by`** (ADR-0154,
  PW0338). Its request can be delivered twice whatever the program does,
  and RFC 9110 lets a client retry only what it knows to be idempotent. Each
  press carries an interaction. T08's setup, a command with none, no longer
  checks, and T08's class is refused whole.

**E14-C, 2026-10-03: T07 is written, all twelve tasks; ADR-0152, a key a
page changes** ([ADR-0152](DECISIONS/ADR-0152-a-key-a-page-changes.md)).
- **What was missing.** A page could not change a query's key. A signal
  given to a page's query was refused (PW5301), and `on_key_change`, which
  PW0325 requires, meant nothing at run time. ADR-0089 had left open whether
  `supersede` and `keep` mean anything. So a menu search or a category
  filter could not be written. The charter's store tests 6-8 (§15.6) could
  not be either: one key is one request, a changed key cancels or
  supersedes stale work, and leaving cancels unneeded requests.
- **Now a page's query given a signal is read again for the new key.**
  - The server applies only the latest read for a page that is still the
    session's, as one patch set in the session's frames.
  - `cancel` stops the old key's read: the browser aborts it, and the server
    lets go of its flight. `supersede` lets it finish, unshown. `keep` reads
    the new key once it has finished.
  - In none is an old key's answer shown for a new key. A key is a
    `String`, an `Int` or a `Bool` (PW5308), and its query declares its
    stale work (PW5309).
- **T07** lets a customer browse the menu by category, Hot slow to read.
  All four controls hold on all three stacks.
  - In the frameworks the setup lets Hot's late answer replace Cold's. The
    plausible wrong fix is React's documented ignore flag: Cold stays, and
    Hot's request runs on.
  - In Pleris that bug cannot be written. The task is the stale work: the
    setup's `keep`, the reference's `cancel`, the wrong fix's `supersede`.

  Every wrong fix builds, and only the hidden test of the stopped request
  fails it.

**Correction, found by T07: the browser ran presses in the order their code
arrived.** A handler's module loads on its first press. Hot then Cold, pressed
quickly, could run Cold's handler first, and leave Hot chosen. Each handler
now starts after the press before it. A browser test delays the first
press's code and checks the second press's effect is the one left.

**E14 gate item 5, 2026-10-03: which bug classes `pw check` refuses, stated
with evidence** ([E14.md](milestones/E14.md), `just e14-unsafe-table`).
For each task, the table reads where each stack's plausible wrong fix was
caught, from its recorded controls. It also runs `pw check` on Pleris's.
- **The count, at `f68f723`.** Pleris's checker refuses nine of the eleven
  wrong fixes. `tsc --noEmit` and `svelte-check` refuse none, and tests
  catch every one.
- **A refusal is narrower than "unrepresentable".** So the statement is made
  rule by rule: what each refuses, and what a program can still write around
  it.
- **Four classes are refused whole within the language**, a host binding's
  signature being the one thing trusted:
  - an optimistic update that cannot be rolled back (T01);
  - one session's data in a cache other sessions read (T03, T12);
  - a form field with no accessible name (T06);
  - a loading state with no failed state (T10).
- **Four are refused in part**, each with what it leaves open:
  - T04's `{#match}`: an `{#if status == ..}` chain that forgets a state
    still checks;
  - T05's streamed query: a slow query not declared streamed still holds a
    page;
  - T08's `retry`: a command with no `idempotent_by` still runs on each
    delivery;
  - T11's dialog: a close handler that leaves the state open is still
    accepted.
- **Two are refused on no stack:** T02's and T09's value kept past what the
  product allows.
- **One cannot be written in Pleris at all:** T02's framework wrong fix, a
  shared ask held forever.

**E14-C, 2026-10-03: T02 is written, eleven tasks of twelve.** All four
controls hold on all three stacks.
- **The task.** The store shows the kitchen's current prep time. It is never
  kept, and every page asks the slow kitchen. Pages opened while the kitchen
  is being asked must share that ask, and the next page after it answers
  must ask again.
- **The fix.** In Pleris it is one word of policy: `concurrency parallel`
  becomes `one_per_key`. In Next.js and SvelteKit it is a module-level map of
  asks under way, since neither framework shares work between requests.
- **The plausible wrong fix** in the frameworks keeps each ask in the map
  forever. Pleris cannot express that, because the runtime ends a flight
  however it ends. Its wrong fix keeps the answer for thirty seconds instead.

  Each stack's wrong fix builds, and only the hidden test of a changed prep
  time fails it.

**Correction, ADR-0151, 2026-10-03: the development server read each page
while it held its subscriber table**
([ADR-0151](DECISIONS/ADR-0151-a-page-s-values-are-read-outside-the-subscriber-table.md)).
Designing T02 found it.
- **What was wrong.** Every query a page reads ran inside the hold, so page
  loads ran one after another, server-wide. A slow query held up every
  other page and every command's frames. `concurrency one_per_key` never
  shared an ask between two pages, because two pages were never read at
  once.
- **Pages are read and rendered outside the table now.** The table is held
  only to install a page. A change that reaches the session meanwhile makes
  the page be read again, and the third attempt is read inside the table.
- **Eight pages read at once share one ask** and take about one ask's time,
  in the server's tests.
- **A command asked every query on the page for the cart's count, and then
  asked them all again.** It now reads the cart's binding alone.

**Correction to ADR-0149's controls: one mutant survived at `013aaff`.**
- The mutant skips `pw diff`'s own refusal of a program that does not
  check. `pw build` refuses the same programs, and the test asked only for
  the error's code.
- The test now asks for what only `pw diff`'s refusal says: the file each
  error is in.
- And T12, which starts from a store the compiler refuses, is also compared
  with the canonical store. Its reference changes nothing there: the fix
  restores the store's meaning exactly.

**E14-C and E14-D, 2026-10-03: T09 is written, ten tasks of twelve, and
`pw diff` reports what a change means.** These are the instruments of gate
items 2 and 1. Their evidence is recorded at the commit after this one.
- **T09** keeps the store's notice fresh. The notice board is expensive, and
  the setup keeps its answer five minutes. The task: a new notice shows
  within ten seconds, and the board is still asked at most once in ten
  seconds. All four controls hold on all three stacks.

  The plausible wrong fix stops keeping the notice. Every stack builds it,
  Pleris's included, since `freshness 0.seconds` is legal. Only the hidden
  test that counts the board's calls fails it: how long a value may be kept
  is a product decision, which no checker can know. `pw diff` reports
  Pleris's reference as one line, the old freshness beside the new.
- **`pw diff OLD NEW`**
  ([ADR-0149](DECISIONS/ADR-0149-pw-diff-what-a-change-means.md)) checks
  and builds two programs. It reports the change in charter §19.2's
  sections, each change on one line with its old value:
  - domain, effects, capabilities, privacy, placement;
  - cache and freshness, invalidation, pages;
  - client impact, server components, obligations, unsafe, diagnostics.

  A side that does not check is refused, naming its errors. For most of the
  benchmark's unsafe patches, that refusal is the review.

**Correction, found by `pw diff`'s first use: a page that streams a query
was charged the query's capability.** Diffing T05's reference reported that
the store page now needed `network.fetch`. The host runs a stream's query as
its own component, as it runs a handler. The page's contract now leaves it
out, as it leaves out a handler's body.

**Correction, found by T09: a Next.js cache's freshness is not its own.**
T09's first Next.js setup kept the notice with `unstable_cache`, and the
unchanged store passed the hidden tests. The cart's server actions call
`revalidatePath` for the store's page. In Next.js 16.3.8 that expires every
`unstable_cache` entry the page read, so each press of Add refreshed the
notice. The setup keeps the notice in a module-level map now, as SvelteKit's
does.

**Corrections, ADR-0150, 2026-10-03: one change reaches a page whole, and a
fragment shows its query's value**
([ADR-0150](DECISIONS/ADR-0150-one-change-reaches-a-page-whole.md)).
- After a command the server pushed one change's frames in two holds of its
  subscriber table, reading the cart again between them. A page could be
  sent one change in two batches. It could also get a value under a version
  the value is not from.
  - Eight more runs of the whole suite caught Chromium's "only cart-related
    part ids update" failing this way.
  - Firefox's "two fast presses are one fetch" failed once in three runs at
    `da0fcff`. It likely failed this way too, but that is not proven.

  One change's frames now go in one hold, and the value comes from the
  snapshot the patches came from.
- The store's menu fragment was kept until a command invalidated it. A menu
  changed at its source kept its old fragment past its freshness. It is
  rendered again now when the value it would show differs.
- The browser suite's evidence now records a failure's expected and received
  values.

The two stream mutants that survived at `da0fcff` have tests that kill them.
- "a declared error is kept": the test's assertion held under the mutant,
  so it now reads the region.
- "the response is not ended with its last region": the test timed the last
  data chunk, not the close.

**E14-C, 2026-10-03: T05 and T10 are written, nine tasks of twelve.** All
four controls hold on all three stacks for each.
- **T05** shows the store's recommendations, which take over a second,
  without holding up the menu. The plausible wrong fix waits for them before
  the page is sent. Next.js and SvelteKit build it, and only the hidden test
  that presses Add while they are pending fails it. Pleris refuses it at
  `pw check` (PW5400).
- **T10** gives the session's delivery estimate a loading and an error state.
  The plausible wrong fix forgets the error state:
  - in Next.js a failed estimate replaces the whole page with "This page
    couldn't load";
  - in SvelteKit the section goes blank.

  Both build. Pleris refuses the same patch at `pw check` (PW5401).

**Correction, found by T10: a Pleris build could not render a private
query's text part.** `pw-render --plan`, the static render inside the build,
had no value for one the values file did not give, so T10's setup did not
build. It now shows nothing of a session's value there, as it already
rendered a private list empty and a private block as nothing.

**Correction to ADR-0148's controls: four mutants were tested by no test that
ran.** The controls ran the server's tests filtered by `stream`, and three
of the five that tell a region's budget, failure and caching have no "stream"
in their names. They run every server test now. A fifth survived because the
second type pass's half was untested. A test now covers it.

**ADR-0148, 2026-10-03: a stream region shows its query's state, and its
settled arm comes in the same response**
([ADR-0148](DECISIONS/ADR-0148-a-stream-region-shows-its-query-s-state.md)).
A `<stream query={Q(..)}>` was refused at build since ADR-0075. It is
compiled now: T05 and T10 are expressible on Pleris.
- A page is sent before a query declared `delivery streamed` answers, with
  the region's placeholder in a `<?start>`/`<?end>` range. The server writes
  the arm the query settled to later in the same response, as the WHATWG's
  `<template for>` patch. Chrome 150 and later applies it itself, JavaScript
  off too; the runtime applies it in every other browser.
- The runtime now starts while a response is still open, from an inline
  `import()`. A deferred module, as it was loaded, starts only once the whole
  response has arrived, in every engine; measured by `just
  e14-stream-probes`.
- The failed arm is given `Option` of the declared error: `None` is the
  host's failure, a spent budget or a trap, which declares nothing.

What is refused:
- a `let` of a streamed query (PW5400);
- a stream missing an arm for a state its query reaches, or holding one for
  a state it cannot reach (PW5401);
- a streamed query with no `timeout` (PW5402).

Two findings on the way:
- The server's handler table walked three kinds of block by name, so a
  button inside a stream was refused.
- WebKit paints nothing until a page holds about 200 characters of text, or
  has loaded. A small streamed shell is blank in Safari until its regions
  settle, whatever renders it.

Demonstrated by `examples/demo/streamed.pw` in Chromium, Firefox and WebKit,
and natively in the host's Chrome 154.

**Corrections, ADR-0147, 2026-10-02: a page's query binding had no type,
and a failed query stopped the development server; T04 is written, seven
tasks of twelve.**
([ADR-0147](DECISIONS/ADR-0147-a-query-binding-is-the-query-s-value.md)).
Writing T04 found two defects:
- `let order = query Order(..)` had no type, so a `{#match}` over a query's
  value typed nothing below it, and its arms were refused. It is the query's
  `Ok` value now, as the server gives it, and a match over it must cover
  every case.
- A query that failed panicked the server while it held its table of
  subscribers, and every later request failed. Such a page is answered 503
  now, a served one is told to reload, and the server goes on.

T04 adds an order's new state, ready for pickup. All four controls hold on
all three stacks. The plausible wrong fix adds the state and forgets to show
it. Next.js and SvelteKit build it, in each framework's common idiom, and
show nothing for a ready order. Pleris refuses it at `pw check`: `{#match
status}` does not cover `Ready` (PW0305).

**E14-Q, fourth slice, ADR-0146, 2026-10-02: a block a query decides is
rendered and kept current**
([ADR-0146](DECISIONS/ADR-0146-a-block-a-query-decides-is-rendered-and-kept-current.md)).
`{#if cart.lines}` and `{#match order.status}` on the store page now render,
from each binding's whole value; a component's case reaches the renderer as
a case. After a command the server sends a block again where its rendering
changed, and not where it renders the same. What a loop's row reads of
another query is refused, since a row is rendered again for its own item.
T04 and T10 need this.

**E14-Q, third slice, ADR-0145, 2026-10-02: a page keeps every part and list
its queries decide current; T03 is written, six tasks of twelve**
([ADR-0145](DECISIONS/ADR-0145-a-page-keeps-what-its-queries-decide-current.md)).
Until now the dev server patched the store's cart count and its menu by
name. A page listing a private query's values did not render at all.

Now it renders every list a session's queries fill. It records what each
document shows, and after a command sends the difference:
- a text patch for each part that changed;
- keyed operations for each list: remove, insert after the one before,
  move, and set in place inside the item, so an item that stays keeps its
  nodes.

One change's patches travel as one `patch_set` frame. The page holds the
new version once all of them applied, and a set it cannot apply whole makes
it read the page again.

Found on the way: a block a query's value decides was left out of the page
plan. The store with `{#if cart.lines}` checked and built, and the server
failed at its first render. The plan refuses such a block now, naming it,
so `pw build` says so, until a host renders and patches one.

T03 lists the cart's lines from a private query of their own. All four
controls hold on all three stacks. The plausible wrong fix keeps the lines
between requests, keyed by the store alone. Next.js and SvelteKit build it,
and a second customer sees the first one's cart; Pleris refuses it at
`pw check` (PW5001).

**ADR-0144, 2026-10-02: a view holds its own signals, and a page provides
signals to the views it composes**
([ADR-0144](DECISIONS/ADR-0144-a-view-holds-signals-and-a-page-provides-them.md),
ADR-0130's third step). A header's cart button and the drawer it opens can
be two views now, sharing one signal with nothing passed by hand.
- `signal drawer: Bool` at module level is a name and a type.
- A page or view gives it a value with `provide drawer = false`, for
  everything it contains.
- A view names it as it names anything it imports.
- A view's own `signal count: Int = 0` is held once per use.
- One compiled handler serves every use, and the element says which
  instance it changes.
- An instance in a block starts again when the block shows another arm.

What is refused:
- a page whose views need a signal nothing provides (PW5305, the message
  naming the views it comes through);
- two `provide`s of one signal in a body (PW5306);
- a view holding a signal in a loop's row (PW5307).

Demonstrated by `examples/demo/provide.pw` in Chromium, Firefox and WebKit.

**Correction, ADR-0143, 2026-10-02: PW5014 accepted unlabelled fields and
refused labelled ones**
([ADR-0143](DECISIONS/ADR-0143-what-names-a-form-control.md)). The rule
took any `id` as a field's name, and never checked that a `<label for>`
pointed at it. It also never checked that an `aria-labelledby` reached
anything, or that an `aria-label` had text. So T06's unsafe store passed
`pw check` with a placeholder and no label. The render fixture `tricky.pw`
had shipped such a field since E7. The rule also refused a field wrapped in
its `<label>`, which the HTML standard defines as labelled. A field is now
named only by what reaches it in the same declaration, as the HTML standard
and accname 1.2 define:
- a `<label for>` with text;
- a wrapping `<label>` with text;
- an `aria-labelledby` reaching text;
- a non-blank `aria-label`.

Each refusal's note names the case and its repair.

**E14-C, 2026-10-02: T06 is written, five tasks of twelve.** A form asks
the store for an item. Its field has a visible label; an empty request
shows an error tied to the field and announced, and a request is thanked.
All four controls hold on all three stacks. The plausible wrong form uses a
placeholder for a label. Next.js and SvelteKit build it, and only the hidden
tests fail it. Pleris refuses it at `pw check` (PW5014), since ADR-0143.

**ADR-0142, 2026-10-02: an input bound to a signal**
([ADR-0142](DECISIONS/ADR-0142-an-input-bound-to-a-signal.md), ADR-0131's
fifth ruling). `bind:value={name}` shows a signal in a field and sets it to
what is typed. An attribute a signal decides is set in place, and a field is
never set from its own typing. Found on the way: a block a signal decides
was rendered again for every signal read anywhere in it. A field bound
inside one, a form in a dialog, would have been replaced at each key. A
block is rendered again only for what it cannot set in place now.
Demonstrated by `examples/demo/bind.pw` in Chromium, Firefox and WebKit.

**E14-C, 2026-10-02: T11 is written, four tasks of twelve.** Clear asks
first, in an accessible modal dialog; all four controls hold on all three
stacks. The plausible wrong fix forgets that Escape closes the dialog by
itself. Next.js and SvelteKit build it, and only a hidden test fails it.
Pleris refuses it at `pw check` (PW5303).

**ADR-0140 and ADR-0141, 2026-10-02: the store's page holds signals, and a
dialog a signal shows is the browser's modal dialog**
([ADR-0140](DECISIONS/ADR-0140-a-page-that-reads-queries-holds-signals-too.md),
[ADR-0141](DECISIONS/ADR-0141-a-dialog-a-signal-shows-is-the-browsers-modal-dialog.md)).
The store's route renders each signal's first value and ships the signals'
manifest. A `<dialog>` a signal's block renders is shown with
`showModal()`: it takes focus, makes the page behind it inert, and closes on
Escape. Its `close` handler keeps the signal true to what the page shows
(PW5303). Focus goes back to what invoked it, WebKit included, which neither
restores focus on `close()` nor focuses a pressed button. Demonstrated by
`examples/demo/dialog.pw` in Chromium, Firefox and WebKit. T11 is next.

**Correction, ADR-0139, 2026-10-02: a change made within two seconds of a
reload could be lost.** The stream adapter dropped each frame 25 ms after
writing it, though its own comment said it waited for the page's next
request. A page that had been reloaded, or a second tab, left its stream
held on the server. That stream wrote the new page's frames into a socket
nobody read, and dropped them. The keyed-list suite's intermittent failure
was this, failing 1 run in 10 at the commit before ADR-0138 and 4 in 10
after it. A frame is dropped now when a page says it applied it, and 20 runs
of 20 pass
([ADR-0139](DECISIONS/ADR-0139-a-frame-is-forgotten-when-the-page-says-it-applied-it.md)).

**ADR-0138, 2026-10-02: a handler is given its event**
([ADR-0138](DECISIONS/ADR-0138-a-handler-is-given-its-event.md), ADR-0131's
first slice). `on:input={(e: InputEvent) => typed = e.value}`: the browser
reads the event's record in the listener, before the handler's code loads,
and `on:submit|prevent` stops a form's submission there. Demonstrated by
`examples/demo/events.pw` in Chromium, Firefox and WebKit. Corrections found
building it, each of which checked or built clean:
- **every handler listened for a click**, whatever its event: `on:input` ran
  when the input was clicked and never when it was typed in. It was recorded
  in KNOWN_LIMITATIONS, and `pw build` accepted it;
- **a lambda's written parameter type was dropped**, in every lambda:
  `fn(y: Nada) 1` checked clean;
- **two handlers on one element**: a browser kept only the first of their
  two capture attributes, and the runtime bound only the first handler.

**Correction, ADR-0137, 2026-10-02: ADR-0133's limits claimed two
refusals the plan did not make.** A signal read inside an `{#each}` a query
decides, and a query's `{#each}` inside a block a signal decides, each built.
The first would have shown the signal's first value forever, and the browser
could not render the second again: it holds the signals alone. No served page
had either, since a page of signals reads no query. The plan now refuses
every part a signal decides that the browser does not render again, and every
part of a block it renders that reads what it does not hold. That covers
three more of the same kind, unlisted until now: a signal's block inside a
query's loop, a signal's list outside its block, and a handler in a signal's
block capturing the page's value
([ADR-0137](DECISIONS/ADR-0137-what-the-browser-renders-again-it-can.md)).

**ADR-0136, 2026-10-02: a view is written where it is used**
([ADR-0136](DECISIONS/ADR-0136-a-view-is-written-where-it-is-used.md),
ADR-0130's second step). `<MenuLine item={item} />` composes: the view's
markup is lowered in place, in the page's one numbering, its parameters read
as the paths its props give, and a name it binds renamed where it would hide
one of them. A view's handler keeps its own module, and the document carries
what it captures under the view's names for it. Checked where a view is used:
- props, as arguments (PW0619);
- what a view's handler captures, against the page's resume destination
  (PW5007, PW5018);
- a signal a view's handler would capture, refused (PW5301).
The page plan and the optimistic speculation read the composed page.
Demonstrated by `examples/demo/panel.pw` (byte-identical, now from views) and
`examples/demo/pick.pw` (one view used twice) in Chromium, Firefox and
WebKit.

**Correction, ADR-0135, 2026-10-02: in a program of several files, one
page's button could run another page's handler.** The build keyed each
handler's identity by indices counted within a file, so two pages of one
shape in two files collided, silently. Keyed by the file too
([ADR-0135](DECISIONS/ADR-0135-a-handlers-identity-is-its-own-files.md)).

**ADR-0133, 2026-10-02: a page holds its own UI state** (signals, first
slice; [ADR-0133](DECISIONS/ADR-0133-a-page-holds-its-own-ui-state.md), after
the rulings of [ADR-0130](DECISIONS/ADR-0130-ui-state-is-a-signal-provided-where-it-lives.md)
and [ADR-0131](DECISIONS/ADR-0131-a-handler-is-given-its-event-as-plain-data.md)).
`signal panel: Panel = Panel.Shut` in a page, changed by a handler that need
call no command, read in text and `{#if}`/`{#match}` blocks. The server
renders first values; the browser holds the signals and renders again exactly
what reads them, blocks by the server's renderer built for the browser
(`pw-render-wasm`, 90 KB gzipped, loaded on first use). Demonstrated by
`examples/demo/panel.pw` in WebKit and Chromium. Found on the way:
- **every handler but the store's two was refused in the browser**: the
  resume decision knew them by name
  ([ADR-0132](DECISIONS/ADR-0132-the-resume-decision-knows-what-the-build-compiled.md),
  fixed: it knows the build's handlers by identity);
- **a handler not written `resumable(..)` built, and its button was inert**
  ([ADR-0134](DECISIONS/ADR-0134-every-handler-is-resumable.md), fixed: every
  `on:` lambda is a handler, its captures inferred from what it reads; a
  handler that is not a lambda is refused at build).

**ADR-0129, 2026-10-02: a value's label follows it through calls, bodies,
branches and assignments** (E14-L,
[ADR-0129](DECISIONS/ADR-0129-a-values-label-follows-it.md)). Seven ways a
value's label was lost, each a probe that checked clean, are refused now: a
session id through a parameter that states its label, a secret returned by a
body or a helper, `if secret { "a" } else { "b" }` and a public log inside
such a branch, a mutable binding assigned a secret, a session id logged
publicly, and a session's value beside a secret in markup. No accepted,
store or kiokun program gains a diagnostic. ADR-0085's open ruling is
settled: labels are inferred over bodies, a secret a parameter states is a
key, and a label is lowered only at a host binding until a program needs an
audited `declassify`.

**Correction to ADR-0127, 2026-10-02: a concurrency defect shipped in the
second E14-Q slice.** `concurrent_commands_on_one_session_all_commit` failed
in about one CI run in three. The server kept query values in a side table
bounded to four per key, and a reader could be handed a token whose value
had already been dropped; and a reader invalidated eight times by concurrent
commits gave up. `pw-resource` now caches the value itself
(`Resources<V>`), and an invalidated read reads directly after one retry.
The test then passed 300 runs in a row.

**ADR-0128, 2026-10-02: a shared cache holds no one reader's value, whatever
its declaration says**
([ADR-0128](DECISIONS/ADR-0128-a-shared-cache-holds-no-one-readers-value.md)).
Charter §7.8's first must-fail example, `SharedCache<Cart@Session>`, passed
`pw check` in two shapes: a cart query declared `public` (or with no keyword)
and given the session as a parameter, which is T12's plausible wrong fix; and
any reader's value in a shared cache keyed by that reader, which the
generality matrix held as a *valid neighbour*. Both are refused now, and a
tenant's value keyed by its organization is the one partition a shared cache
may carry. **Generality is 31 / 31 with no known narrow invariant** (30 / 31
with 1 since C3): the ruling closed both executable known gaps. Found on the
way and written down as the next ruling: five ways a *value's* label is still
lost (a labelled parameter, a helper's body, a branch's condition, a public
log taking a session's value, and PW5003 reading only a label's first
restriction).

**E14-Q, second slice, 2026-10-02: queries run by their declared policies**
([ADR-0127](DECISIONS/ADR-0127-queries-run-by-their-declared-policies.md)).
`pw-resource` now applies each query's freshness, cache partition, key,
retries and timeout in the running server, and a commit drops exactly the
entries it invalidates. Found on the way, both fixed:
- **a page served after a menu change showed the old menu**: the fragment was
  regenerated without being invalidated, which `regenerate` treats as current;
  open pages were patched, new ones were not;
- **two presses at once rolled both back**: one commit invalidated the cart
  read the other was making, and the invalidated read failed the command.

**E14-Q, first slice, 2026-10-02: the page shows what its queries return**
([ADR-0125](DECISIONS/ADR-0125-a-page-shows-what-its-queries-return.md)). The
server ran none of the store's queries; `store.name` was a literal and the
count a Rust sum. The compiler now plans each page's values, compiles
`domain.line_count` as a component of its own, and the server runs the plan.
Query policies (freshness, cache, key, concurrency) are the second slice.

**E14-C started 2026-10-02: three of twelve tasks written**
([plan](milestones/E14.md)). T01, T08 and T12 hold all four controls on all
three stacks. T12's did not on Pleris until ADR-0128: moving the cart into a
`public` query kept it in a shared cache and checked clean. Nine tasks cannot be graded on Pleris today: six need the
server to run the store's queries (E14-Q), and three need language features
that wait on rulings (T05 streams, ADR-0075; T06 the event parameter,
ADR-0058; T11 view composition, ADR-0072).

**E14-B done 2026-10-02: the offline harness**
([ADR-0124](DECISIONS/ADR-0124-the-benchmark-harness-and-its-controls.md),
`just e14-harness`). One task, T08 (a press delivered twice adds once), on
three stacks, with all four controls holding on each. Pleris's plausible
wrong fix is refused by `pw check` (PW0312), whose repair names the right
one; Next.js's and SvelteKit's fail a contract test and both hidden tests.
Found building it:
- **the development server ran the repository's committed commands, not the
  program it served** ([ADR-0123](DECISIONS/ADR-0123-the-development-server-runs-what-pw-build-built.md)):
  components and contracts came from `docs/evidence/`, the graph was
  compiled in. It runs `pw build`'s output now. Query values are still the
  server's (E14-Q);
- the controls found a broken harness twice: every Next.js build failing
  (Turbopack and a symlinked `node_modules`), and a stage of zero tests read
  as a failure. Each control now requires tests that ran.

**Held 2026-10-02: an optimistic transition runs in the browser**
([ADR-0122](DECISIONS/ADR-0122-an-optimistic-transition-runs-in-the-browser.md)).
`add_to_cart`'s `optimistic` clause was checked and executed by nothing, its
transition `Carts.with_line` returned the cart unchanged, and the backend
could not compile the page's `cart.line_count`. The compiler now emits a
speculation module per page; the browser holds the value of the entry it
speculates on (`entry_value` frames); the count moves before the round trip,
reconciles to the server's version, and is restored exactly when a command
fails. Nine browser tests had used the count as evidence the server's patch
had arrived, and each now waits for the patch.

**Held 2026-10-02: an idempotent command runs once per interaction**
([ADR-0121](DECISIONS/ADR-0121-an-idempotent-command-runs-once-per-interaction.md)).
`idempotent_by InteractionId` was checked and read by nothing: a retried
request added twice. The contract now carries it, the runtime sends one
interaction per press, and the development server runs an idempotent command
once per interaction, refusing a request without one. What it keeps is
bounded per session.

**E14-A done 2026-10-02**
([ADR-0120](DECISIONS/ADR-0120-three-stores-one-contract.md),
[evidence](evidence/E14/contract.txt), `just e14-contract`): the store in
Next.js 16.3.8 and SvelteKit 2.70.3 beside the Pleris store, one behavioural
contract passing on all three, and five mutant stores each failing it. Found
on the way:
- **the Pleris store declares an optimistic update and per-interaction
  idempotency, and its runtime performs neither.** `pw check` checks both
  clauses; no backend executes them. The shared contract excludes both;
- the first SvelteKit store lost an add when two presses raced its session
  cookie. Both framework stores now issue the session on the first response,
  as Pleris does.

**E10 closed 2026-10-02.** All five gate items were re-recorded at `bff437c`
rather than closed on the 2026-09-25 evidence, since about 80 ADRs and a
Wasmtime upgrade had landed since:
[build](evidence/E10/build.txt), [oracle](evidence/E10/oracle.txt),
[load](evidence/E10/load.txt), [close-bench](evidence/E10/close-bench.txt),
[ownership](evidence/E10/ownership.txt). Re-recording found:
- **a defect, fixed in `0ad72c7`:** ADR-0115's authorization lookup refused
  every world-level export, so gate item 4's hand-written Rust baseline could
  not be called. Its benchmark is an ignored test, and CI did not see it;
- **an unstable instrument:** E7's gate 8 sees one 52-63 ms long animation
  frame, with no script attributed, in about half of runs on this machine, at
  HEAD and at `6545029`, the recorded tree, alike. Not a regression; the
  recorded zeros were single passing samples
  ([control](evidence/E10/gate8-control-2026-10-02.md)). Open as E7-G8.

Charter tasks that are not gate items are carried to E15 as deferred
obligations (EVIDENCE_LEDGER): task 2's Wasm for compute-heavy browser modules
(E10-T2), step 10's compiled data layer (E10-S10), and a memory strategy
beyond invocation regions (E10-M).

Task 7's affine annotations are the effect rows `resource.acquire<T>` and
`resource.release<T>`, and since ADR-0045 the invariant they express is
checked on every path.

 **E10-I closed 2026-09-24**
([evidence](evidence/E10/e10-i-2026-09-24.md), [ADR-0032](DECISIONS/ADR-0032-compiled-components.md)):
`add_to_cart` compiles to a Wasm component, runs through the E8 host with the
host's own operations, and the dev server's Rust closure path is deleted.
**Resumable handler bodies compile to JavaScript modules since 2026-09-25**
([evidence](evidence/E10/handlers-2026-09-25.md), [ADR-0033](DECISIONS/ADR-0033-compiled-handlers.md)),
so the store's behaviour is compiled from `.pw`: the page's handlers and the
commands they reach. E9 closed on 2026-09-24 (E9-V1..V6,
[evidence](evidence/E9/value-relations-2026-09-24.md),
[ADR-0031](DECISIONS/ADR-0031-value-relations.md)). Next: the rest of E10, and
the kiokun proof slice. See [NEXT](NEXT.md).

**Correction, 2026-09-25:** GitHub CI failed on the pushed E10-I merge
`26feb6b`, although `just ci` had passed locally. The dev server's `engine`
feature turned on E8's guest tests in every workspace build, and a clean
checkout has no guests. Fixed in `83af93c`, and CI is green on it. Recorded in
[the E10-I evidence](evidence/E10/e10-i-2026-09-24.md).

**Correction, 2026-09-25: ADR-0011's exhaustiveness rule was not enforced for
most matches, and the analysis proved some incomplete ones exhaustive**
([ADR-0038](DECISIONS/ADR-0038-every-match-typed.md)). ADR-0011 requires `pw` to
reject an incomplete match whatever the effect row.
- The checker analysed only a scrutinee that was a parameter annotated with a
  sum type the program declares. A match over a call, a field, a local, or any
  `Option` or `Result` was never analysed, so `match get(w) { Some(x) => .. }`
  passed `pw check`. The kiokun slice found it: the backend refused such a
  match while compiling `Lookup`.
- Fixing it found four ways the analysis reported an incomplete match as
  exhaustive, all present at `1e2d0de`:
  - nested patterns were read against the scrutinee's type, so
    `Circle(Draft)` covered `Circle(Sent)`;
  - a constructor its type lacks read as a wildcard;
  - a shadowed parameter was read as the parameter;
  - `=> return Ok(())` parsed as an arm ending at `return` plus a phantom arm
    whose pattern was `Ok(())`, with no error, and the phantom arm read as a
    wildcard.
- All five are fixed, with tests in
  `compiler/pw-core/tests/match_exhaustiveness.rs`. A constructor its type
  lacks is the new PW0608. What the analysis still cannot read (a literal, a
  pattern nested under `Some`) blocks it, with the reason, and is never proven
  (KNOWN_LIMITATIONS).
- Mutation controls: 14 of 14 killed. Each of the 14 pieces of the fix,
  undone in turn, fails the tests ([match.txt](evidence/E10/match.txt),
  `just e10-match`). The E9
  evidence and the E10 oracle were re-recorded at the same commit, `f08672b`:
  E9's counts and its 10 of 10 mutants are unchanged, and the oracle's missing
  case is now the checker's refusal.

**Correction, 2026-09-25: "kiokun's ranking" is the slice's port**
(ADR-0041, corrected). Every agreement ADR-0041 reports is with
`data::Index::search`, ADR-0037's Rust port. kiokun.com's current
`/api/search` takes hangul and CJK punctuation as CJK and astral Han as Latin,
and searches script variants (地図 ⇄ 地圖); the port does neither.

**Correction, 2026-09-25: PW2005 enforced less than its invariant**
([ADR-0045](DECISIONS/ADR-0045-affine-exactly-once.md)). "An affine value
must be consumed exactly once" was checked as "released before each
`return`". A transaction never ended, one live across a failing `?`, and one
ended twice all passed `pw check`, and a declaration promising to end a
transaction parameter was never held to it. Every path is counted now.

**Correction, 2026-10-02: a public query made one reader's value, and every
rule took it for a public one**
([ADR-0118](DECISIONS/ADR-0118-what-a-declaration-reads-it-reads-through-what-it-calls.md)).
The privacy rules read a declaration one call deep. A query declared
`public` that read the session itself, through a helper, or through another
query passed each of these at `671931b`:
- a `partition public` fragment depending on it (PW5101), so the first
  reader's cart was served to every reader;
- a `cache shared` query reading the session through a helper (PW5004);
- a `cache shared` page reading it (PW5001);
- that page's contract, which allowed `build`.

A declaration's label is joined with what it reads through what it calls, to
a fixed point, and the contract reads the same labels. A command's reads stay
its own (ADR-0113). No fixture's diagnostics change.

Evidence: [reads-through-calls.txt](evidence/E10/reads-through-calls.txt)
(`just e10-reads-through-calls`).

**Correction, 2026-09-26: a built page read a request's value**
([ADR-0114](DECISIONS/ADR-0114-what-is-built-before-any-request-reads-no-requests-value.md)).
A `placement build` page rendering its `id` passed `pw check`. Its file is
built before any request supplies `id`, and served to every reader. A
build-placed declaration reads none of its parameters now (PW5026).

Evidence: [built-pages.txt](evidence/E10/built-pages.txt)
(`just e10-built-pages`).

**Correction, 2026-09-26: a handler wrote the database from the browser**
([ADR-0113](DECISIONS/ADR-0113-a-resumable-handler-runs-in-the-browser.md)).
The store's Add handler written `Carts.add(current_session(), item.id,
PositiveInt(1))`, rather than as a call to `add_to_cart`, passed `pw check`,
and `pw emit-handlers` refused it. The page's contract leaves its handlers
out, since a command has its own, and nothing asked where a handler's own
effects run. They are the browser's to grant now (PW5005). R-010 and a
generality witness each had this second defect, and each is corrected.

Evidence: [handlers-in-the-browser.txt](evidence/E10/handlers-in-the-browser.txt)
(`just e10-handlers-in-the-browser`).

**Correction, 2026-09-26: a secret reached the browser and a public log,
by the spelling of an import**
([ADR-0112](DECISIONS/ADR-0112-a-calls-privacy-is-the-declaration-it-resolves-to.md)).
The privacy rules read a callee by its fully qualified spelling, and a bare
imported name has none. Each of these passed `pw check`:
- R-003 with `import secrets.{ payments }`: a payments key rendered into
  markup;
- R-006 with `import log.{ public }`: a payments token in a public log;
- a public query in a shared cache reading `current_session()`, with every
  session's cart in one entry.

Written qualified, each was refused. A callee is resolved as the unit sees
it now. A page PW5001 refuses a shared cache is not reported again by PW5004,
which the qualified spelling had done.

Evidence: [privacy-by-resolution.txt](evidence/E10/privacy-by-resolution.txt)
(`just e10-privacy-by-resolution`).

**Correction, 2026-09-26: a correct handler was refused**
([ADR-0111](DECISIONS/ADR-0111-a-shorthand-field-reads-its-capture.md)). A
handler capturing `item` and building `Pick { n: 1, item }` was refused by
PW5017. The capture paths, the handler artifact and the backend each saw a
capture only through a name or a field path, and a shorthand field has
neither, so the artifact read nothing of `item` and the backend could not
build the record. It reads its capture whole in all three now, and the
handler compiles.

Evidence: [capture-shorthand.txt](evidence/E10/capture-shorthand.txt)
(`just e10-capture-shorthand`).

**Correction, 2026-09-26: `pw check` passed a handler its build refuses**
([ADR-0110](DECISIONS/ADR-0110-a-resumable-handler-reads-what-it-captures.md)).
A resumable handler runs later, in the browser, with what it captured.
`resumable() => add_to_cart(item.id, ..)` inside an `{#each}` read `item` and
captured nothing, and checked; `pw emit-handlers` refused it, "`item` is not
bound here". A handler reads what it captures and what it binds itself now
(PW5025), a shorthand field `Pick { n: 1, item }` included.

Evidence: [handler-captures.txt](evidence/E10/handler-captures.txt)
(`just e10-handler-captures`).

**2026-09-26: a timeout is a budget above zero**
([ADR-0109](DECISIONS/ADR-0109-a-timeout-is-a-budget-above-zero.md)).
`timeout 0.seconds` checked, and the resource runtime expires a flight once
its budget is spent, so the query could never answer. A timeout is a duration
above zero now (PW0335).

Evidence: [timeouts.txt](evidence/E10/timeouts.txt) (`just e10-timeouts`).

**2026-09-26: a query names a resource that exists**
([ADR-0108](DECISIONS/ADR-0108-a-query-names-a-resource-that-exists.md)).
`let menu = query Nonexistent(id)` in a page checked. The name check bound a
keyword statement's first word as if the statement declared it, and PW5100
reports a dangling edge only for a graph clause, which a page has none of.
The resource a `query` or `subscription` names is resolved now (PW0021).

Evidence: [queries-name-resources.txt](evidence/E10/queries-name-resources.txt)
(`just e10-queries-name-resources`).

**2026-09-26: a cache key names each parameter its entry depends on**
([ADR-0107](DECISIONS/ADR-0107-a-cache-key-names-what-its-entry-depends-on.md)).
`query Other(id, other)` with `key id` and a body reading `other` checked, so
`Other(1, 2)` and `Other(1, 3)` shared one cache entry and the second call was
given the first's store. PW5004 held a key to the privacy partitions a value
depends on, and nothing held it to the parameters. A `key` or `dedupe_by`
names each parameter the body reads now (PW0336).

Evidence: [keys-cover-reads.txt](evidence/E10/keys-cover-reads.txt)
(`just e10-keys-cover-reads`).

**Correction, 2026-09-26: `pw check` passed a file whose name another
file had**
([ADR-0106](DECISIONS/ADR-0106-each-file-is-reported-by-its-place.md)).
`pw check` kept each file's diagnostics under its file name, and of two files
of one name the second's replaced the first's: `pw check a/app.pw b/app.pw`
printed "no diagnostics" and exited 0 with a type error in `a/app.pw`. Every
check `just ci` runs names two files `effects.pw`, the standard library's and
the platform's, so the standard library's was never reported on; checked
under a name of its own, it has no diagnostics. Each file's diagnostics are
its own now, by its place, and a shared name is shown as a path.

Evidence: [same-name.txt](evidence/E10/same-name.txt) (`just e10-same-name`).

**2026-09-26: a command invalidates the entry it speculates on**
([ADR-0105](DECISIONS/ADR-0105-a-command-invalidates-the-entry-it-speculates-on.md)).
A command speculating on `Cart` with neither `invalidates` nor an event
`Cart` listens for checked. ADR-0025 has the command's success reconcile the
speculation, and both backends do that through the entry, so nothing
replaced it. A command now reaches each entry it speculates on (PW5107).

Evidence: [speculation-reconciled.txt](evidence/E10/speculation-reconciled.txt)
(`just e10-speculation-reconciled`).

**Correction, 2026-09-26: the dev server committed an event no command
declared**
([ADR-0104](DECISIONS/ADR-0104-the-dev-server-commits-what-a-command-declares.md)).
After any cart write it committed `CartChanged`, whatever the command's
`emits` said, so a command declaring no event updated the page here and would
nowhere else. It commits the events a command declares now, read from the
compiler's graph, and refuses a key it cannot compute before the command
runs. The store's commands declare `CartChanged`, and its browser suite
passes as before.

Evidence: [committed-events.txt](evidence/E10/committed-events.txt)
(`just e10-committed-events`).

**2026-09-26: a write reaches the fragments built on it**
([ADR-0103](DECISIONS/ADR-0103-a-write-reaches-the-fragments-built-on-it.md)).
A fragment is rebuilt only when an event reaches it. One built from a query
with `freshness 5.minutes`, or reading the store in its own body, kept a
renamed store for good: the command emitted nothing, and PW5106 left the
query to expire. A command now emits an
event that reaches each fragment built on what it writes (PW5106).
`invalidates` does not reach a fragment, because the materializer reads
events.

Evidence: [fragments-reached.txt](evidence/E10/fragments-reached.txt)
(`just e10-fragments-reached`).

**Correction, 2026-09-26: the materializer stopped at an event's
listeners**
([ADR-0102](DECISIONS/ADR-0102-an-event-reaches-what-reads-what-it-invalidates.md)).
The compiler's graph says an event reaches its listeners and whatever reads
them, and charter §9.4 makes a fragment a view over what it reads. The
materializer invalidated the direct listeners only. A-009's `MenuFragment`
depends on a `Store` that listens for `StoreChanged`, and kept the old store
after one. The store demo never showed this, because its fragment listens for
every event itself. An event now reaches each entry that reads what it
reaches, at the key the read supplies.

Evidence: [read-through.txt](evidence/E10/read-through.txt)
(`just e10-read-through`).

**Correction, 2026-09-26: a command's write reached no reader, in the
accepted corpus too**
([ADR-0101](DECISIONS/ADR-0101-a-command-invalidates-what-it-writes.md)). A
command that writes the cart and declares neither `invalidates` nor `emits`
checked, and nothing told the queries reading the cart that it had changed.
- The accepted corpus is one program. In it, A-005's `add_to_cart`
  invalidated the library's stub `Cart`. A-004's `Cart` has zero staleness
  and read-your-writes, and was told nothing.
- Five generality witnesses cleared the cart the same way.
- The dev server hid it: it emits `CartChanged` after any cart write,
  whatever the command declares (KNOWN_LIMITATIONS).

A command now reaches each reader of what it writes that has no staleness
window, by name or by an event the reader listens for (PW5106). A-004 listens
for `CartChanged`; A-005 and the witnesses emit it.

Evidence: [writes-invalidated.txt](evidence/E10/writes-invalidated.txt)
(`just e10-writes-invalidated`).

**2026-09-26: a query reads**
([ADR-0100](DECISIONS/ADR-0100-a-query-reads.md)). A `session query` whose
body was `Carts.clear(current_session())` checked. The platform caches,
deduplicates and retries a query as a read, so the write happened once per
cache entry, and again on every retry, with no idempotency key and nothing
invalidated. A query or a subscription that writes or opens a transaction is
refused now (PW0401).

Evidence: [query-reads.txt](evidence/E10/query-reads.txt)
(`just e10-query-reads`).

**Correction, 2026-09-26: failures were dropped, in a fixture that claimed
to be clean**
([ADR-0099](DECISIONS/ADR-0099-a-failure-is-handled.md)). A statement's
`Result` was dropped without a word. Nine fixtures dropped one, and
`rules/affine/transaction-ended-on-every-path.pw` is `@expect: clean` while
losing a failed rollback before `return Ok(())` and committing after a
failed clear. A `Result` nothing uses is refused now (PW0618), and the
fixtures discard theirs by a name that says so.

Evidence: [results-handled.txt](evidence/E10/results-handled.txt)
(`just e10-results-handled`).

**2026-10-01: the Node advisory gate is clean of the newly published
`brace-expansion` findings** ([ADR-0117](DECISIONS/ADR-0117-patched-brace-expansion-stays-below-markos-glob-layer.md)).
Marko's `glob 13.0.6 -> minimatch 10.2.6` path had resolved
`brace-expansion 5.0.9`; pnpm now resolves the patched 5.0.12 through a
range-scoped override. No advisory was allow-listed.

**2026-10-01: Wasmtime 47.x is no longer a live execution pin**
([ADR-0116](DECISIONS/ADR-0116-wasmtime-48-0-3-replaces-the-vulnerable-47-x-pin.md)).
The supply-chain gate found RUSTSEC-2026-0315 and RUSTSEC-2026-0316 in 47.0.4.
The workspace engine, conformance engine, standalone host spike and bootstrapped
CLI now pin Wasmtime 48.0.3. Both Cargo lockfiles were resolved by Cargo, not
edited by hand. Historical evidence remains labelled with the engine version
that produced it.

**Closed 2026-10-01: authorization is held before a command runs**
([ADR-0115](DECISIONS/ADR-0115-a-commands-requires-is-held-before-its-body-runs.md)).
`requires SignedIn` is an invocation precondition on the compiled command
export now. Predicate names are deployment authorization vocabulary; their
arguments are command parameters carried by index. `pw-host` refuses an
export whose requirements were not evaluated, and an unknown, denied, failed
or malformed requirement cannot reach the component body. The development
store explicitly models every local demo session as `SignedIn`; it remains a
development identity model, not production authentication.

Evidence boundary: [component-contracts.json](evidence/E8/component-contracts.json)
records `SignedIn` on both store commands; compiler, host-mirror, component
execution and development-server tests cover the source-to-invocation path.

**2026-09-26: a name is written once where it is declared**
([ADR-0098](DECISIONS/ADR-0098-a-name-is-written-once-where-it-is-declared.md)).
Each of these checked, and one of its two writings was dropped:
- `fn f(x: Int, x: String)`;
- a field or a case declared twice;
- `cache private` then `cache shared`, where every reader takes the first,
  so the order decided whether a session's data was refused a shared cache;
- `<a href="/a" href="/b">`;
- `P.Pair(a, a)`, binding `a` twice in one pattern.

Each is written once now (PW0028). An effect's `impact` is the one head
that repeats.

Evidence: [declared-once.txt](evidence/E10/declared-once.txt)
(`just e10-declared-once`).

**2026-09-26: data embedded in a page cannot end its script element**
([ADR-0097](DECISIONS/ADR-0097-embedded-data-cannot-end-its-script.md)). The
parts manifest's JSON, in `<script type="application/json">`, broke only a
lowercase `</script`. `</SCRIPT>` would end the element and `<!--<script>`
swallow the page. The manifest holds what the compiler wrote, so no page
carried either. The embedded text holds no `<` now.

Evidence: [embedded-json.txt](evidence/E10/embedded-json.txt)
(`just e10-embedded-json`).

**Correction, 2026-09-26: a view could choose where the platform's runtime
loads from**
([ADR-0096](DECISIONS/ADR-0096-a-template-moves-no-url.md)). The page a view
renders into writes the view's markup, then `<script type="module"
src="/pw-runtime.mjs">`. So `<base href={msg}>` in a view moved the
runtime, and every relative link after it, to wherever `msg` named. `<animate
attributeName="href" values={msg}>` set a link's `href` to any URL, past the
URL check. Both checked and built. A template writes no `<base>` and
animates no link or handler now (PW5024).

Evidence: [moved-urls.txt](evidence/E10/moved-urls.txt)
(`just e10-moved-urls`).

**Correction, 2026-09-26: `HREF={msg}` escaped no scheme**
([ADR-0095](DECISIONS/ADR-0095-an-attributes-context-is-read-as-html-reads-its-name.md)).
The template IR chose a value's escaping from the attribute's name as
written, and HTML lowercases it. So `<a HREF={msg}>`, `<img Src={msg}>` and
`<p STYLE={msg}>` were escaped as ordinary attributes, and `msg =
"javascript:alert(1)"` ran. `HREF="javascript:go()"` also escaped ADR-0094's
rule. The name is read as HTML reads it now.

Evidence: [attribute-case.txt](evidence/E10/attribute-case.txt)
(`just e10-attribute-case`).

**Correction, 2026-09-26: a value a program writes could run as a script**
([ADR-0094](DECISIONS/ADR-0094-a-template-writes-no-code.md)). The renderer
escapes a value as text, an attribute, a URL or a style, and four places
are none of those. Each of these checked and built:
- `<script>{msg}</script>`, a text part, runs `msg` as the page loads;
- `<button onclick={msg}>`, an attribute part, runs it when pressed;
- `<iframe srcdoc={msg}>` decodes back into a document that runs on this
  origin;
- `<style>{msg}</style>` is a stylesheet a value writes into, although the
  IR documents a `style` element as the Style context;
- `<a href="javascript:go({id})">` runs `id` too, since a browser
  percent-decodes the URL first.

A template writes no script, no inline handler, no script URL, no `srcdoc`
and no value into a stylesheet now (PW5023). Nothing in the corpus wrote any
of them.

Evidence: [code-in-markup.txt](evidence/E10/code-in-markup.txt)
(`just e10-code-in-markup`).

**2026-09-26: an element handles an event the platform declares**
([ADR-0093](DECISIONS/ADR-0093-an-element-handles-an-event-the-platform-declares.md)).
`<button on:clik={go}>` checked. The platform declares its events in
`events`, and the one rule that reads them skipped a name it did not find.
The runtime listens for a click whatever the name, so the handler ran by
accident. An `on:` attribute names a declared event now (PW5022).

Evidence: [declared-events.txt](evidence/E10/declared-events.txt)
(`just e10-declared-events`).

**Correction, 2026-09-26: ADR-0089 refused `idempotent_by Int`.** A type a
policy names was looked up among declarations alone, and `Int` is not one.
It is resolved as a written type is now (ADR-0089's correction).

Evidence: [policy-values.txt](evidence/E10/policy-values.txt), re-recorded.

**2026-09-26: a dependency-graph clause belongs to a declaration that can
mean it**
([ADR-0092](DECISIONS/ADR-0092-a-graph-clause-belongs-to-a-declaration-that-can-mean-it.md)).
A query that `emits` or `invalidates`, a command that `invalidates_on` or
`depends_on`, a `fn` that emits and a page that invalidates each checked.
The graph drew edges nothing reads, or dropped the clause. Each clause
belongs to the declarations ADR-0007 gives it now (PW5105).

Evidence: [clause-places.txt](evidence/E10/clause-places.txt)
(`just e10-clause-places`).

**Correction, 2026-09-26: an inventory change never invalidated its
store's menu**
([ADR-0091](DECISIONS/ADR-0091-a-listener-binds-its-entrys-key.md)). The
materializer asked whether each of an event's values was somewhere in an
entry's key. `InventoryChanged(47, item 3)` was deferred forever, since no
menu is keyed by an item, so the store's `MenuFragment` and A-009 stayed
fresh when their inventory changed. A value at one position also matched a
key at another. Nothing checked what a listener wrote either. The store,
A-009 and two witnesses listened with `item` or `_item: MenuItemId`, which
name nothing, and one witness gave a store's event a consumer. A
listener's argument is a parameter the entry's key binds, or `_` (PW5104),
related to the event's values. The materializer compares them position by
position.

Evidence: [listeners.txt](evidence/E10/listeners.txt)
(`just e10-listeners`).

**Correction, 2026-09-26: `pw build` compiled programs `pw check` refuses**
([ADR-0090](DECISIONS/ADR-0090-the-build-checks-what-pw-check-checks.md)).
The declaration rules (PW0312, PW0313, PW0102, PW0325, ..) ran only in the
`pw check` command, beside `check_sources`. `pw build` checks through
`check_units`, which did not run them, so it compiled R-015's `retry
forever` query into a component and built R-014, a non-idempotent command
that retries. One checker runs them now. Their diagnostics had never met the
checker's standard either. Two restated their invariants in words the
registry does not use, seven lacked a boundary span or an explanation, and
PW0312 did not name the policy R-014 expects. Each is repaired.

Evidence: [one-checker.txt](evidence/E10/one-checker.txt)
(`just e10-one-checker`).

**Correction, 2026-09-26: no policy's value was checked, and three privacy
and retry rules could be bypassed by spelling**
([ADR-0089](DECISIONS/ADR-0089-a-policy-value-is-one-its-domain-has.md)).
The policy table says what each head's value is, and nothing held a value
to it:
- `cache Shared` on R-004's page, a session's cart in a shared cache,
  checked. The privacy rule read it as no shared cache.
- `placement originn` checked, and the declared world was dropped for a
  derived one.
- `retry nope(..)`, `retry transport_only(maxx = 2)` and `key nope`
  checked.
- The store's own commands named `InteractionId` without importing it.

Three rules matched a value loosely:
- `freshness 05.seconds` on a session read began with `0`;
- a cache key's `username` contained `user`;
- `transport_onlyish(..)` began with `transport_only`.

A value is one its domain has now (PW0335), and each reader reads it
exactly. The table gains what the corpus writes and the manifest reads:
`parallel`, `fixed`, `supersede`, `keep` and `application`.

Evidence: [policy-values.txt](evidence/E10/policy-values.txt)
(`just e10-policy-values`).

**Correction, 2026-09-26: a clause's key was never checked, and the
store's own was ill-typed**
([ADR-0088](DECISIONS/ADR-0088-a-clause-names-a-declaration-and-gives-it-its-key.md)).
`depends_on`, `invalidates` and `emits` name a declaration and pass it
values, and the values were text. The graph drew an edge to whatever a name
found, and nothing resolved, counted or typed a key. `invalidates
Cart(nosuch)`, `invalidates Cart(item)` (a menu item where `Cart` is keyed by
a session), `invalidates CartChanged(..)` (an event) and `emits Cart(..)` (a
resource) all checked. Nine keys in six files had the wrong type:
- the store's `emits CartChanged(current_session())` gave a
  `Session<SessionId>` to an event declared with a `SessionId`;
- R-045 and its rule twin passed a store id to `Cart`;
- three witnesses keyed a fragment by a `SessionId` and passed it to `Cart`.

Each is corrected, and the event carries the session now. A clause names a
declaration of its kind (PW5103), and its key's arguments are terms,
resolved and related to what it names (PW0021, PW0604, PW0605, PW0617).
Found on the way: an optimistic clause written with extra spaces put every
diagnostic inside it on the wrong columns. `invalidates_on` is next: what
its arguments bind, and the materializer's matching, which never
invalidates store 47's menu when its inventory changes (NEXT).

Evidence: [clause-keys.txt](evidence/E10/clause-keys.txt)
(`just e10-clause-keys`).

**2026-09-26: a call names a term**
([ADR-0087](DECISIONS/ADR-0087-a-call-names-a-term.md)). A call to a view,
a page, an event or an effect checked: `let e = CartChanged(s, 3)`,
`secret(1)`, and `fn f() -> Int { Badge(1) }`, a view returned where an
`Int` is declared. The rule for a bare call accepted a name that resolved in
any namespace, and every analysis answered for the call as for a call to
nothing. A call names a function, a data operation, or a type it builds now
(PW0027).

Evidence: [call-names.txt](evidence/E10/call-names.txt)
(`just e10-call-names`).

**2026-09-26: a function crosses no boundary**
([ADR-0086](DECISIONS/ADR-0086-a-function-crosses-no-boundary.md)). A
resumable handler that captured a function, a view's `fn(Int) -> ()`
parameter or a local declared one, checked, and the build refused the
handler for a name that "names no declaration". A value that is or holds a
function crosses no boundary now (PW5008).

Evidence: [function-captures.txt](evidence/E10/function-captures.txt)
(`just e10-function-captures`).

**Correction, 2026-09-26: a secret laundered through any declared
function over plain values**
([ADR-0085](DECISIONS/ADR-0085-a-call-carries-what-it-is-given.md)).
ADR-0064 carried a label through a declared call only where the result
mentions a type parameter. So `log.public(String.trim("with
{secrets.payments()}"))` passed, and so did `String.to_upper`, `slice`,
`join` and a program's own `fn echoed(text: String) -> String`. A declared
call carries every argument given to a parameter that states no label now,
as the charter's data flow asks (§7.8). A parameter declared
`key: Secret<Payments>` keeps its contract, so `Payments.capture`'s receipt
is not the key. This reverses ADR-0064's choice that the length of a list
of secrets is public, and closes the recorded limitation that an element a
secret index chose was public. No example changed.

Evidence: [labels-through-plain-values.txt](evidence/E10/labels-through-plain-values.txt)
(`just e10-labels-through-plain-values`).

**2026-09-26: what a string interpolates has a text form**
([ADR-0084](DECISIONS/ADR-0084-what-a-string-interpolates-has-a-text-form.md)).
`"{xs}"` over a list, a record or an `Option` checked, and the backend was
the first to refuse it. A string's holes are related as a template's are
(ADR-0074) now, PW0609 and PW0600. Three fixtures interpolated a value with
none, and are corrected:
- two clean ones logged a `Result`, the rule fixture
  `privacy-sink/public-value-to-public-log.pw` and the generality neighbour
  `value_exceeds_sink_level/valid-public-neighbour.pw`;
- the witness `private_in_shared_cache/branch-join.pw` showed a whole
  `Cart` and `Store`.

Evidence: [string-holes.txt](evidence/E10/string-holes.txt)
(`just e10-string-holes`).

**2026-09-26: a call's arguments open on the callee's line**
([ADR-0083](DECISIONS/ADR-0083-a-call-opens-on-its-callees-line.md)). A `(`
at the start of a line continued the expression before it: `g(n)` then `()`
parsed as `g(n)()`, and a correct function was refused for "`` does not
resolve". A `(` at the start of a line begins a statement now, as `<`, `-`
and `!` did already.

Evidence: [call-lines.txt](evidence/E10/call-lines.txt) (`just e10-call-lines`).

**2026-09-26: a `derived` value performs no effect**
([ADR-0082](DECISIONS/ADR-0082-a-derived-value-performs-no-effect.md)). The
charter's `derived` is "a pure value computed from other values", and
ADR-0047 recorded that nothing checked it: `let t = derived clock.now()`
passed in a declaration whose row allows the clock. PW0334 refuses an effect
inside a derived value, whether it is called, read through a member, or
named as a value.

Evidence: [derived.txt](evidence/E10/derived.txt) (`just e10-derived`).

**Correction, 2026-09-26: a call with named arguments compiled them in
written order** ([ADR-0081](DECISIONS/ADR-0081-a-named-argument-is-its-parameters.md)).
- **A silent miscompile.** `g(b = 1, a = n)`, with `fn g(a: Int, b: Int)`,
  compiled as `g(1, n)`: `Named(10)` answered `-9` through the E8 host,
  where it means `9`. A signature carried no parameter names, so nothing
  could place a named argument.
- **Named arguments were checked by nothing.** A wrong type, a name the
  callee lacks, a parameter given twice and a positional argument after a
  named one each passed.
- **A generic result's label was carried from the wrong argument**, so a
  secret given by name to a generic helper came out public.

Signatures carry parameter names now, one arrangement serves the checker,
the labels and the backend, and PW0617 refuses one that fails. No example
calls a declaration with named arguments.

Evidence: [named-arguments.txt](evidence/E10/named-arguments.txt)
(`just e10-named-arguments`).

**Correction, 2026-09-26: PW2005 found a transaction by its name, so a
transaction never ended passed**
([ADR-0080](DECISIONS/ADR-0080-the-affine-rule-follows-bindings.md)). ADR-0045
counts every path, and the affine checker was the analysis ADR-0063 did not
move onto bindings. So:
- an outer `tx` never ended passed, where each branch began and ended an
  inner `tx`;
- an outer and an inner `tx`, each ended once, were refused as the outer
  ended twice, and so was a lambda whose own parameter is named `tx`;
- with `let end = Database.rollback`, `end(tx)` counted nothing: ending it
  that way once was refused, and twice passed.

The checker follows the binding a name means now. A local bound to a
declaration is that declaration, and a transaction given to any other
function value is refused, since it may be ended there where nothing
counts. No fixture or example changed.

Evidence: [affine-bindings.txt](evidence/E10/affine-bindings.txt)
(`just e10-affine-bindings`).

**Correction, 2026-09-26: a secret travelled through a function value
unlabelled** ([ADR-0079](DECISIONS/ADR-0079-a-function-value-carries-its-label.md)).
A name meaning a declaration was public, and a call through a value was
labelled by its arguments alone. So `let f = secrets.payments` then
`log.public("{f()}")` passed, and so did a record field holding the
function. A declaration named as a value is labelled by what calling it
makes now, and a call through a value carries its callee's label. No
existing program was affected.

Evidence: [labels-through-values.txt](evidence/E10/labels-through-values.txt)
(`just e10-labels-through-values`).

**Correction, 2026-09-26: an effect travelled through a function value
unseen, so a view could read the clock or the page's geometry**
([ADR-0078](DECISIONS/ADR-0078-an-effect-is-performed-where-its-function-is-named.md)).
A view has an empty effect row, and R-037 states that a generic helper
cannot launder its callback's effects. The inference counted calls. So a
view declared `!{}` passed while reading the clock through
`List.map(xs, stamp)`, `let f = clock.now` then `f()`, a helper given
`stamp`, or a record field holding it: R-037's invariant held only for a
callback written as a lambda. The rows of helpers that declare none counted
calls alone, so a view calling one that read `el.offsetWidth` passed too.
A declaration named as a value now performs its effects where it is named,
and a new generality witness pins the named-callback case. No existing
program was affected.

Evidence: [effects-through-values.txt](evidence/E10/effects-through-values.txt)
(`just e10-effects-through-values`).

**2026-09-26: a call through a field holding a function is checked**
([ADR-0077](DECISIONS/ADR-0077-a-call-through-a-field.md)). `r.f(x)`, with
`f` a field of type `fn(Int) -> Int`, resolved to nothing: a wrong argument,
a wrong count and a wrong use of the result each passed `pw check`, and the
backend refused the call by name. Building it found that an effect travels
through a function value unseen, which ADR-0078 takes up.

Evidence: [field-calls.txt](evidence/E10/field-calls.txt)
(`just e10-field-calls`).

**2026-09-26: an arm no value reaches is refused**
([ADR-0076](DECISIONS/ADR-0076-an-arm-no-value-reaches.md)). The
exhaustiveness analysis computed unreachable arms from the start, and
nothing read them. `_ => 0` before `Circle(r) => r` checked, as did a case
matched twice, both refused only by the backend; a literal matched twice
built with its second arm dead. PW0333 refuses each.

Evidence: [unreachable-arms.txt](evidence/E10/unreachable-arms.txt)
(`just e10-unreachable-arms`).

**2026-09-26: a stream and a mounted resource do not build**
([ADR-0075](DECISIONS/ADR-0075-a-stream-and-a-mounted-resource-do-not-build.md)).
Each lowered as a literal element: A-008's `<stream>` was refused by
`pw build` for its `query` attribute, and A-007's
`<map-container resource={StoreMap} center={center} />` built a page that
fails when rendered, with a resource nothing mounts. The template IR refuses
both by name now.

Evidence: [streams.txt](evidence/E10/streams.txt) (`just e10-streams`).

**Correction, 2026-09-26: ADR-0071 left `{:else if o}` over an `Option`
related to nothing, and nothing related what a template writes**
([ADR-0074](DECISIONS/ADR-0074-what-a-template-writes-has-a-text-form.md)).
- **ADR-0071's gap.** Its truth relation left an `Option` or a `Result` to
  PW0600, which reads only an `{#if}`'s subject. So `{:else if o}` over an
  `Option` passed, and the renderer refuses it.
- **What a template writes.** A list, a record, a function, a `Float` or an
  `Option` in a text hole, an attribute or a URL's hole passed, and failed
  when rendered. So did a boolean attribute given a case, a loop keyed on a
  record, and a loop or a key read through a field the value does not have
  (`{#each s.itemz}`, `(x.missing)`).
- **Three fixtures were ill-typed.** Two generality witnesses read a field
  a `StoreId` does not have and rendered a `Result`; the rejected R-037
  keyed a loop on a `Float`. Each is corrected, and each still emits its
  invariant alone.

PW0609, PW0600 and PW0610 refuse each now. The store, kiokun and the
accepted corpus check clean.

Evidence: [template-text.txt](evidence/E10/template-text.txt)
(`just e10-template-text`).

**Correction, 2026-09-26: `pw build` wrote templates that fail every
render, and keyed loops on the wrong field**
([ADR-0073](DECISIONS/ADR-0073-a-template-reads-each-value-by-path.md)).
- **A computed hole built.** `{n + 1}`, `title={"lit"}` and
  `disabled={!b}` lowered with an empty path, and `{mk().a}` with `.a`, so
  every render failed. `{#if !b}` lowered to a part the renderer refuses,
  which `pw emit-template` refused and `pw build` wrote.
- **A `style:` directive built as an attribute named `style:width`**, which a
  browser ignores.
- **A loop's key was its last segment.** `(k.r.id)` keyed on `k.id`, and
  `(item.id)` in a loop over `x` on `x.id`, silently. The last-segment
  audit had filed that site as syntax that resolves nothing.

A computed hole is valid Pleris that this backend cannot render: it checks,
and `pw build` refuses it with the reason. A key is read from the loop's
element along its whole path, and one read from another name is PW5021. No
built program was affected: the store's and kiokun's keys are all
`(x.id)`, and their holes are paths.

Evidence: [template-values.txt](evidence/E10/template-values.txt)
(`just e10-template-values`).

**Correction, 2026-09-26: a view used in another view built as an unknown
HTML element** ([ADR-0072](DECISIONS/ADR-0072-an-element-named-with-a-capital-is-a-view.md)).
The charter writes one view inside another as `<Money value={item.price} />`
(§8.1). Nothing read such an element: it checked, and built as a literal
`<Money>` tag. The view's markup was never rendered, and its props were
checked by nothing, so a prop of the wrong type, one left out, and one the
view does not take each passed. No example composed views, so nothing
shipped this way. PW5020 refuses the element until views compose, and
refuses a tag that names no view. How a view composes needs a ruling (ADR-0072 sets
out two designs).

Evidence: [view-elements.txt](evidence/E10/view-elements.txt)
(`just e10-view-elements`).

**Correction, 2026-09-26: an `elif` chain compiled as its first branch and
its next condition, and calls through function values were checked by
nothing** ([ADR-0068](DECISIONS/ADR-0068-what-each-construct-takes.md)).
- **A silent miscompile.** Every `if a { x } elif b { y } else { z }`, and
  every `else if`, lowered to `if a { x } else b`. A chain of `Bool`s checked,
  compiled and answered wrong through the E8 host. No compiled program wrote
  a chain; A-018's derived value did, and the new branch relation found it.
- **Calls through a function value** were related to nothing: not their
  arguments, their number, their result, nor that the value was a function.
  A local function value named like a declaration was checked against the
  declaration.
- **Unrelated operands.** `for` over a non-list and `?` on a `Bool` checked,
  as did an `if` whose branches produce two types where its value is bound.
  A-005 applied `?` to a `Bool`, and four generality witnesses joined a
  `String` with a `Secret`, a `Result` or a record. Each is corrected.

**Correction, 2026-09-26: a record built with the wrong fields, and an `if`
without `else` as a value, passed `pw check`**
([ADR-0067](DECISIONS/ADR-0067-a-record-is-built-with-its-fields.md)). A
record built by its fields' names was related only field by field, so a
field left out, a field its type lacks, and a field given twice each checked;
the backend was the first to refuse them. An `if` without `else` had no
stated type, so a body declaring an `Int` could end in one. The fields are a
relation now (PW0612), and an `if` without `else` is the unit value.

**Correction, 2026-09-26: a secret read by a nested function was public, and
a nested function's annotations checked nothing**
([ADR-0066](DECISIONS/ADR-0066-a-nested-declaration-sees-around-it.md)).
- **A secret through a nested function.** A `fn` nested in a `fn`, a
  `command` or a `component` was analysed alone. A name it read from around
  it had no type and no label, so a secret the enclosing body held, logged
  publicly by the nested function, passed `pw check`. So did a secret field
  of an enclosing parameter.
- **Unresolved annotations.** A nested declaration had no module, so every
  annotation its body was read by was unresolved. The rejected fixture R-002
  hid two defects behind that: an unimported `Store`, and a `Store` declared
  where a `Result` is answered. Both are corrected.
- **Bindings only the name check knew.** A stream's `<ready as={x}>` and a
  `release(h) { .. }` clause were resolved by the name check alone. A call to
  a function value outside its scope passed, and `(x) => x + 1` did not bind
  its `x`.

**Correction, 2026-09-26: a secret passed through a generic function came
out public** ([ADR-0064](DECISIONS/ADR-0064-a-label-through-a-call.md)). A
call to a declared function was labelled by the declaration alone, and none
of the standard library's list functions declares a label. So a
`Secret<Payments>` logged publicly passed `pw check` when it went through
`List.get`, `List.map`'s result, `List.fold`, or a program's own
`fn first<T>(xs: List<T>) -> Option<T>` on the way. So did `tokens.get(0)`
over a list nothing typed, whose receiver an undeclared call left out. A
result that mentions one of its callee's type parameters now carries the
labels of the arguments that bring it in.

**Correction, 2026-09-26: a name bound twice was checked by none of its
bindings, and typed and labelled by the wrong one**
([ADR-0063](DECISIONS/ADR-0063-every-name-means-one-binding.md)). Three
analyses kept an environment per body keyed by name. Each of these passed
`pw check`:
- **Type errors through a reused name.** The value relations read a name
  bound at two sites as unknown wherever it was used. So `Some(x) => x + 1`
  over an `Option<String>` passed where another match also bound an `x`, and
  so did a lambda's `s + 1` over a `List<String>` where another lambda bound
  an `s`. A `for` loop's name had no type at all.
- **Two accepted programs disagreed.** A-017 passed a
  `LayoutSnapshot<Float>` to `set_width`, which takes a `Float`, where A-016
  reads its snapshots' `.value`, as assumption A-020 states. Nothing checked
  A-017's call, because its loop variable had no type. It reads `.value` now.
  A-020 is open: under its alternative, a phase rule, A-017's original form
  was right.
- **A handler's capture typed by the wrong binding.** The declared-type
  environment gave a name its last binding's type. With a second `{#each}`
  binding `item` to a `Store`, the store page's add-to-cart handler typed its
  capture `item.id` as a `StoreId`, and its capture schema and identity said
  so, silently. The value relations checked the call against the first
  binding; the backend compiled it against the second.
- **Secrets leaked through unlabelled bindings.** The privacy labels gave no
  label to a `for` loop's name, a lambda's parameters or an `{#each}`
  block's name. A `Secret<Payments>` logged publicly through one (PW5006), or
  rendered through one (PW5003), passed. A public value logged under a name a
  secret also bound was refused.

Each use of a name now means the one binding in scope where it is written,
by the scopes the backend lowers with. All three analyses read that binding.
`compiler/pw-core/tests/lexical_scope.rs` states each case with a control,
and every one of its 14 tests fails at the commit before.

**Correction, 2026-09-26: a type and a query of one name failed every
component** ([ADR-0062](DECISIONS/ADR-0062-generic-types-in-the-backend.md)).
A type `Either` and a query `Either` live in two namespaces, and checked.
Where the worlds are generated, the type took the query's place, so the query
had no signature and the whole WIT package failed ("missing component
signature"): every component of the program was refused. A type is never a
component now. `a_type_and_a_query_of_one_name_are_two_things` in
`compiler/pw-conformance/tests/wit_names.rs` is the regression test.

**2026-09-26: what a template's blocks and events take**
([ADR-0071](DECISIONS/ADR-0071-what-a-template-takes.md)): `{#each n}` over
an `Int`, `{#if s}` over a declared sum type, and `on:press={n}` over an
`Int` each passed `pw check` until then, and the first two failed only when
rendered. PW0609 and PW0614 refuse them. The rule fixture
`loop-capture-untyped.pw`, which runs `{#each}` over an `Int` on purpose,
now reports PW0609 beside its PW5016.

Evidence: [template-operands.txt](evidence/E10/template-operands.txt)
(`just e10-template-operands`).

**2026-09-26: an assignment to a field has the field's type**
([ADR-0070](DECISIONS/ADR-0070-an-assignment-to-a-field.md)): `b.value =
"wrong"` passed `pw check` until then, since only a named target was related.

Evidence: [field-assignment.txt](evidence/E10/field-assignment.txt)
(`just e10-field-assignment`).

**2026-09-26: a list's items share one type, and an `Int` literal fits an
`Int`** ([ADR-0069](DECISIONS/ADR-0069-list-items-and-int-literals.md)): both
passed `pw check` until then, `[1, "a"]` as a `List<Int>` and a literal past
64 bits. PW0615 and PW0616 refuse them.

Evidence: [lists.txt](evidence/E10/lists.txt) (`just e10-lists`).

**2026-09-26: what each construct takes, checked**
([ADR-0068](DECISIONS/ADR-0068-what-each-construct-takes.md)): a call through
a function value is checked against its type, and a value that is not one is
PW0614; `for`'s list and `?`'s operand are operands; a used `if`'s or
`match`'s branches produce one type (PW0613); an `elif` chain is nested ifs.

Evidence: [calls.txt](evidence/E10/calls.txt) (`just e10-calls`).

**2026-09-26: a record is built with each of its fields, once**
([ADR-0067](DECISIONS/ADR-0067-a-record-is-built-with-its-fields.md)): PW0612
refuses a field left out, one its type lacks, and one given twice; an `if`
without `else` is the unit value.

Evidence: [record-fields.txt](evidence/E10/record-fields.txt)
(`just e10-record-fields`).

**2026-09-26: a nested declaration sees the bindings around it**
([ADR-0066](DECISIONS/ADR-0066-a-nested-declaration-sees-around-it.md)).
- A nested declaration sees the enclosing declaration's parameters and the
  bindings in scope where it is written. Each is typed and labelled as the
  enclosing declaration has it.
- A nested declaration is in its enclosing declaration's module.
- A stream's parts, and a release clause's name, are resolved, typed and
  labelled. A call's callee is the binding in scope, or a declaration.
  `(x) =>` binds its `x`.
- Not done: the name check keeps its own scope walk.

Evidence: [nested.txt](evidence/E10/nested.txt) (`just e10-nested`).

**2026-09-26: a value that holds at every type**
([ADR-0065](DECISIONS/ADR-0065-a-value-of-any-type.md)).
- `None`, `[]`, `Ok` and `Err`, `todo`, an early return, and a generic value
  whose parameter no field mentions (`Maybe.Nothing`) each hold at every type
  their hole could be. The value relations decide them, where each was
  undecided.
- Undecided relations fall from 41 to 1 in the store, from 20 to 1 in
  kiokun, and from 70 to 18 in the accepted corpus. Every relation decided
  before is decided the same. What remains is other constructs: `measure`
  blocks, dimensioned literals, `derived`, a painter's context.
- A `let mut` holds one type, completed by an assignment: `let mut xs = []`
  then `xs = [n]` is a `List<Int>`.
- A function's link is kept: `Maybe.Just` named as a value is not a function
  of anything.

Evidence: [any.txt](evidence/E10/any.txt) (`just e10-any`).

**2026-09-26: a label carried through a call**
([ADR-0064](DECISIONS/ADR-0064-a-label-through-a-call.md)).
- A declared call's result joins its declaration's label with the label of
  each argument whose type mentions a type parameter the result mentions. A
  piped value and a method call's receiver are the first argument.
- A lambda is labelled by what it computes, and an undeclared call by all
  that goes into it, its receiver included.
- Not done: an implicit flow, such as an element chosen by a secret index.

Evidence: [labels.txt](evidence/E10/labels.txt) (`just e10-labels`).

**2026-09-26: every name means one binding**
([ADR-0063](DECISIONS/ADR-0063-every-name-means-one-binding.md)).
- `crate::lexical` resolves each local name once, to a parameter, a pattern,
  a `use`, a template block or arm, or a policy term's binder. A name no
  binding in scope has is the program's own.
- The value relations, the declared-type environment and the privacy labels
  each keep their facts by binding. A `for` loop's name and an `{#each}`
  block's are typed by their collection's element, and labelled by the
  collection.
- The value relations decide more. Undecided relations fall from 58 to 20 in
  kiokun, from 43 to 41 in the store, and from 96 to 70 in the corpus. Every
  relation decided before is decided the same.
- Not done: a label carried through a declared function, such as `List.get`
  over a list of secrets (next); a value with a hole that means any type,
  which is all 20 of kiokun's remaining undecided relations.

Evidence: [lexical.txt](evidence/E10/lexical.txt) (`just e10-lexical`).

**2026-09-26: generic types in the backend**
([ADR-0062](DECISIONS/ADR-0062-generic-types-in-the-backend.md)).
- A nominal type carries its arguments. A generic record, sum type or opaque
  type is laid out per instance inside a component and a module, where each
  was refused by name.
- An instance's arguments come from its fields, as a generic callee's
  arguments do, or from its use. A parameter nothing fixes is refused by name.
- A generic type still does not cross the component boundary.

Evidence: [generics.txt](evidence/E10/generics.txt) (`just e10-generics`).

**2026-09-26: a declared sum type in a template's match**
([ADR-0061](DECISIONS/ADR-0061-template-matches-over-sum-types.md)).
- A template's `{#match}` takes a declared sum type apart, where it was
  refused (PW5019). Its arms are checked as any match's: a missing case
  (PW0305), a case the type lacks (PW0608), a field count (PW0603).
- An arm binds each field of its case, `{:Rect(w, h)}`, and may name its
  type, `{:Shape.Circle(r)}`.
- The template IR names a declared case by its WIT name, as the value a
  component gives does; the renderer binds a case's several fields from a
  list, and kiokun's server carries a declared case to it.

Evidence: [template-sum-types.txt](evidence/E10/template-sum-types.txt)
(`just e10-template-sum-types`).

**Correction, 2026-09-26: a case named under another compiled as a
binding** ([ADR-0060](DECISIONS/ADR-0060-nested-and-literal-patterns.md)).
`match o { Some(Empty) => 1, None => 0 }` over an `Option<Shape>` passed
`pw check`: the analysis could not see into `Some`'s payload. The backend
then read `Empty` as a binding of that name, so every `Some` took the first
arm, and `Some(Circle(3))` answered 1. It was present since ADR-0036 for any
case name written under `Some`, `Ok` or `Err`, and ADR-0059 extended it to
declared cases. The checker now names the missing `Some(Circle(Int))`, and
the backend compiles such a pattern to a test of the payload.
`pats.EmptyOnly` in `compiler/pw-conformance/tests/patterns.rs` is the
regression test.

**2026-09-26: nested and literal patterns**
([ADR-0060](DECISIONS/ADR-0060-nested-and-literal-patterns.md)).
- The exhaustiveness analysis types each match's subject, an ADT per
  instance. A pattern nested under `Some`, `Ok`, `Err` or a declared case is
  read against its own type, where it was Blocked.
- `true`, `false` and each literal are constructors. A `Bool`, an `Int` or a
  `String` can be matched, and `-1` is a pattern.
- Every name a pattern binds is typed in its arm, a name alone as the whole
  value.
- A match that is not one level deep compiles to a decision tree of nested
  matches and tests. An arm reached on several paths is compiled on each.

Evidence: [patterns.txt](evidence/E10/patterns.txt) (`just e10-patterns`).

**Correction, 2026-09-26: a Pleris name WIT reserves broke the whole
build.** A case `List`, a field `own` or a query `Own` became a WIT keyword
(`list`, `own`), written bare. The program's WIT package then did not parse,
so every component in it was refused, not only the one naming it. Such a
name is written `%list` now, which WIT reads as the identifier `list`, so
values and the host see it unescaped. Found writing ADR-0059's tests.
Evidence: [wit-names.txt](evidence/E10/wit-names.txt)
(`just e10-wit-names`): five of five mutants killed.

**Correction, 2026-09-26: four kinds of wrong program passed `pw check`
with a declared sum type** ([ADR-0059](DECISIONS/ADR-0059-declared-sum-types.md)):
- `Shape.Circle("x")`, a field of the wrong type: a case had no type at all.
- `Shape.Bogus(1)`, a case the type does not declare.
- `match s { Shape.Empty => 0 }` over a type of four cases. A pattern through
  its type parsed as a binding of that dotted name, so it matched everything
  and the match was proven exhaustive.
- `Some(x) => x + "a"` over an `Option<Int>`. The value relations did not
  bind an arm's names in its body, so nothing inside an arm but its result
  was checked.

Each is refused now, with a test and a control in
`compiler/pw-core/tests/sum_types.rs`.

**2026-09-26: declared sum types, typed, built and matched**
([ADR-0059](DECISIONS/ADR-0059-declared-sum-types.md)).
- A case is typed where it is written through its type, `Shape.Circle(3)`;
  a pattern may name its type too; an arm's fields are typed in its body.
- The backend builds and matches declared cases. In the component a sum type
  is a WIT variant, and a parameter's joined flat slots are read back as the
  Canonical ABI reads them, which also lets an `Option` parameter be matched.
  In the module a case is `{ $case, value }`.
- `_`, name and `A | B` arms compile, over any variant.
- A union of one case is a WIT variant; it was `tuple<>`. A type that
  contains itself is refused by name; it was WIT that does not parse.
- Not done: nested and literal patterns, generic sum types in the backend, a
  sum type in a template's `{#match}`, and `==` on anything but a primitive.

Evidence: [sum-types.txt](evidence/E10/sum-types.txt) (`just e10-sum-types`).

**2026-09-25: handlers that compute**
([ADR-0058](DECISIONS/ADR-0058-handlers-that-compute.md)).
- A resumable handler's body is lowered through the backend IR and written
  by the pure-computation emitter. It computes, branches, loops, and calls
  several commands, each awaited in order.
- Until now it was one command call with simple arguments. Each module is
  tested under Node against a context that records what it sends.
- The store's handlers send what they sent; their text changed.
- The event as a parameter is not done: no syntax binds one, and the runtime
  listens for a click only.

Evidence: [handlers-compute.txt](evidence/E10/handlers-compute.txt)
(`just e10-handlers-compute`), and the three-engine browser suite
([handlers-browser-suite.txt](evidence/E10/handlers-browser-suite.txt)).
Its runs 2 and 3 pass all 303 tests. Run 1 failed once in WebKit, in a
keyed-list test the change does not touch ("changing one item's field
replaces no sibling"). It did not recur in five isolated reruns, nor in
fifteen runs of the file with every core loaded, and the server's recovery
thresholds (a 120 s idle, a 256-frame backlog) are out of a test's reach.
The cause is open.

**2026-09-25: maps and sets**
([ADR-0057](DECISIONS/ADR-0057-maps-and-sets.md)).
- `Map<K, V>` and `Set<T>` are language types with standard-library
  modules. Keys are `Int`s or `String`s, in ascending order.
- The component lays them out as the world writes them, `list<tuple<K, V>>`
  and `list<T>`. The module holds sorted arrays.
- A map or set a query is given, or a host answers, is checked on arrival.

Evidence: [maps.txt](evidence/E10/maps.txt) (`just e10-maps`).

**2026-09-25: Unicode case mapping**
([ADR-0056](DECISIONS/ADR-0056-unicode-case-mapping.md)).
- `String.to_lower` and `String.to_upper` map each code point as Unicode
  17.0 does, including mappings to several code points.
- The component and the module read one set of tables, generated from the
  compiler's `char`, and never a platform's mapping.
- A test pins the Unicode version. The first version of that test assumed
  16.0, which a Rust outside the workspace knows; the pinned 1.97.1 knows
  17.0.

Evidence: [case.txt](evidence/E10/case.txt) (`just e10-case`).

**2026-09-25: slicing, and the placeholders computed**
([ADR-0055](DECISIONS/ADR-0055-slices-and-placeholders.md)).
- `List.drop`, `List.slice`, `List.reverse` and `String.slice` compile, in
  the component and the module. Each bound is clamped.
- `List.sum` and `List.maximum` had placeholder bodies that answered 0.0,
  and `List.enumerate` answered `[]`. `sum` and `maximum` are written in
  Pleris now; `maximum` returns an `Option`. `enumerate` is removed.
- A-016, A-021 and R-035 changed with the library.

Evidence: [slices.txt](evidence/E10/slices.txt) (`just e10-slices`).

**2026-09-25: an opaque value is built and read inside a component**
([ADR-0054](DECISIONS/ADR-0054-opaque-values.md)). `Count(n)` and `c.value`
are the same value under another type, `Instr::Retype`. In the component
the value keeps its locals or its address; in the module it is the same
JavaScript value. Evidence: [opaque.txt](evidence/E10/opaque.txt)
(`just e10-opaque`).

**Correction, 2026-09-25: a generic representation never resolved**
(ADR-0054). An opaque type's representation was kept as a spelling, and one
with type arguments was Blocked. So `opaque type Names = List<String>` had
no representation, and `n.value` was refused in its own module (PW0610) as
a member the type did not have. The representation is a type tree now.

**Correction, 2026-09-25: a callback's parameters had the wrong types**
([ADR-0053](DECISIONS/ADR-0053-callback-parameters.md)). The checker gave a
lambda's first parameter the element type of any list beside it, whatever
the callee declared.
- `List.fold(ws, 0, (t, w) => t + w.score)` was refused (PW0609), with the
  accumulator `t` typed as a `Word`.
- A program's own callback taker was refused (PW0605) the same way.
- Every other parameter was untyped, so nothing in a lambda's body that read
  one was checked: not `fold`'s element, a lambda bound with a written type,
  or a returned lambda.

Each parameter now takes the type its use declares. Found compiling an
opaque type's fold. Every fixture reports what it did before. Evidence:
[callbacks.txt](evidence/E10/callbacks.txt) (`just e10-callbacks`).

**Correction, 2026-09-25: five mutation anchors had drifted.** A mutation
control replaces an exact anchor, and later commits had moved five:
- one in `function_value_mutations.py`, reformatted before its evidence was
  recorded;
- one in `names_mutations.py` (ADR-0051);
- one in `pure_mutations.py` (ADR-0050);
- two in `recursion_mutations.py`.

Each evidence file was true at its own commit, and none reproduced at HEAD.
The anchors are repaired, and `just ci` now checks every anchor
(`just mutation-anchors`). Five recipes' NOT CLAIMED texts, which later ADRs
had made false, are corrected, and their evidence is re-recorded.

**2026-09-25: a function is a value**
([ADR-0052](DECISIONS/ADR-0052-function-values.md)). A lambda used as a value,
or a declaration's name where a function is wanted, compiles to a closure.
It is stored, returned, passed to the program's own declarations, and
called through. In the component it is an environment of its captures,
whose first word is its code's slot in a `funcref` table, called with
`call_indirect`. In the module it is a JavaScript function. The
backend now lowers a binding's written type as what its value must be; it
ignored it before.

**2026-09-25: an early `return`, `?`, and `for` loops compile**
([ADR-0051](DECISIONS/ADR-0051-early-return-and-loops.md)). The IR gains
`Return`, and `Local`, `Set` and `Get` for `let mut` bindings; a `for` loop
is a list loop whose body's value is discarded. A callee that returns early
is compiled beside its export, so its `return` leaves it and not its
caller. The checker now says what may be assigned. A binding must be
`let mut` (PW0611), where a parameter or a plain `let` was assignable
before. The assigned value must have the binding's type (PW0607), where
`x = "a"` for an `Int` `x` passed. Evidence:
[control-flow.txt](evidence/E10/control-flow.txt) (`just e10-control-flow`),
13 of 13 mutants killed.

**2026-09-25: recursion and generic callees compile**
([ADR-0050](DECISIONS/ADR-0050-recursion-and-generic-callees.md)). A call is
still inlined, and one that recurses is compiled beside its export, once per
instance, and called: in the Wasm component and in the JavaScript module
alike. A generic callee is instantiated from its arguments. Programs with no
recursion compile as they did: the store's and kiokun's 19 artifacts are
byte-identical to `fd95b59`'s. Evidence: [recursion.txt](evidence/E10/recursion.txt)
(`just e10-recursion`), 7 of 7 mutants killed.

**2026-09-25: a string's escapes are the language's**
([ADR-0049](DECISIONS/ADR-0049-string-escapes.md), settling A-023). One
decoder reads a string token: `\n` `\t` `\r` `\\` `\"` `\{` `\}` and
`\u{..}`, holes, and raw `"""` strings. An escape it does not define is
PW0014. The backends refused such strings, or passed the token to their
targets' rules. Each now encodes the value in its own syntax, Koka's checked
against Koka 3.2.3. The Marko adapter rendered an interpolated string as its
token, braces and all; it joins the pieces now. Evidence:
[strings.txt](evidence/E10/strings.txt) (`just e10-strings`), 16 of 16
mutants killed.

**Correction, 2026-09-25: a member no type has was never refused**
([ADR-0048](DECISIONS/ADR-0048-members-exist.md)). A read or call through a
value is related to its type's members now (PW0610). Eight reads in code that
checked clean named members nothing declares:
- the store's page rendered `{cart.line_count}`, and `Cart` has only `lines`;
  the dev server filled the path from its own state. `domain` now declares
  `line_count`, the count it was computing.
- A-015 read `box.x` and `box.bottom`, called `anchor.bounds()`, and assigned
  `self.style.transform`. It now uses the platform's declared geometry and
  setter, and its claim is witnessed by the effect analysis for the first
  time.
- A-001 read an opaque `PositiveInt`'s `.value` from another module, and
  A-016 and A-018 read members their types lack.

An opaque type's `.value` is its representation in its own module, and
refused elsewhere. Evidence: [members.txt](evidence/E10/members.txt)
(`just e10-members`), 10 of 10 mutants killed; the audit decides 28 member
reads in the accepted corpus and leaves 26 undecided. E9's audit is
re-recorded at the same commit ([value-relations.txt](evidence/E9/value-relations.txt)).

**Correction, 2026-09-25: a name used as a value was never resolved**
([ADR-0047](DECISIONS/ADR-0047-every-name-resolves.md)). `PW0021` examined
calls and qualified paths only, so `let x = nothing` and `{nothing.here}` in a
template passed `pw check`. Every name is now resolved in lexical scope. The
walk found two parse defects in accepted code:
- `derived`, the charter's computed value (§7.5), was a bare name. So
  `let total = derived widths |> List.sum()` parsed as `let total = derived`
  and a discarded statement: A-016 and A-018 computed nothing into `total`,
  `max` and `columns`. It is one expression now.
- `observe intersection(self, threshold = 0.1)` read its named argument as an
  assignment to an undeclared name.

It also found six fixtures reading a session that nothing declared, and one
form submitting to an undeclared handler. Each is corrected, and each is
caught or clean exactly as before. Evidence:
[names.txt](evidence/E10/names.txt) (`just e10-names`), 20 of 20 mutants
killed.

**Correction, 2026-09-25: `pw check` did not type an operator's operands**
([ADR-0043](DECISIONS/ADR-0043-operands-are-typed.md)). `1 == "a"` checked,
and so did two accepted fixtures that divide a `Float` by an `Int`: A-017 and
A-021. ADR-0039 §2 refuses both. PW0609 relates each operand to the type its
operator takes. The fixtures now convert with the new `Float.from_int`, and
their claims are unchanged.

**Correction, 2026-09-25: `{:else}` in a template was a silent miscompile**
([ADR-0042](DECISIONS/ADR-0042-template-branches-and-attributes.md)).
`{#if a}A{:else}B{/if}` passed `pw check` and `pw build`. It rendered A and B
together when `a` held, and neither when it did not. The HIR lowering dropped
every `{:..}` marker. Nothing checked that a block closed with its own name,
and an unknown directive passed `pw check`. All are fixed:
- markers are kept as branches;
- a malformed block is PW5019;
- a template `{#match}` is exhaustive (PW0305).

**Correction, 2026-09-25: a secret in an attribute string checked clean.**
`<a href="/pay/{key}">` with `key: Secret<Payments>` passed `pw check`: the
hole was static text, which no privacy rule reads. Neither renderer
interpolated attribute strings, so nothing leaked. An attribute's holes are
expressions now (ADR-0042), and the page is PW5003.

**Correction, 2026-09-25: E9's value relations left a generic call's `let`
binding uninstantiated** ([ADR-0041](DECISIONS/ADR-0041-kiokun-in-pleris.md)).
E9 claims generic callables are instantiated per call.
- `infer.rs` typed `let ys = List.filter(xs, ..)` with `filter`'s declared
  `List<T>`, and the value relations skip names already bound. So `ys` stayed
  `List<type parameter 0>`.
- A later call over `ys` was then refused (PW0605) for a mismatch the typer had
  made.
- Such a binding is now solved like any call, with a regression test in
  `compiler/pw-core/tests/value_relations.rs`. The E9 evidence is re-recorded.

**Correction, 2026-09-25: a statement keyword could name a value.**
`let query = ..` parsed. Then every later `query` in an expression read as a
`query ..` statement, and the program checked while meaning something else.
The parser now refuses such a name (PW0013, ADR-0041, ruling needed).

Decisions awaiting a ruling:
- ADR-0084: a record or a list has no text form, in a template or a string.
- ADR-0078: a function type states no effect row, so an effect is counted
  where its function is named rather than where it is called; the
  alternative is effect rows on function types, as Koka's are.
- ADR-0073: a computed template hole checks and does not build; compiling
  one, as a value the page computes before it renders, is not decided.
- ADR-0072: how a view composes, inlined into the parent's template at
  compile time (the proposal) or rendered in place at run time; until then
  a view used in another view is refused.
- ADR-0071: a template condition keeps the renderer's truth (a number, a
  string, a list, a record), where a code `if` takes a `Bool` alone.
- ADR-0068: the branches of an `if` or a `match` are related to each other
  only where its value is used, not where it is a statement.
- ADR-0066: a nested declaration sees the enclosing bindings in scope where
  it is written, not every binding of the enclosing body.
- ADR-0065: a declared function's result its arguments do not fix stays
  unknown rather than any type, since a host function may answer at no type
  its arguments fix.
- ADR-0085 (correcting ADR-0064): a declared call's result carries every
  argument given to a parameter that states no label, and a parameter
  declared with one keeps its contract; a function that makes public data
  from a secret takes it in such a parameter. Label polymorphism in
  signatures, or an audited declassification, is the alternative.
- ADR-0063: an element of a labelled collection carries the collection's
  label, as a `for` loop's names, an `{#each}` block's and a `{#match}`
  arm's do; a lambda passed to a call takes the join of the call's other
  arguments' labels; a policy term's tree sees the declaration's parameters
  and none of the body's bindings; and a record literal whose first field is
  shorthand, `P { x }`, still parses as a name and a block.
- ADR-0061: a declared case named like the language's own (`Some`, `None`,
  `Ok`, `Err`) is refused in a template, whose value names those cases the
  language's way.
- ADR-0060: a match that is not one level deep compiles to a decision tree
  that copies an arm's body onto each path reaching it, rather than a join
  point, which the structured IR has no jump for.
- ADR-0059: a case is written through its type in an expression; a case
  without a payload may be written alone where one visible type has it, and
  one with a payload may not (PW0021, naming the qualified form). (An arm no
  case reaches is reported by the checker since ADR-0076.)
- ADR-0058: a command's answer is not read by a handler; the syntax that
  would bind an event to a resumable handler is not chosen.
- ADR-0057: a map's key is an `Int` or a `String`; entries are in
  ascending key order, not insertion order; a map or set from outside out
  of order is refused, not sorted.
- ADR-0056: case mapping is per code point, so a word-final capital sigma
  lowers to `σ`, not `ς`.
- ADR-0055: `List.maximum` returns an `Option`, `None` for an empty list,
  rather than negative infinity; `List.enumerate` is removed, since the
  language has no tuple type.
- ADR-0054: building an opaque value checks nothing, since the language
  states no invariant for it.
- ADR-0053: a callee with no signature types no parameter of a lambda passed
  to it.
- ADR-0052: a closure captures by value; a `let mut` binding assigned after
  the closure is made is not seen by it.
- ADR-0051: `for` over a list is the only loop; there is no `while`.
- ADR-0050: a callee is inlined until it recurses, and only the recursion is
  a call, rather than every declaration being a function of its component.
- ADR-0048: an opaque type's representation is read as `.value`, only in the
  module that declares it, rather than by a constructor pattern.
- ADR-0047: clauses written as statements (`scope component`,
  `release(h) { .. }`, `view { .. }`) are recognised by position, from the
  policy table, rather than re-parsed as policies; `derived` is reserved and
  cannot name a binding (PW0013).
- ADR-0046: whether the next performance work is instance reuse, which the
  measurements favour, rather than a memory strategy.
- ADR-0045: a release in a loop is refused even when the loop runs once; the
  body's value moves a resource to the caller; a declaration whose row
  releases `T` owes its `T` parameter one release on every path.
- ADR-0044: a module's values: `BigInt` for `Int`, objects keyed by Pleris
  field names, and `{ $case, value }` for `Option` and `Result`.
- ADR-0043: `Float.from_int` names the `Int` to `Float` conversion; there is
  no implicit one.
- ADR-0042: `{#match e}{:Some(x)} .. {:None} .. {/match}` is the template
  syntax for taking an `Option` or a `Result` apart.
- ADR-0042: a value interpolated into a URL attribute is one URI component,
  and the URL must begin with the author's text.
- ADR-0041: a binding or parameter cannot be named with a statement keyword
  (PW0013); contextual keywords are the alternative.
- ADR-0040 (Unicode case mapping is ADR-0056's since 2026-09-25):
  `String.to_lower_ascii` maps `A`–`Z` only; Unicode case mapping is
  not decided.
- ADR-0039 §1: `Int` traps where its exact result does not fit; `/` and `%`
  are Euclidean, as Koka's are; a zero divisor traps, where Koka answers 0.
- ADR-0038: a bare pattern name is a constructor whenever some type has one of
  that name, so a binding cannot share a constructor's name.
- ADR-0038: `return` stays a statement, whose value is the next statement,
  rather than becoming an expression.
- ADR-0032: the contract locates each export in its component.
- ADR-0033 / A-022: captured values carried on the element are resume
  metadata, not identity markup.
- ADR-0049: the escape set (`\n` `\t` `\r` `\\` `\"` `\{` `\}` `\u{..}`), and
  `"""` strings are raw.
- (settled by ADR-0049) ADR-0033 / A-023: a string literal has a value only where no escape rule is
  involved, until the language defines escapes.
- ADR-0034 §3: a placeholder is not a refusal until something depends on it.
- ADR-0037 §3: kiokun's share-alike data (CC-CEDICT, JMdict, Tatoeba) lives in
  this repository, attributed; the alternative is to keep only the script.

ADR-0032's other ruling-needed item, the handler sending the pressed loop
instance, is superseded by ADR-0033.

Three ADR-0031 decisions were made without an architect ruling and are offered
for reversal:
- privacy qualifiers are not value-transparent (§7);
- function types exist (§4);
- snapshots are read through `.value` (A-020).

The former status file mixed chronological notes with obsolete headlines such as
"E9 is complete" and "no benchmarks yet". Its complete bytes are preserved in
[the historical ledger](STATUS-history-2026-09-15.md). That ledger records earlier
observations, not the current completion state. No old raw evidence is rewritten.

## last passing commit

`201ca0e` (2026-10-03, ADR-0170): `just ci` passes locally, the workspace's
1796 tests pass, and the browser suite passes 529, with 2 skipped. Its
evidence is `docs/evidence/E14/nested-lists.txt`, recorded at that commit;
ADR-0169's is `row-reads.txt`, at `36d263c`.

E14's gate items 1, 2 and 5 are recorded over all twelve tasks at `0c608dc`,
and again at `4f75868` for what it changed: T08's controls, ADR-0153's and
ADR-0154's rules, keyed reads in three engines, the diffs, the browser suite
and the unsafe table. GitHub CI is the authority for each pushed head.

## completed gate items

- **2026-10-03: E14 gate items 1, 2 and 5, again at `0c608dc`, over all
  twelve tasks.** All four controls hold on all three stacks for each
  (`harness-T*.txt`). `pw diff` reports every task's Pleris patches, and
  the unsafe table counts nine of twelve wrong fixes refused by Pleris's
  checker, none by the frameworks'.

  Every mutant is killed:
  - stream controls, 26;
  - document reads, 4;
  - keyed reads, 10, and press order, 1;
  - diffs, 6;
  - optimistic, 6.

  The browser suite passes 460 of 460 in each of three runs.
- **2026-10-03: E14 gate item 5, and items 1 and 2 again, at `f68f723`.**
  - Item 5, which bug classes became unrepresentable: stated rule by rule in
    `docs/milestones/E14.md`, from `docs/evidence/E14/unsafe-table.txt`
    (`just e14-unsafe-table`). Pleris's checker refuses nine of eleven
    wrong fixes, and the frameworks' checkers refuse none.
  - Items 1 and 2: eleven tasks now. All four controls hold on all three
    stacks for each, and `pw diff` reports every one.

  Recorded at the same commit, every mutant killed:
  - the stream controls, 26 of 26;
  - ADR-0151's, 4 of 4;
  - the diff controls, 6 of 6;
  - the optimistic controls, 6 of 6.

  The browser suite passes 457 of 457 in each of three runs.
- **2026-10-03: E14 gate items 1 and 2, at `013aaff`.**
  - Item 1, semantic diffs for the store: `pw diff` (ADR-0149) over every
    task's Pleris reference and unsafe patch,
    `docs/evidence/E14/semantic-diffs.txt` (`just e14-diffs`).
  - Item 2, at least ten tasks across all three stacks: T01, T03, T04, T05,
    T06, T08, T09, T10, T11 and T12. All four controls hold on each stack,
    in `docs/evidence/E14/harness-T*.txt` (`just e14-harness T..`).

  Recorded at the same commit:
  - the stream controls, 26 of 26 mutants killed;
  - the optimistic controls, 6 of 6;
  - the browser suite, 457 of 457 in each of three runs in three engines.

  The diff controls killed 5 of 6. The survivor bypasses `pw diff`'s own
  refusal of a program that does not check, and `pw build` refuses the same
  programs. Its test, which only asked for the error's code, is corrected
  in the next commit.
- **2026-09-25: E10 task 4, memory strategies evaluated (ADR-0046).** The host
  reports each call's instructions, peak linear memory and instantiation
  time. Every compiled kiokun query was measured on the whole shard. 98.9%
  of Search calls stay in their first 64 KiB page; the heaviest, `T`, grows
  its region to 3.2 MB. A fresh instance (7–14 µs) costs more than a `Place`
  or `Lookup` call. Invocation regions stay; the revisit conditions are
  written down. Evidence: `docs/evidence/E10/memory.txt` (`just e10-memory`).

- **2026-09-25: E10 task 2, pure computation as JavaScript modules
  (ADR-0044).** A query that reaches no host compiles to an ES module from
  the component's own IR. The module and the component agree on 6,800
  generated calls under Node, kiokun's shard rule among them, and ten mutants
  that each restore one of JavaScript's own semantics are caught. `pw build`
  writes the modules. A handler that computes, and Wasm in the browser, are
  not done. Evidence: `docs/evidence/E10/javascript.txt`
  (`just e10-javascript`).

- **2026-09-25: operands are typed (ADR-0043).** Not a gate item; the checker
  gap KNOWN_LIMITATIONS named. PW0609 relates operands and conditions to the
  types they take, and `Float.from_int` converts. Evidence:
  `docs/evidence/E10/operands.txt` (`just e10-operands`).

- **2026-09-25: the template gaps (ADR-0042).** Not a gate item; `docs/NEXT.md`
  named them after ADR-0041.
  - `{:else}` and `{:else if}` are branches.
  - `{#match}` takes an `Option` or a `Result` apart, exhaustively, with its
    payload bound and typed.
  - An attribute interpolates, and each value is escaped for its context. In
    a URL, each value is one URI component.
  - kiokun's two word pages are one, `WordPage`, and its search links are
    `href="/{hit.target}"`. An entry now shows its Korean words, Japanese
    names and character, with nested `{#match}` over `Option<Int>` fields
    (ADR-0037, amended). All 17,597 entries of the whole shard render.
  - Found and fixed: `{:else}` was a silent miscompile, and a secret in an
    attribute string checked clean (corrections above).
  - Evidence: `docs/evidence/E10/templates.txt` (`just e10-templates`).

- **2026-09-25: kiokun's shard rule and ranking, in Pleris (ADR-0041).** Not a
  gate item; the test of E10 task 2's computation that `docs/NEXT.md` named.
  - `examples/kiokun/Shards.pw` and `app.pw` state kiokun's rule and ranking.
    They compile to `shards.Place`, `shards.Places` and a 12 KB
    `kiokun.page.Search`, and the host keeps only the index and the files.
  - On a kiokun-data checkout, the compiled rule agrees with kiokun's on all
    1,485,890 words.
  - The compiled ranking agrees with kiokun's Rust ranking on 28,951 queries
    and 77,225 hits over the whole shard. p50 is 0.25 ms a query.
  - A lookup now reaches every shard, so all 17,597 of the shard's entries
    render, stubs included.
  - Found and fixed:
    - the E9 `let` defect and the keyword names (corrections above);
    - `List.group_by` was missing;
    - kiokun's file names are not its words: its builder writes nine characters
      as `_`, and 33 files are escaped;
    - a component call per word made the load three times slower. Batched,
      the load took 8.6 s to 10.8 s across four runs, against 9.5 s with
      kiokun's Rust rule.
  - Evidence: `docs/evidence/E10/kiokun.txt` (`just e10-kiokun`, with
    `KIOKUN_DATA`) and `docs/evidence/E10/kiokun-mutants.txt`
    (`just e10-kiokun-mutants`).

- **2026-09-25: E10 task 2, the standard library compiled (ADR-0040).**
  - `List`'s operations and a new `String` module are `intrinsic`
    declarations the component backend compiles; the placeholder bodies are
    gone.
  - Each operation agrees with Rust's `Vec`, `str` and `char` over generated
    inputs, trapping where they have no value.
  - Found and fixed: `(a, b) => e` had never parsed.
  - Evidence: `docs/evidence/E10/stdlib.txt` (`just e10-stdlib`).

- **2026-09-25: E10 task 2, pure computation compiled to Wasm (ADR-0039).**
  Not a gate item; the backend breadth `docs/NEXT.md` puts first.
  - The component backend compiles `Int` and `Float` arithmetic, comparisons,
    `&`, `|`, `!`, `if`, string literals, interpolation, records, and calls to
    other declarations, inlined.
  - Every operator agrees with an exact `i128` reference over generated
    inputs, trapping exactly where the reference has no 64-bit value, and with
    Koka 3.2.3 wherever Pleris produces a value.
  - Found and fixed on the way: a query body the parser dropped with an error
    only the CLI saw; a backend that compiled programs that did not parse; a
    Koka backend that emitted invalid negation; an import missing when it was
    called only inside a `match` arm.
  - Evidence: `docs/evidence/E10/pure.txt` (`just e10-pure`).

- **2026-09-25: E10 gate item 2, and the kiokun slice.**
  - The backend compiles `match` over `Option` and `Result`, field reads, and
    `Some`/`None`/`Ok`/`Err` (ADR-0036). The store's components are
    byte-identical afterwards.
  - The kiokun slice (ADR-0037): entry lookup and search over one real shard
    of kiokun.com.
    - On the whole shard, the compiled `Lookup` and `EntryPage` looked up and
      rendered 16,921 entries and followed all 103 in-shard redirects, with 0
      failures.
    - 15/15 in three browser engines, with JavaScript on and off.
  - The differential oracle: all seven compiled declarations agree with
    independent Rust references over 300 generated cases each, and three wrong
    references are caught. Evidence:
    [oracle-2026-09-25.md](evidence/E10/oracle-2026-09-25.md) and
    [kiokun-2026-09-25.md](evidence/E10/kiokun-2026-09-25.md).
  - Found:
    - the platform package depended on the store example;
    - interpolated attribute strings rendered literally;
    - the checker did not check exhaustiveness over `Option` and `Result`
      (corrected the same day, with four related false proofs; see above).

- **2026-09-25: E10 gate item 4, size and performance against baselines.**
  - The store's components are 3,037 to 4,221 bytes, against 5,276 for a
    hand-written Rust `no_std` guest and 43,837 with `std`.
  - Through the same host, `add_to_cart` costs 21.1 µs a call and the Rust
    guest 22.1 µs. The native operation costs 0.30 µs, so per-call
    instantiation dominates; that is named, not reduced.
  - Browser activation shows no measurable change against the pre-E10 runtime
    (15.70 against 15.80 ms median, 21 interleaved samples each, twice).
  - Two corrections made while recording:
    - a one-sample comparison with E7's record showed a fivefold gain that is
      not real;
    - a blocked comparison showed a regression of half again (10.3 against
      15.7 ms), which was machine drift between the blocks.
  - Evidence: [bench-2026-09-25.md](evidence/E10/bench-2026-09-25.md).

- **2026-09-25: E10 gate item 5 and task 10.**
  - Only effect-row producers make a value affine: `DatabaseConnection`,
    `DatabaseTransaction` and `MapHandle`, in the store and in the accepted
    corpus.
  - The store's 10 declarations have 20 typed value positions, with 0 affine
    and 0 annotated.
  - Borrow, lifetime, move and box syntax are not Pleris.
  - Carried captures serialize deterministically. Resume versioning is E7V's.
  - Evidence: [ownership-2026-09-25.md](evidence/E10/ownership-2026-09-25.md).

- **2026-09-25: E10 gate item 3, sustained load (ADR-0035).**
  - Measured first, with no bound in place: 3,000 commands left 3,000 consumed
    events in the outbox. 1,000 departed visitors and 300 menu changes left
    600,000 queued frames and 1,001 materialized entries.
  - Now: 0 events, one `Recovery::Reload` per departed visitor, and nothing
    held once they have been idle 120 s.
  - The runtime acts on `Reload`; it had only logged it.
  - 20,000 compiled `add_to_cart` calls through the host left resident memory
    flat (24 µs per call in release).
  - Evidence: [load-2026-09-25.md](evidence/E10/load-2026-09-25.md).

- **2026-09-25: E10 gate item 1, the store builds from source (ADR-0034).**
  - `pw build` writes every artifact of a checked program:
    - the template IR;
    - both handler modules;
    - all five commands and queries as audited components;
    - the contracts and the WIT.
  - It ran with neither Koka nor Node on the PATH, and its outputs are
    byte-identical to the artifacts recorded separately
    ([build.txt](evidence/E10/build.txt)).
  - Found: lowering refusals did not say which declaration they belonged to,
    and `pw emit-template` re-read its sources with `unwrap_or_default()`.
  - Not claimed: the dev server runs the queries as components.

- **2026-09-25: compiled resumable handlers (E10, ADR-0033).**
  - `backend/js.rs` compiles each handler's body to an ES module;
    `pw emit-handlers` writes `<identity>.mjs`. The store's `add_to_cart`
    handler is `context.command("store.page.add_to_cart",
    [context.captures["item"]["id"], 1])`.
  - Every fact is read from the stage that owns it. `values::named` is
    extracted from the typer, and `contract::component_id` replaces three
    derivations.
  - The element carries exactly the capture paths the handler reads (`item.id`).
  - The host types the browser's arguments by the artifact's own parameters and
    refuses the rest with 400.
  - The dev server has one command path. The server-written modules, the
    address resolution and the per-command glue are deleted.
  - Found: `clear_cart` never committed its state; the Wasm IR's string
    constants were source tokens. Both are fixed.
  - `just ci` passes. Browser suite: three engines, 273/273 in three
    consecutive recorded runs (`f8adc03`). The first recording failed one
    WebKit test. The cause was harness interference: `public-fragment` renamed
    the shared menu on the shared host. It was reproduced on demand and fixed
    by isolating that suite; the failed record is kept.

- **2026-09-24: E10-I, a Pleris-compiled command through the E8 host.**
  - `backend/wasm.rs` now emits Canonical-ABI core modules, with every
    signature, flattening, layout and mangled name from `wit-parser` 0.257.1.
  - `backend/component.rs` wraps them with `wit-component` 0.257.1 and audits
    each artifact against its world through `wit-component`'s decoder.
  - The store's `add_to_cart` is a 4,221-byte component importing exactly
    `pw:host/session#read` and `store:data/carts#add`. All five store commands
    and queries compile and audit.
  - `pw_host::engine::call_within` ran it: the component read the host's
    session, called `carts#add("session-7", "cortado", 2)`, and returned the
    data layer's result.
  - Controls: admission and engine refusals, an unimplemented grant, fuel, and
    a core-identical but component-different import.
  - The dev server's commands run the compiled components, and its closures
    are deleted.
  - Browser suite: three engines, 258/258 in three consecutive runs,
    recorded (`docs/evidence/E10/browser-suite.txt`).
  - Found on the way: `imports_of` reported type exports as imports, which
    corrected E8's audit counts. Per-call compilation also amplified a
    pre-existing Firefox flake; components are now compiled once.


- **2026-09-24: E9-V1..V6, the ordinary value relations.**
  - `compiler/pw-core/src/values.rs` checks, three-valued, with diagnostics
    projected from a queryable analysis:
    - arity and argument types for path, member, piped, query, construction
      and policy-term calls;
    - declared results through branches, arms, `return` and `?`;
    - annotated bindings;
    - written types that resolve to nothing.
  - Language additions: callable type parameters instantiated per call,
    `type` parameters kept, function types typed bidirectionally, `?` as a HIR
    node, and declared-constructor arity.
  - Tests: 38 gate tests; 10 of 10 mutation controls killed.
  - Coverage: every relation in the store program is decided and agrees;
    the accepted corpus has 144 agreeing value relations and 343 resolving
    annotations; `pw audit-values` counts the undecided remainder.
  - Defects found and repaired (corpus C8): in the milestone demo, the
    accepted corpus and the libraries; see the evidence.
  - The pre-C8 text of every repaired rejected fixture is still caught for its
    declared invariant (`corpus_history.rs`).
  - `just ci` passes.
  - Not claimed: member existence, sum-type variant constructors,
    named-argument calls, non-phantom generic layouts at the boundary.


- **2026-09-16 resource repair:** shared requests now return the real terminal
  outcome; cancellation/invalidation fence late publication; command admission
  is atomic within this runtime; unwinding commands retain an explicit unknown
  outcome; logical deadlines and privacy mismatches are enforced. The full local
  workspace passed **937 tests, 0 failures, 1 existing ignored test**, including
  **24 new tests**. All nine initial regressions failed on the old runtime first.
  Formatter, workspace Clippy, corpus checks, recipe gates, and census/tooling
  suites passed. See [ADR-0029](DECISIONS/ADR-0029-owned-resource-flights-and-command-outcomes.md)
  and [scope, reproduction, and census mapping](evidence/E4/resource-repair-2026-09-16.md).
  This is a synchronous process-local repair, not durable exactly-once, a
  production cancellation adapter, or completion of E9/E10-I. Its own PR checks
  must establish the engine-feature build and current dependency audit.


- **2026-09-16: atomic resolved-signature cutover.** Parameter and return slots
  now contain recursive `TypeResolution`; the old written fields and independent
  `Interface::of` derivation are removed. Inference, members, privacy, captures,
  boundary decisions, WIT and backend callable signatures consume resolved
  identity. Contract identities are stable structured projections.
  Eleven new behavioral regressions failed on the old compiler and pass here;
  six authority tests and two historical-boundary controls also pass. Local
  workspace validation: **956 passed, one existing ignored documentation test**,
  including 178 core unit tests and 418 core integration tests. The accepted
  and store programs, formatting, workspace Clippy and census tests pass.
  [Evidence and exact scope](evidence/E9/signature-authority-2026-09-16.md).
  **This is not comprehensive ordinary-call typing or E10-I completion.**

- **2026-09-15 compiler follow-up:** recursive written types and one complete
  return annotation now survive lowering. Nested arguments resolve recursively;
  built-in arity and the qualified type namespace are checked; unit spelling is
  recognized. Direct generic resource results retain their arguments in manifests.
  The local compiler run passed **178 unit tests and 399 integration tests**
  across all 51 integration targets, including 11 new regressions. Formatter,
  compiler Clippy, and the accepted/store corpus checks passed. See
  [the bounded E9 evidence](evidence/E9/recursive-written-types-2026-09-15.md).
  That prior change did not complete ordinary argument/return checking or the
  signature migration. The latter is completed by the September 16 change above.

- The Web Failure Census and layout-attribution repair were merged in
  `c10b825de40528a591e101653218ddff52e9ec6e`. The inventory contains 224 failure
  records and 72 source/build plus 72 runtime obligations. These are research
  records and proposed acceptance contracts, not 224 eliminated compiler bugs.
- The supply-chain repair was merged in
  `4a9bcddc095b3b4d0eac0e0c2ddb8a0143de03d9`. The September 15 baseline CI run
  [35034596466](https://github.com/Kimeiga/perfect-web/actions/runs/35034596466)
  passed both Ubuntu architectures and the license/advisory job. Its head,
  `3bd5dbdb02b84d52a49e033a12bb71cff6a62b31`, was that baseline plus a temporary
  read-only source-snapshot workflow. That helper is removed by this repair.
- The evidence-gate repair adds `errexit` and `pipefail` to the recipe shell,
  drains summary output instead of closing the pipe early, and puts regression
  tests in `just ci`. Locally, all 45 success/failure scenarios over 17 real
  recipes passed. The original shell masked 24 of 28 injected failures.
  Grouped-command, renderer, large-output, and deliberately weakened-shell
  controls also passed. See [the bounded evidence report](evidence/tooling/evidence-gates-2026-09-15.md).
- The existing census Python suite and layout-attribution Node suite were rerun
  locally: 20 tests passed in each. No browser timing measurement was rerun.

The baseline CI result is not a result for this patch. Use the patch's own PR
checks for its Rust builds, audit, and complete gate-regression suite. Earlier
E7/E8 observations remain in the historical ledger and milestone documents;
these compiler changes do not independently re-establish those milestones.

## failing gate items

- **Member existence is not a value relation yet.** `box.x` on a `Rect`
  without `x` is unknown rather than refused. This is the first follow-up after
  E10-I, recorded in KNOWN_LIMITATIONS; it was not one of E9-V1..V6.
- **E10 is closed; what it carried is not done** (ADR-0119): a compiled data
  layer (E10-S10), Wasm for compute-heavy browser modules (E10-T2), and a
  memory strategy beyond invocation regions (E10-M). The handler backend has
  no event parameter, and captures are not a patched part.
- **E7 gate 8 is unstable on this machine** (E7-G8): about half of runs see one
  long frame no page script made long. `just e10-close-bench` records every
  run. Ruling needed on the instrument.
- **E14's gate items 3 and 4 are open.** Items 1, 2 and 5 are met at
  `f68f723`. Items 3 and 4 need agent runs (E14-E), which wait on the
  owner's choice of models and budget. Its plan is
  `docs/milestones/E14.md`.
- **CI cost of the engine, measured:** since E10-I every workspace build
  compiles Wasmtime. On a warm cache, CI on `ebd4696` took 2m54s, against 2m28s
  to 2m47s before. The two cold-cache runs took 5m06s and 5m29s.
- **Broader proposals remain proposals.** Temporal authorization, compatibility
  across live versions, commitment/unknown outcomes, composed budgets, and
  browser-owned editing behavior are recorded in the census. Their presence in
  prose does not establish complete generated-runtime enforcement.

## exact commands to reproduce

```sh
cargo test --locked -p pw-resource
just evidence-gates
just ci
just audit
python3 research/failures/tools/validate.py
python3 -m unittest discover -s research/failures/tools -p 'test_*.py' -v
node --test spikes/layout-phase-scheduler/test/loaf.test.mjs
just e10-component
just e10-i
just e10-handlers
just e10-build
just e10-load
just e10-ownership
just e10-bench
just e10-oracle
KIOKUN_DATA=/path/to/kiokun-data/output_dictionary just e10-kiokun
just e10-browser chromium
just e10-browser "chromium firefox webkit" docs/evidence/E10/handlers-browser-suite.txt
just e9-values
cargo test --locked -p pw-core --test value_relations --test corpus_history
python3 scripts/e9_value_mutations.py
cargo test -p pw-core --test signature_behavior --test signature_authority --test corpus_history
cargo test -p pw-core --test recursive_declared_types --test resolved_types --test call_arity --test canonical_abi --test one_comparison --test evidence_is_current
```

`just evidence-gates` requires Python 3 and `just`, but not Rust or browsers. It
runs actual recipe bodies in temporary trees with fake producers. The other
commands retain their real toolchain requirements. `just doctor` is read-only;
`just bootstrap` installs the pinned project dependencies.

## known environmental issues

**2026-10-02: Firefox hangs under the own-renderer suite on this machine.**
Running `spikes/own-renderer`'s browser suite, Firefox tests stall for 3 to 24
minutes and fail on timeouts, a different set on each run. The same specs on
master `c7cf9c8`, with ADR-0121's changes stashed, failed 6 the same way
(9.7-minute stalls). Chromium and WebKit pass every spec run one engine at
a time. Not a regression; not yet diagnosed.

**2026-09-24: the host's disk filled during E10-I.** 460 GB, with between 0
and 3 GB free, almost all of it used outside this repository. Shell commands
failed until the repository's incremental build cache (3.6 GB, regenerable) was
removed. Builds since use `CARGO_INCREMENTAL=0`. macOS purged
`~/Library/Caches`, which included every Playwright browser build. The browsers
were reinstalled once about 25 GB was released, and the three-engine run was
recorded then.

**2026-09-25: the Playwright builds were purged again.** Free space fell to
6.9 GB while the workspace built Wasmtime into several test binaries (`target/`
is 8.8 GB). macOS then emptied `~/Library/Caches` again, and the Cargo
registry's unpacked sources with it: 44 crates remain unpacked, and builds run
from compiled artifacts. The three browsers were reinstalled (Playwright 1.58.0:
Chromium 145.0.7632.6, Firefox 146.0.1, WebKit 26.0). Wasmtime 47.0.4's API was
checked against docs.rs, because its source was no longer on disk.

The September 16 resource repair used the pinned Rust 1.97.1 and locked registry
snapshot locally on Linux x86_64. Its full workspace run completed with a captured
zero exit code. These real runtime/compiler results are distinct from the earlier
mocked recipe tests. The evidence report records scope and environment.


The September 15 local review environment was Linux x86_64 with Python 3.13.5,
Node 22.16.0 and just 1.58.0. Rust was not available locally; actual Rust builds
and dependency auditing are checked through GitHub Actions, separately from the
injected-producer tests. No compiler or browser behavior is inferred from mocks.

The compiler follow-up used a verified, isolated copy of the pinned Rust 1.97.1
and its locked registry dependencies. Its real local compiler tests are distinct
from the earlier shell failure-injection tests. Long full-suite commands exceeded
the review runner's execution window, so all compiler integration targets were
run to completion in explicit batches; the incomplete attempts are not passes.

Changing the parent recipe shell does not repair every nested shell, conditional,
or independently invoked script. Redirected evidence files may still be partial
after failure. A file's existence is not a successful test result.

## last benchmark summary

2026-10-02, at `bff437c`, Apple M2 Pro (`docs/evidence/E10/close-bench.txt`):
the store's components are 3,037-4,221 bytes, against 5,276 for a
hand-written Rust `no_std` guest; `add_to_cart` costs 18.8 µs a call through
the host against 20.6 µs for the Rust guest and 0.30 µs natively; 19,000
compiled calls leave resident memory unchanged (`load.txt`). E7 gate 8: 3 of
8 runs saw a long frame. Older numbers retain their original revisions,
machines, and scope. In particular, earlier frame-level reads of forced-layout attribution
cannot establish browser non-support; ADR-0027 corrects that interpretation.

## next three concrete tasks

1. **Decrement, remove, and a per-line cart** (§15.3, the audit's fourth
   gap): each line's item, price, quantity and total, the cart's total, and
   controls to decrement and remove, idempotent and optimistic as
   `add_to_cart` is.
2. **A command retried on a transport failure** (§15.4, the audit's fifth
   gap), with its interaction, so a retry is the same mutation.
3. **§15.5's missing controls** (the audit's sixth gap): store and cart
   delays, and a one-shot error for the next real command or read.

Owner decisions before E14-E (agent runs): which models, the budget, and how
Pleris is taught to an agent (`docs/milestones/E14.md`).

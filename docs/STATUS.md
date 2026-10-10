# STATUS

<!-- Charter §3.4 requires exactly these sections. Keep them. -->

**Reviewed:** 2026-10-07, against master `4310769`, with ADR-0232 to
ADR-0234.
**Charter:** v2, `PROJECT_CHARTER.md`.
**Numbering:** engineering E0-E15, public proofs P0-P9, risk experiments RQ-*.

**Current milestone:** E14, the AI benchmark, ahead of E11-E13 by the
owner's ruling of 2026-10-02
([ADR-0119](DECISIONS/ADR-0119-e10-closes-and-the-ai-benchmark-comes-next.md),
[plan](milestones/E14.md)). It starts with the Next.js and SvelteKit stores
and an offline harness; no model is called until the owner chooses the models
and budget.

**ADR-0249, 2026-10-07: a verification shard sets up only what its recipes
need** (the owner's approval, relayed 2026-10-07). A shard spent more than
half its time setting up: shard 3 at `c1738ec` freed the disk for 84 s,
installed browsers for 54 s and built for 131 s, then ran its recipes for
233 s. Recipes of a kind now share shards: only those that drive a browser
install one, only those that read the build build, the disk is freed in the
background, and every run fits the Free plan's 20 jobs at once. Playwright
advises against caching its browsers, so they are not cached.

**ADR-0301, 2026-10-10: kiokun's word page moves an equivalent simplified
form** (track `kiokun`, W6, merged from `4cbe3aa`). Where a word's own file
is a simplified form that equals its one traditional form in meaning, the
page answers 308 to that form's page, the query kept, as kiokun.com does:
`redirect_on KiokunError.Moved permanent`, ADR-0295's clause, now the
program's own. Held to kiokun.com's own `equivalentTraditionalTarget` over
1,643 words, every one with a simplified form among them: 3 moves in the
whole dictionary, nothing unexplained, each target a page. ADR-0295's five
tests keep every assertion. And the word page's tests compile each distinct
program once per test process, its files kept in memory and written into
each test's own directory (ADR-0158): `e14-kiokun-word` took 3 h 06 m on CI,
about 160 s a mutant where it took 220. 69 of 69 mutants killed, and
`e14-redirects` 15 of 15, on CI (run 38006256503). What remains of the
recipe's time is the server's load, Wasmtime compiling the program's
components in a debug build, 7.7 s a test: the integrator's, queued (NEXT,
the infrastructure follow-ups).

**ADR-0300, 2026-10-09: a build is named by what it built** (the
integrator's, merged from `e441733`), the first of three steps to the parts
two pages share kept in place. `pw build` writes `build-id`, a hash over every
file it writes; a host refuses a build that names none and says the name on
every document and on its handler table, beside each page's document schema
and scope. The browser's resume decision holds a document to what the build
that served the runtime says of its page: a document of another schema is
refused (6), as is a capture of a scope that does not flow into the page's
(5), each recovering as a stale handler does. Until then every manifest said
`cart-doc` and `public` and the build `B1`, and the decision compared
constants with themselves. Found on CI: a runtime-recovery mutant ("the
recovery codes are read one place off") survived once the store's page had
its real scope, a session's; the stale press's recovery is now held to its
name (3 of 3). 12 of 12 mutants (verify 37998533705; its WebKit job's one
failure, a sign-up setup timing out in the messages spec, passed on rerun).

**ADR-0299, 2026-10-09: kiokun's examples and pitch accent** (track
`kiokun`, W6, merged from `7e3cb51`). The word page shows each sense's
examples as kiokun.com does, the first and the rest behind a disclosure
that needs no script, and each word's pitch accent from kiokun.com's own
pitch files: morae high and low, and the pattern 平板, 頭高, 中高 or 尾高.
The Japanese examples are held to kiokun.com's own `japaneseExamplesForSense`
over 1,082 sampled words with nothing to name; the Chinese and Korean
examples and the pitch rules, which live inside Svelte components, are held
by hand-written cases. Found: kiokun.com places its entries by a hash over
code points and its pitch files by one over UTF-16 units, held as it is; and
the word page's tests outgrew the 4 GiB bound at one thread per core, run
on four. 28 server tests, 64 of 64 mutants killed on CI (run 37961821792,
beside WebKit's "Load more", fixed on master since).

**ADR-0298, 2026-10-09: the store's data behind the seam, in memory and
on PostgreSQL** (track `store-pg`, W5, merged from `d043fa6`). The store's
operations run through its data layer as the feed's do (ADR-0246), on
either layer, its guarantees stated as a source's (ADR-0207). Five
findings fixed on the way: an order dropped its lines (ADR-0193's ruling);
`OrderChanged` was never consumed from the materializer's outbox in
memory, each order leaving a row for good; a menu change's event was
committed nowhere; `/bench/stock` set a stock no menu knew; and store 48's
items were answered not found. The host's 332 tests pass with the store on
PostgreSQL and again in memory, 12 of the layer's own, 12 of 12 mutants
killed, and every re-anchored script's recipe green on CI (run
37932273524, 29 recipes, beside WebKit's "Load more", fixed on master since
by ADR-0296). The DoorDash track (W8) builds on it.

**ADR-0296 and ADR-0297, 2026-10-09: a change is derived outside the
table, and a commit tells the pages that ask, at once** (CI's two
findings: WebKit's "Load more", failing on every branch, and the feed's "a
follow reaches another reader", failing in CI's three-engine baselines
and nowhere else). Every commit's telling derived each open document
inside the host's one subscriber table, 50 to 200 ms a hold, and every
stream and read waited; and it told the other sessions one after another,
every one with a document reading what the commit dropped, the documents
of pages closed up to two minutes before among them: a failing test's
printed records showed the second reader told five seconds and 47
tellings after the follower, or not at all. A change is now derived
outside the table and pushed only where the document still shows what it
was derived against; only documents whose pages asked in the last three
seconds are derived, one passed by when its page next asks; and eight
sessions are told at once. A failing feed test prints its pages' and the
server's records, and a red mutation baseline shows them. 8 server tests
and 2 of the baseline's,
16 of 16 mutants killed on CI, and all 36 recipes green with WebKit's
"Load more" passing (run 37943438178, its fifth WebKit pass in five).
**Corrected with it:** the arrival-clock merge's comment named the earlier
test's unanswered unfollow as the follow test's cause; it was suspected,
awaited, and the test failed again.

**2026-10-09, every recipe's evidence is in the repository** (an audit,
after six were found missing at today's merges). **Corrected:** nineteen
more recipes their ADRs cite had no evidence on `master`, from ADR-0240 to
ADR-0273 (`e14-clauses-read-once`, `-transition-values`, `-each-heads`,
`-statements-separated`, `-let-discard`, `-resource-clauses`,
`-returned-labels`, `-member-resolution`, `-materialization-chains`,
`-every-entry`, `-arrival`, `-handles`, `-session-to-browser`,
`-form-routes`, `-wide-parameters`, `-matched-resources`, `-trap-causes`,
`-telling`, `-materialization-bodies`): their runs were cited and their
evidence never fetched, before ADR-0281's merge flow fetched it. All
nineteen were run again on `master` at `29cae71` (run 37933879838), every
mutant killed, and recorded. `scripts/evidence_present.py` names any
recipe whose evidence file is missing, and `master`'s CI fails on one
(`scripts/tests/test_evidence_present.py`); a branch's new recipe is
recorded at its merge.

**2026-10-09, navigate merged (ADR-0280, below), and two more recipes
recorded for the first time.** **Corrected:** ADR-0248's `just
e14-map-keys` and ADR-0272's `just e14-stream-boot` were cited and never
recorded on `master`; navigate's run records them (11 of 11, 3 of 3
mutants). With the four found this morning, six recipes were claimed
without their evidence; every recipe is audited for its file next.

**ADR-0295, 2026-10-09: a page at another address of the page is moved
there** (track `kiokun`'s need: `KiokunError.Moved`, declared and never
returned). A page declares the error that means its address is another of
its own, and how it moves: `redirect_on KiokunError.Moved permanent`.
PW0350 holds it to one value of the type of the one parameter the page's
route carries, a case a query the page reads can answer, and not the case
it is absent by. The host answers it 308, or 307, to the page's own route
with that value, encoded as a link's hole is, the query kept and no origin
named, so no `Host` header picks one; kept by no cache; a move to itself or
to `..` is the program's fault, 500, never a loop. 7 compiler tests, 5 of
the host's on kiokun's word page, 15 of 15 mutants killed on CI (run
37912147550). **Corrected with it:** ADR-0262's `just e14-host-bindings`
was cited and never recorded on `master`; that run records it, 6 of 6.

**2026-10-09, three findings fixed, and three recipes recorded for the
first time** (`track/arrival-clock`). **Corrected:** ADR-0257's `just
e14-follows`, ADR-0268's `just e14-keepalive` and ADR-0275's `just
e14-waiting-rows` were cited with their mutants killed, and no evidence of
any of them was ever on `master`; CI recorded all three in run 37918808029
(12 of 12, 8 of 8 and 7 of 7). Fixed: ADR-0290's plan crashed on a push
that touches no recipe, taking the longest of no shards, so every such push
failed its plan job (d81b8af's did): it plans no shard now. A part sent
after a delay was measured 0.25 ms early on CI, the test's clock started
after its request was written: it starts before. And the feed's "a follow
shows before the server answers" ended with its unfollow unanswered; it
waits for the answer. One test and one mutant (`just e14-dealt-by-time`, 9
of 9). Named at the merge: WebKit's "Load more", and five recipes' baselines
red on it and on "a follow reaches another reader" (open, NEXT), whose
evidence is not taken.

**ADR-0294, 2026-10-09: a list renders in its length** (W6's finding on
kiokun's sample). さえこ's 128 names took 215 ms to render, and the curve
was quadratic: 32 names 16.7 ms, 64 57.7 ms, 128 218 ms. Each item's scope
copied the page's whole environment, the page's value and so the list
among it, and every fragment's HTML. An item's scope now shares them by
pointer and holds its own binding alone: on CI, 250 names render in 2.2 ms
and 2,000 in 18 ms, a ratio of 8.3 for eight times the names, where scopes
that copy took 7.1 s for 2,000. 2 of 2 mutants killed, and the renderer's
other recipes re-run (run 37899457744; `e14-nested-lists` again alone, 6 of
6, run 37915325800).

**ADR-0293, 2026-10-09: kiokun's page head, held to kiokun.com's own
code** (track `kiokun`, W6, merged from `4df071b`). The word page's title
and description are kiokun.com's `buildDictionarySeo` in Pleris: its forms,
its meanings as `definitionFragments` makes them, and its cuts at 68 and 158
UTF-16 units, never inside a surrogate pair, where JavaScript's would. The
head carries robots, Open Graph's and Twitter's tags; the canonical link and
JSON-LD wait on the integrator's rulings, the preview image on an image
renderer the owner approves. kiokun.com's own code is the oracle, run by
Node from the owner's checkout and never committed: 5,513 sampled words, no
unnamed difference; CI holds the served head to a committed fixture of its
answers. The integrator's sample of the whole dictionary first: 5,938 of
5,938 answered at 1c's page, p50 2.3 ms, p99 13.8 ms, and さえこ's 128 names 311 ms, the
renderer quadratic in a list's length (fixed by ADR-0294).
23 server tests, 15 browser tests, 53 of 53 mutants killed on CI (run
37911011082, beside WebKit's known flake).

**ADR-0292, 2026-10-09: a mutation script's processes are bounded in
memory** (found by CI's heartbeat). CI's runner died every time
`e14-graphs-on-the-wire` ran there, with exit 143 and no word: under its
mutant "the renderer takes a node twice", the 100,000-node chain test copies
each node's whole rest and keeps it, 14.5 GiB in thirty seconds. Every
process a mutation script starts now holds at most 4 GiB, watched from
`mutation_baseline` in all of the scripts; one past it is stopped and said,
and a kill by the bound is a kind of its own, named in each script's last
line and listed apart in the run's summary, so a suite whose kills were the
bound's cannot pass for one whose kills were its tests'. On CI the runaway is
stopped at 3.9 GiB and the recipe passes in ten minutes (run 37908865560).
15 tests, 16 of 16 mutants killed on CI.

**ADR-0291, 2026-10-09: a host's record is written as the program names its
fields** (W6's finding). kiokun's data layer wrote an entry's field
`chinese_char`, as the program's `Entry` names it, and the query trapped:
the host read a record's fields by their WIT names alone, `chinese-char`.
A host now reads each field by its WIT name, or else by the program's, as
the browser has since ADR-0172, and a field under neither is refused naming
both. Two tests in `pw-host`; `e14-cart-lines` (41 of 41) and
`e14-descriptions` (9 of 9) re-run on CI with their anchors moved (run
37894486009).

**ADR-0290, 2026-10-09: a verification run is dealt by the seconds its
recipes last took** (found in CI: the nightly cancelled). The nightly of
2026-10-08 did not finish: two of its 17 shards were cancelled at the job's
345 minutes with half their recipes unrun, while another ended after 41,
because the plan weighed a recipe by the mutants it plants, which foretell
its time poorly (0.43 over 213 recipes). Every evidence fetch now keeps each
recipe's seconds, and the plan deals the longest first to the shard where it
ends soonest, its setup counted; the database's recipes take as many shards
as end the run soonest. Replayed on that night's seconds: sixteen shards at
about 152 minutes and the database's at 87, where its plan held 437 minutes
in one. 34 tests, 8 of 8 mutants killed on CI (run 37907578665). Amends
ADR-0249 (a recipe needing less may run where more is set up) and ADR-0278
(as many database shards as end the run soonest).

**ADR-0288 and ADR-0289, 2026-10-09: kiokun's labels and character
header, its loader's merges and the header's written forms** (track
`kiokun`, W6, merged from `de0c4ca`). The word page shows JMdict's codes by
kiokun.com's own label table, read from `KIOKUN_APP` (a code the table lacks
as it is, as kiokun.com shows one), and the character header: the learner
gloss, its HSK and JLPT levels, the mnemonic's meanings and each language's
readings. A word's files are read in one batch; kiokun.com's loader rules (a
stub followed, its variants and related forms merged, an equivalent form
found) and the header's written forms are Pleris, each named for the
kiokun.com function it is read from. Found: a host record's field is named as
its world names it, in kebab case (the host reading either name is on
`track/record-names`). Twenty server tests, fifteen browser tests in three
engines, 45 of 45 mutants killed on CI (run 37896778819, beside WebKit's
known flake and the lone-shard summary, fixed on master).

**ADR-0287, 2026-10-09: a handler is held where it runs** (found probing
ADR-0283). A page placed at `build` whose button sends a command was refused,
"`database.write<Thing>` is not available at placement Build", for a lambda
that calls the command and for the command named alike, where its contract
allows `build`, the row check lets it be, and ADR-0113 holds a handler in the
browser, where a command it calls is a request the command performs. Only
the declared-placement check held a handler's work to the page's placement.
It no longer does; a handler that writes the database itself is still
refused, once, where it runs. The DoorDash menu, every customer's alike and
built ahead, needs it. Three tests, 1 of 1 mutant killed on CI (run
37900218861).

**ADR-0283 and ADR-0284, 2026-10-09: a component's contract is what its
code does, and a value of any type is fixed by the call that meets it**
(W6's two findings on kiokun's word page, and what probing them found). A
query that read kiokun's entries through a function of its own checked
clean, and `pw build` refused it: the contract read the declaration's body
alone, where the backend compiles every function it reaches. Probing it
found a query that read the database through a function passed by name, or
inside a lambda handed to `List.map`, required no capability, and its
contract allowed the browser and the build. Placement, the source checks
and the contract counted calls alone, where the row check has counted
function values and members since ADR-0078, and the contract deferred every
lambda as a page's handler. Now a component imports what the functions
compiled into it call, placement and the contract read what a body performs
as the row check walks it, and only a handler's work is deferred; the
store's contracts and worlds are unchanged.
And `List.fold(xs, [], f)` checked clean however `f` built its list: the
accumulator was bound to a list of anything whole, so an `Int` was returned
as a `String`. Each part of any type now gets a variable of its own, which
the function fixes: the checker refuses those folds, and the backend builds
the seed at the fold's solved type. A cached query that read through a
function value was no reader of what it read, so a write never invalidated
it (PW5106); it is now. Eight tests of the contract and two of the fold, and
12 of 12 mutants killed on CI, run 37888911377 (`just
e14-what-a-component-does`).

**ADR-0285 and ADR-0286, 2026-10-09: kiokun.com in Pleris, its inventory
and its word page** (track `kiokun`, W6, merged from `d19850f`). Every
route and feature of kiokun.com's SvelteKit app is inventoried, built,
partial or missing, and held to the app: 102 routes and 63 features. Step 1
began with the word page: `examples/kiokun-site`, a third program on the
development server, reads kiokun's entries through a read-only layer, finds
a word's file by kiokun's own shard rule and escape, follows a stub one hop,
and shows each language's words as kiokun.com's page does, server-rendered
and working with script off. A word too long for a file name was a 503, and
is a 404 (fixed before the merge). Found on the way: kiokun.com's own label
table never matches JMdict's codes, so 53 labels show as codes on the live
site; the rewrite does not copy it, and the owner was told. Its two compiler
findings became ADR-0283 and ADR-0284. Ten server tests, fifteen browser
tests in three engines, 15 of 15 mutants killed on CI (run 37886442522).

**ADR-0282, 2026-10-08: what a value holds is one label** (four soundness
findings of 2026-10-03, reproduced at `d346c43`; amends ADR-0118, corrects
ADR-0128). A secret a query answered, kept by a shared fragment at the edge,
checked clean; a fragment of a session's cart placed at build checked clean;
a page placed at build that read the session through a query of its own
checked clean; and PW5002 said "it requires" and nothing more. Three
derivations of what a value holds fed different rules, and a fragment's
`depends_on` fed the graph alone. Now a secret a declaration answers is
held and one it only uses as a key is not, a materialization reads what it
depends on, and placement reads what a declaration holds, as the cache rules
read it, in the checker and the contract alike. ADR-0128 said effects keep a
session off the build world; they do not, since `session.read` declares no
placement, and only the label does. Six tests, three rejected exhibits (C18)
and 8 of 8 mutants killed (`just e14-held-labels`).

**ADR-0281, 2026-10-08: a merge is held to CI's verification run** (the
owner's decision, relayed 2026-10-08). A merge waited on a local chain of
every touched mutation script, 4.5 hours for ADR-0277's seventeen, while
CI's verification runs the same recipes in seventeen shards in under an
hour. A branch is merged on its green `ci` and `verify` runs now, locally
`just ci`, the changed tests and the change's own mutation script once.
CI's plan missed a change to a browser spec, and plans one now. The record
is the run's evidence, fetched into `docs/evidence/` once the owner approves
that download in the integrator's session; what CI cannot run, the
machine's measurements, macOS's own behaviour and the owner's local data,
stays local.

**ADR-0279, 2026-10-08: direct messages, a conversation read from each
side** (track `messages`, W4, merged from `87dbea0`). A conversation is
private to its two users without a label naming two principals: each reads
it from their own side, through a private query keyed by their own handle
and the other's id, and only the host makes a handle. X's rule decides who
may message whom, `MayMessage(to)`: one may message someone who follows
them, or who has messaged them. A message is a row written in its command's
transaction, in both of the feed's layers (migration 0007), shown before
the server answers and waiting by ADR-0275's rule. A third user, signed in
or not, sees none of it, by page, by `/pw-read`, by cache and by stream.
19 server tests (7 on PostgreSQL), the browser in three engines, and 25 of
25 mutants killed (`just e14-messages`), on CI in a shard of its own.

**ADR-0278, 2026-10-08: a recipe run against a database is a shard of its
own** (found by the messages track; amends ADR-0246 and ADR-0249). The
database job ran its recipes in series on one PostgreSQL, and the messages
track's run 37831080654 outlasted its 120 minutes: `e14-identity` took 33
minutes and `e14-messages` 45, then the job was cancelled inside
`e14-notifications`, and `e14-uploads` never ran. Each such recipe is a shard
of its own now, beside a PostgreSQL of its own; one matrix job gives it only
to them, since a service whose image is empty does not start. They count
among a run's 17 shards, which with the three browser jobs are the Free
plan's 20 at once, and the rest are dealt into the shards left.

**Correction, 2026-10-07: a mutant survived at 07f8a2c.** The first
verification run of `e10-recursion` on Linux (run 37646762298) recorded 6
of 7, and so does this host: "a record or variant is passed flat" survives.
It was killed when written (ADR-0050), because the encoder could not read a
variant's flat values back from memory; ADR-0059's `load_variant` can, and a
record or a variant passed flat to a callee computes what one passed by
pointer does. The tests pass 18 such values, and agree with the browser's
module either way. The mutant is retired as equivalent, and the encoder's
comment says the pointer is a choice. The same run's other failure,
`e10-lexical`'s two label survivors (ADR-0242's finding), had two causes.
Every test of a loop's or a lambda's name held a `Secret<Payments>`, secret
by its type; and a log in a loop over secrets, or in a lambda given them, is
refused without the name's label too, for running where a secret decides it
(ADR-0129). The label is what makes the diagnostic name the value logged,
"cannot log a `Secret<Payments>` value", where without it the log is refused
"where a `Secret<Payments>` value decides it". Two tests now log a `String`
a helper read a secret into, and hold the diagnostic to the value.

**ADR-0280, 2026-10-08: a handler navigates after its command commits**
(the owner's brief, from a Next.js bug: a save, then a soft navigation to a
page the Router Cache served from before it). `navigate Page(args)` names
the page and types its parameters, and is written last in the `Ok` arm of
a command's answer: an `Ok` is a commit, and the runtime refuses one that
did not commit. From the navigation the page takes no press and no second
navigation, and each press made before it is answered first, whichever
answer comes first; then the page's address is loaded, read after the
commit by the order of answers and reads, never from a cache. The store's
"Place order" goes to the order's page, in three engines. Two premises of
the brief as NEXT wrote it were wrong: a link's arguments are not checked,
and no ruling carries a basis across a navigation, which here is a document
load. Keeping the parts two pages share in place is the next ruling. Merged
2026-10-09: 6 compiler tests, 10 browser tests, 19 of 19 mutants killed on
CI (run 37911914095).

**ADR-0277, 2026-10-08: a materialization is kept, and a page reads it**
(ruling 10's last piece, second part). A materialization that derives its
value is a component now: each `query R(..)` its body reads is the
platform's read of R, `pw:host/reads`, a dependency the contract names and
no capability. The host answers a read from R, a query through its kept
answer or a materialization from its entry, and runs the body again for
each read it lacks, until it asks for nothing more. Its value is the
materializer's entry, encoded to read back whole. An event that reaches
one, by its own `invalidates_on` or through what it reads, makes the chain
again, the dependency before what reads it, each once; each open document
that reads an entry made again with another value, at its key, is told,
and no other. What could not be made again is served its last good value.
The store's page shows "3 items in 1 section", `MenuLine(id)` from
`MenuSize(id)` from the menu, live as the menu changes, in three engines. A
page reads a public one alone; a private one waits for a partition by
principal. 21 of 21 mutants killed (`just e14-materializations-kept`). The
first run killed 19: the test of a document that reads another key held
only that it was sent no operation, and a document read again whose page
did not change is sent none either. It holds now that the document is not
told.

**ADR-0276, 2026-10-08: `pw fmt` changes no program's meaning** (found
formatting the feed; amends ADR-0013). `pw fmt` wrote the feed's `max_bytes
5_000_000` as `5 _000_000`, which `pw check` refuses, and `-1` in a clause
as `- 1`: a clause's value is tokens read as text, which the rules for an
expression's spaces split, and ADR-0013's gate compared only the tokens. A
`_` between two digits now groups them, one token, as in Python, JavaScript
and Go; a policy's value keeps its gaps as written; and `pw fmt` refuses to
write a program whose tokens, or a value's gaps, it would change, naming the
first. Two layout defects with it: a lambda's `else` branch a level deeper
than its `if` branch, and a clause's continued value flush with the
clauses. The feed, the store, the demo and the generality cases are held to
`pw fmt --check` now, 342 programs. 9 of 9 mutants killed (`just
e14-fmt-meaning`).

**ADR-0275, 2026-10-08: a row shown before the server answers waits**
(found by W3, ADR-0274). A post the page shows before the server answers is
keyed by an id the page made, `pending-..`, and its Like found no such post.
Until the server's row stands in its place, its author and handle are
placeholder links and its Like and Delete are disabled; the server's row
links and acts, and a like pressed on it is counted. The program's own,
`waits(id)` in the feed, as ruled at notifications' merge. A reply shown
before the server answers still links to itself: the thread's replies are a
view that contains itself, which computes no value yet (ruling 0073-a). 7 of
7 mutants killed (`just e14-waiting-rows`).

**ADR-0274, 2026-10-08: notifications** (track `notifications`, W3, on
ADR-0270). A like, a reply or a follow that involves you writes a
notification, a row in the act's own transaction, in memory and on
PostgreSQL (migration 0006), for the post's author, the replied-to post's,
or the one followed; one's own act writes none, and a deleted post takes
its rows with it. The reader's notifications and unread count are private
queries keyed by the reader's handle, `cache private`: another user's
session, signed in and not, sees none of it by page, by `/pw-read`, or by
the cache every reader shares. The home page shows "Notifications: N
unread", `/notifications` lists them twenty at a time, and Mark all read
reaches each of the reader's sessions. 23 of 23 mutants killed, the browser
suite 9 of 9 in three engines (`just e14-notifications`, on the database
job). W3 stopped at the account's weekly limit after its last push, its work
complete; the integrator merged it from its tip.

**ADR-0273, 2026-10-08: a materialization is a value its body derives**
(ruling 10's last piece, "materializations made real", first part). A
materialization was declared and inert: no body, type or generator, and no
page could read one. One that declares its type derives it in its body now,
held to it as a query's is, reading what it depends on as a page does,
`query R(..)`, which the graph takes as its edges, so a cycle or a private
read through them is refused as through `depends_on`, and `depends_on`
beside a body is PW5110. Found on the way: a materialization was in the
views' namespace, so a read of one was never typed, and a test of a chain
passed on nothing; and inside a block a clause's value ended only at a known
head, so a body beginning with a name or a literal was the last clause's
value. A materialization is a term now, and such a value ends with its line
unless it cannot have (`just e14-materialization-bodies`). Its generator,
and the host serving one, are the next parts.

**ADR-0272, 2026-10-08: a region the browser fills while the runtime boots
is bound** (found by the notifications track on CI, run 37748154579). A
press on a recommendation Chrome had streamed into its region did nothing:
the runtime indexes the page, then boots across the network, and Chrome 150
filled the region meanwhile, which the runtime had indexed pending and
never read again, its buttons bound to nothing ("no element 1 for part
6"). A region pending at the first index and settled when the runtime has
booted is read again and bound now, and reported filled by the browser. A
test holds the runtime's boot at the network until Chrome has filled the
region, so the race comes every time; without the fix it failed 4 runs of 4
(`just e14-stream-boot`).

**ADR-0271, 2026-10-08: a reader is told once per burst, and served in
turn** (found by WebKit's "Load more", which failed on CI six times). The
records a failing feed test keeps showed it here, under six workers: a
page received some seventy renders of others' posts, each of its twenty
rows, while its own read for forty waited past five seconds. Each commit
told every other reader (ADR-0219), and a session's hold, a `std` mutex,
was taken in no order, so a reader told again and again took it before the
reader's own read. A reader is told once for every commit waiting, one
telling at a time, and one more for a commit that came meanwhile; the
session's hold is taken in turn (`Turns`). Four rounds of forty, before
and after in turn: 13 of 160 failed before, 2 of 160 after (`just
e14-telling`).

**ADR-0270, 2026-10-08: the reader's user is the host's, and a listener's
handle binds a user's id** (track `notifications`, W3; amends ADR-0091).
`context.current_user()` is the host's now, answered from the session's
principal, or its guest where no one signed in; a user's id is the
platform's `capability.UserId` throughout the feed, which a program may
make, where a handle it may not (ADR-0263). In `invalidates_on` alone, a
parameter of the platform's `User<T>` binds an event's value of type `T`:
`invalidates_on Notified(reader)` drops the entries at that user in each
of their sessions, and their open pages are told. Telling only that user's
sessions is queued.

**ADR-0269, 2026-10-08: a resource is held by what takes it apart** (found
by ADR-0250). `match Maps.create(c, at) { Ok(h) => Maps.destroy(h), Err(_)
=> () }` was PW2005, "matched where it is acquired", and the same match on
a binding, or `let h = r?` on one, was "not consumed on every path": a
release of `h` counted for nothing, and the failure, which holds no handle,
owed one. What carries a resource is held by what takes it apart now, the
arm's name or the `?`'s, which must end it once on every path, and an arm
or a failure that carries none owes nothing; `Ok(_)`, a `_` that meets the
handle and a name holding it whole are refused where they drop it. The
last-segment audit refused the first version's `Result.Ok`, read by its
last segment: the cases are named exactly (`just e14-matched-resources`).

**ADR-0268, 2026-10-08: a command outlives the page that sent it** (found
by a WebKit failure on CI). Verify run 37728182124 failed "a press the
server refuses is restored on the cart's page" in WebKit: its setup pressed
Add, waited for the cart's count, the speculation's, shown before the
request leaves, and read `/cart`, which held no line. A user who follows a
link at once meets the same race, and loses a press the page showed taken.
Each command's request is marked `keepalive` now, which the Fetch standard
lets outlive its document, within 64 KiB of such bodies in flight, counted
in bytes; one past what remains is sent as before. The five tests that
pressed and then left the page wait for the press's answer, and one leaves
it at once on purpose (`just e14-keepalive`). The cause is inferred: the
run's trace is not read.

**ADR-0267, 2026-10-08: a component's trap says why it stopped**
(ADR-0259's "Not claimed"). The browser's module names each trap, and
ruling 0057-c's repeated key stops an invocation "by name"; in the
component every trap was Wasm's `unreachable`, and the host reported a
failed call. Each trap of the component's own calls a function named by its
cause before it stops, the name section saying `pw-trap: <its words>`, and
the host reads the cause from the frame the trap stopped in, as Wasmtime's
own documented example reads a function's name; Wasm's own traps it names
by their code: `stopped: Int overflow: ...`. The host's engine states the
backtrace and the absence of inlining that reading a frame needs, both
Wasmtime 48's defaults. The browser test compares a trap by its cause now,
where it compared that both stopped (`just e14-trap-causes`). The committed
components, the store's and kiokun's, hold the seven functions and the name
section, 315 bytes more each, and are committed again (`just e10-component`,
`just e10-kiokun`); the invariants controls' first baseline, red, found them
stale.

**ADR-0266, 2026-10-08: an export's parameters past the flat limit arrive
in memory** (found by the uploads track). The Canonical ABI passes
parameters flat up to 16 core values, and past it stores them as a tuple
the host allocates and passes one pointer to; the backend refused such an
export, so a row's derived value could not take an `Item` past 16 values,
and the uploads track carried a post's image as a list. Each parameter is
held where the host stores it now, at its offset in the tuple, aligned as
its type is (`just e14-wide-parameters`).

**ADR-0265, 2026-10-08: a form goes where something answers it** (the
identity track's second question and the uploads track's fourth). A link
was checked against the program's pages and a form not at all, but a
file's, so a form could send a `get` to `/sign-out`, which answers a
`post`, and a link to `/sign-in` was refused, its route the host's. The
route table states what each route answers now: a page a `get`, the
relying party its four, every deployment's (`/sign-in`, `/sign-up` and the
callback a `get`, `/sign-out` a `post`), an upload a `post` of a file. A
form's action answers its method or is PW5041, and a link reaches what
answers a `get`; the development server's identity is held to the same
list by a test (`just e14-form-routes`).

**ADR-0264, 2026-10-08: a session's handle never reaches the browser** (a
soundness defect found writing ADR-0263). A session's id is its `HttpOnly`
cookie's value, which the page's scripts never read, and a page could print
it: a query answering a record that held the session, printed as
`{mine.session}`, checked clean. No query's, subscription's or command's
answer, and nothing markup prints (a hole, an attribute's value, a hole in
an attribute's string, a view's prop), holds a session's handle now
(PW5040); a user's or an organization's id is a name, and may. One corpus
witness typed a session-scoped summary as `Session<SessionId>`; a value's
scope is where it came from (ADR-0129), and it says so now (`just
e14-session-to-browser`).

**ADR-0263, 2026-10-07: a session's, a user's or an organization's handle is
the platform's to make** (a soundness defect W3 found, generalized here). A
handle reads the data of the one it names, and a program could make one:
`Session("someone-elses-session")` checked clean, and so did a command whose
session the browser supplies in its body; the platform's own
`current_user()` was `User("")`, every reader one user. No program
constructs a handle now (PW5037), no data layer's operation answers one
(PW5038), and no command's or page's parameter or signal holds one
(PW5039); `current_user()` and `current_organization()` are the host's
operations, `pw:host/principal#read` and `pw:host/organization#read`. An id
is still the program's to make: it names someone and grants nothing (`just
e14-handles`). Next, ADR-0264: a session's handle printed into a page gives
its cookie's value to the page's scripts.

**ADR-0262, 2026-10-07: a host binding is an operation a host provides**
(found by the uploads track). A binding was any quoted string, and one with
no interface, `feed:uploads#claim`, broke WIT generation for every
component of the program with an error naming no line. It is held where it
is written now: `"namespace:package/interface#name"`, each part a WIT
identifier as wit-parser 0.257.1 holds one, or PW0335 at the clause, saying
which part and why (`just e14-host-bindings`).

**ADR-0261, 2026-10-07: a deleted post's image is not served, and its blob
is collected** (ADR-0260's first question at its merge). ADR-0260 served
every blob it kept, and kept every one, so a deleted post's image stayed
served at the address its readers had been shown. A blob is served now
while a committed row names it, as the data layer says, and a layer that
cannot say is answered 503; once a delete commits, a blob no post names is
deleted, in memory and on PostgreSQL, unless a command in flight holds a
claim to the same bytes, which its post will name. One of ADR-0260's
mutants, a lease served where committed images are, is retired as
equivalent: no post names a lease's bytes, and that is asked first (`just
e14-uploads`). Found on the way: ADR-0260's one local intermittent, a test
whose catalog query could read another test's constraint as its schema was
dropped; it reads its own first now.

**ADR-0260, 2026-10-07: an image on a post** (track `uploads`, W2, the
second track merged under ADR-0253). A program states its upload,
`upload PostImage` with its route, what it serves, its bytes, its types
and its width and height, checked at build (PW5601-PW5603: each clause
stated and literal, its paths its own, and a form that sends a file
posting it to one), and the host holds a browser to it; a deployment may
lower a limit, and one raised stops the server at its start. What a file
is comes from its bytes, PNG's, JPEG's (an Exif turn included), WebP's and
GIF's headers read by hand, never from what the browser says it is. A
signed-in user attaches within an hour's budget (429 past it); the bytes
wait in the server's memory, leased to the session, until a command claims
them and publishes them with the post, in its transaction, or discards
them, PW2005 holding each path; the image is the post's own data, in memory
and in migration `0005`'s columns. Only what a post committed is served, at
its SHA-256, typed by its bytes again, `nosniff`, sandboxed and immutable.
No crate was added. Its questions at the merge: a deleted post's image
stops being served and its blob is collected (queued first), a runtime
that sends a file so a draft survives attaching (queued), `e14-uploads` on
the database job (done), and every form's `action` checked with the host's
routes (queued, with identity's). Its recorded evidence is from `2af787b`,
before its last rebase; the merge's verification records it again.

**Correction, 2026-10-07: a test read a region one way it can arrive.** The
verification of `e075289` (run 37694155286) failed `e14-keyed-reads` and
`e14-query-blocks` on one server test,
`a_failed_estimate_fills_its_region_and_the_page_is_served`, and
`e14-slots` and `32f293b`'s `e14-not-found` on a test their mutation
baselines do not name; before `e075289` the suite had passed on CI each
time it ran. The test read the estimate's region from its patch, after the
document. But its estimator fails at once, and a region whose query has
answered when the document is rendered is rendered in place, with no patch
(`Settling::for_document`). Forced here, by holding the document back half
a second, the test fails as CI's did, "the region's arm"; it reads the
region in place or as its patch now, as the slots' tests do. That CI's
failures were this one is inferred, since the evidence kept the test's name
and not its assertion: a recipe that keeps a test run's lines keeps where a
test failed now too, its `panicked at` line, and the next verification
says.

**ADR-0259, 2026-10-07: a map or set from outside is sorted on arrival**
(the owner's ruling 0057-c). A query's map or set, or a host's answer,
given out of order stopped the invocation (ADR-0057), so every host had to
give code point order: a database's collation, an order by UTF-16 unit, a
hash map's iteration, each was refused. It is sorted as it arrives now, in
the component and in the browser's module, and a key twice still stops the
invocation, the module's trap saying so. The divergences from the Component
Model's `map` and from `Map.from_lists`, both keeping the last of a repeated
key, are recorded (`just e14-arrival`).

**Correction, 2026-10-07: two tests edited the feed where they did not mean
to.** CI's verification of `4fde2cb` (run 37700255338) found
`e14-every-entry`'s "a private drop is told to no other session" surviving.
Its test made "the first shared cache keyed by `id`" private, meaning the
thread's; ADR-0257 declared `Profile`, the same text, before `Thread`, so
the test made the profile private, the thread stayed shared, and the mutant
had nothing to fail. It is anchored on the thread's own declaration now, and
kills it. An audit of every anchor the tests write in the feed found one
more: `optimistic_keys.rs` rewrote `like`'s arm and the following
timeline's clauses with the post's target; it rewrites the post's alone.
And a recipe that kept a test run's results alone, or a few tests by name,
names a failing test now, by its `---- name stdout ----` header:
`e14-not-found`'s did not, on CI, when one test of 189 failed (found by
W1).

**ADR-0258, 2026-10-07: accounts and sign-in** (track `identity`, W1, the
first track merged under ADR-0253; the owner confirmed argon2 0.6.0 before
it). Pleris is the relying party: a sign-in's state, nonce and PKCE S256,
its callback, a session rotated at sign-in and sign-out, `HttpOnly` session
cookies, `Secure` off loopback, a same-origin check on every request that
changes something (Go 1.25's `CrossOriginProtection`), and `requires`
evaluated: `SignedIn`, and `OwnsPost(post)` read in the command's own
transaction, a refusal answered 403 with its predicate. The provider is the
deployment's, behind `identity::Provider`; a development provider, its
passwords Argon2id, starts only where the deployment says development on
loopback. The guest model stays the development server's default. The
feed's posts, replies and likes are their principal's, and an author may
delete a post. Merged onto the follows timeline, whose reads and writes ask
the session's principal now, and whose rows offer Delete on the reader's
own. Its recorded evidence is from `b38d10d`, before the rebase; the
merge's verification records `e14-identity` again, on the database job,
which now sets up browsers and the build for a recipe that needs them
(the integrator's answer to its third question).

**ADR-0257, 2026-10-07: the follows timeline** (the integrator's track,
ADR-0253; ADR-0195's ruling 10's feed). A reader follows and unfollows a
user from their page, `/user/{id}`, which counts who follows them and whom
they follow and lists their newest posts; the button and the count show
before the server answers. `/following` is the timeline of those a reader
follows, and its own posts, beside the home page, which stays everyone's.
Both are idempotent, as Mastodon's are, and following oneself or no one is
not found. A follow is a row the feed's source holds (`database.write<Follow>`),
so PW5106 holds both commands to every reader of one: each drops its
reader's following timeline at every limit, ADR-0256's `_`, and its
relation, and tells every open page of the user by its event. In memory and
on PostgreSQL, migration `0004_follows` (`just e14-follows`). Found on the
way: the browser suite's hosts took `PW_FEED_DATABASE_URL` from whoever ran
the suite, so a run with it set put every engine's feed host on one
database, and each run's posts on the last's; three mutation runs' browser
baselines here failed so. They keep the feed in memory now, whatever the
environment. And a failing feed test keeps what its page's runtime said
beside its trace, for WebKit's intermittent "Load more": its second trace
shows the server answering the read, `{"applied":1}`, and the page keeping
its twenty rows.

**Correction, 2026-10-07: and the rest of `body_label`.** The verification
of `5aa9430` (run 37682873614) ran `e10-privacy-by-resolution` for the first
time since ADR-0112 recorded it on 2026-09-26, and "the body's label is read
by spelling" survived. The label it undid, `body_label`'s, was joined in the
shared-cache rule with `Reads::observed`, which since ADR-0128 reads the
same calls, resolved the same way; a call only `body_label` resolved names
nothing the workspace resolves, and is refused (PW0021). `body_label` is
removed, and the mutant retired.

**ADR-0256, 2026-10-07: an entry written `_` is every entry at the rest**
(ADR-0195's ruling 10, its other half). An `invalidates` key's `_` was a
name that resolved to nothing, so a command could drop one entry of a query
and no more. Now `invalidates Timeline(current_session(), _)` drops the
session's timeline at every limit: the command computes the values it
gives and calls the platform's function that takes them,
`feed-app-timeline-EVERY-1`, and the host drops the entries at them,
whatever the rest. An event's unbound position is the same, where it
dropped the whole query. `_` in `emits` is refused by the parameter it
leaves out, and a bare key's repair says how every entry is written.
Found on the way: an invalidation dropped a private entry in the committing
session's partition alone, so a private query keyed by anything but the
session kept every other reader's copy, untold. It drops it in every
session's, and tells another session unless a key pins the entry to the
committing one (`just e14-every-entry`).

**ADR-0255, 2026-10-07: a materialization may read another** (ADR-0195's
ruling 10, its compiler half). `depends_on` looked names up among
resources, so one naming a materialization was refused, PW5103; it names a
resource or a materialization now, and its arguments are checked. A cycle
is refused, PW5109, once, by its first member; a shared materialization
reading a private one is PW5101; and what one reads, the one reading it
reads too, so a write reaches each of a chain (PW5106), and the key audit
counts what a materialization read separates. Found on the way: a clause
naming a function or a view, no node of the graph, made an edge to nothing,
and no check said so; it is refused (PW5103). The runtime already followed
reads, and a test holds it for a chain. A materialization still has no
body or generator; that is queued (`just e14-materialization-chains`).

**Correction, 2026-10-07: a mutant equivalent since ADR-0128.** ADR-0255's
run of `reads_through_calls_mutations.py` whole, its first since ADR-0118
recorded it on 2026-10-02, found "a body reads one call deep" surviving. It
dropped what a body's reads give from `body_label`, whose one reader, the
shared-cache rule (PW5004), has also joined `Reads::observed` since
ADR-0128: what the same reads give, through the same fixed point. The join
it undid is removed, and the mutant retired.

**ADR-0254, 2026-10-07: a member names the declaration its module sees** (a
correction, found with ADR-0250). The member table kept one declaration per
receiver and name, and a second replaced the first: `h.destroy()` was
`VendorSdk.destroy`, which releases nothing, in a module importing only
`Maps`, and `s.current()` was one of three `current`s whatever the module
imported. Every declaration is kept now; of several, a member is the one the
module declares or imports, or that is declared beside its type, and a
module that sees several or none is refused, PW0628
(`just e14-member-resolution`).

**ADR-0253, 2026-10-07: two tracks are built in parallel, each in files of
its own** (the owner's approval of parallel workers, charter R10). Accounts
and sign-in (W1) and image uploads (W2) run beside the integrator, who keeps
the follows timeline and alone merges, numbers ADRs and writes these
documents. Each track has an imported justfile (`just/identity.just`,
`just/uploads.just`), a block of codes (`PW55xx`, `PW56xx`, each held to
its owner by a test), and a module of the development server
(`identity.rs`, `uploads.rs`) reached from `main.rs` at lines marked
`TRACK SEAM`; `requires` is decided there now. `verify` runs on a track's
push, for what it changed. The protocol is `docs/PARALLEL.md`.

**ADR-0252, 2026-10-07: a value returned early carries its label to the
caller** (a correction, found holding those survivors). A body's label was
its last statement's, joined with each `?`'s value: a `return` inside a
branch or a loop was in neither, so `fn g() -> String { if c { return
token() } "none" }` was public to its callers, and `log.public(g())`
passed a secret. Each `return` counts now, with the conditions it runs
under, as does each `?` (`just e14-returned-labels`).

**ADR-0251, 2026-10-07: a resource's clauses are held to PW2005** (a
correction, found with ADR-0250). A `resource` declaration's `acquire` and
`release` clauses are terms no statement reaches, and PW2005 walked none of
them; a component's `resource` statement's were walked, but its `release`
owed nothing. A `release` that never ended its handle passed in both forms.
Now what `acquire` makes, the resource holds, and what `release` is given it
ends exactly once on every path, where `acquire` acquires it; an acquisition
in another clause is refused (`just e14-resource-clauses`).

**ADR-0250, 2026-10-07: `let _` discards a value, and an acquisition is
held or refused** (the owner's ruling 0099-a, Twitter item 2, and a
correction its probes found). `let _ = e` parses and binds nothing: a
`Result` so discarded is handled, and a resource bound to `_` is refused as
never consumed. PW2005 followed only `let x = acquire()` and `use x = ..`;
an acquisition dropped by a statement, given to a call that does not end
it, in an `if` statement's branch, held with `?`, or returned by a function
value passed unread, and a body declaring `()` gave its caller a
transaction it never had. Each is held by a name, ended where it is made,
given to a caller, or held by a resource's `acquire` clause now, or refused
(`just e14-let-discard`).

**ADR-0248, 2026-10-07: a map's key is an `Int`, a `String`, a `Bool`, or
an opaque type over one** (the owner's ruling 0057-a, Twitter item 2). A
`Map<Float, Int>` checked and only `pw build` refused it, and a
`Map<ProductId, V>` could not be written. `Bool` keys (`false` before
`true`) and opaque types over an ordered type are keys now, in the
component and the browser's module alike, and another key is refused at
check (PW0627), where it is written or where a call instantiates it
(`just e14-map-keys`).

**ADR-0247, 2026-10-07: a clause written in a block is judged by its
domain** (a correction, found with ADR-0243). PW0335 judged a policy only
where it headed a declaration; a block's clauses were judged by nothing:
`scope bogus` in a resource's block, `captures bogus` in a
`handler_policy`, `respects bogus` and `intrinsic_height bogus` checked.
The names check returns the clauses it reads, and the same table judges
each. A length, which nothing judged anywhere, is a count and a unit CSS
Values 4 defines (`just e14-block-clauses`).

**ADR-0246, 2026-10-07: the feed's data in PostgreSQL, held to what its
source states** (the owner's parallel track, written on `pg-data-layer` and
integrated here). Where a deployment names a database
(`PW_FEED_DATABASE_URL`), the feed's data layer is PostgreSQL: each command
one serializable transaction, its events written to an outbox in the same
transaction and delivered once it commits (ADR-0208). The feed states what
its database guarantees (`source FeedData`), `pw build` writes it
(`sources.json`), and the host measures the database it opens and refuses
to serve on a shortfall. The in-memory layer stays the default. Against
PostgreSQL 18.6 here, and on CI in a service container
(`just e14-feed-postgres`).

**ADR-0245, 2026-10-07: the heavy verification runs on GitHub Actions** (the
owner's direction, relayed 2026-10-07). A `verify` workflow runs the
evidence recipes in parallel shards on Linux, and the browser suite in each
engine: a push, the recipes its commits touch, the chain's choice; each
night, all 208. Each recipe's output and evidence is an artifact, and `just
evidence-fetch <run>`, the recorded command, copies a run's evidence into
`docs/evidence/`, each file naming the run. The laptop keeps the fast check
before a commit.

**ADR-0244, 2026-10-07: Wasmtime 48.0.5, and a host that enables only what
it runs** (a correction). Master's CI had been red since 2026-10-05 on its
"licenses and advisories" job: three RustSec advisories against Wasmtime
48.0.3 (RUSTSEC-2026-0325 to 0327, one CVSS 9.3), and behind them, never
reached, three npm advisories. Nothing read CI's state. Every live pin is
48.0.5 now, the CLI's from GitHub's release digests, and the host's one
engine configuration turns off GC, exceptions and the component model's
async, which Wasmtime enables by default and no Pleris component uses: a
component using one is refused when it loads. Two npm packages moved inside
their ranges; `braces`, with no fixed release, is accepted with its reason.
This machine's CLI had been 47.0.3 since before ADR-0116.

**ADR-0243, 2026-10-07: a block's statements are separated, by `;` or a
line** (a correction, found with ADR-0237). `fn f(a: Int, b: Int) -> Int !{}
{ a b }` was two statements, `a` dropped, and checked. Two statements on one
line are PW0030 now, at the second. A first cut refused 171 lines of the
repository's programs, each meant: a block keeps a `return` and its value, a
clause's head and its value, and a block after a statement as statements on
one line, and its readers read each pair as one. The grammar knows each by
its shape, and the names check, which knows whether a word is a clause's
head where it is written, refuses one that is not: `key 1`, where `key` is a
binding. And a `;` ends a clause's value there, as in the grammar:
`scope component; nothing_here` read the name as `scope`'s value, unresolved.
Writing it found that nothing judges what a clause in a block holds: `scope
bogus` in a resource's block checks. That is next
(`just e14-statements-separated`).

**ADR-0242, 2026-10-07: an `{#each}`'s head is read once, by the grammar**
(a correction, found with ADR-0237). The head was kept as text, and five
places split it at ` as ` and `(`. So `{#each xs ys as x (x)}` and an
unclosed key checked, `{#each xs}` was refused for what it lacked downstream
and never for its missing `as`, and PW5011 took any `(` in the directive for
a key. The grammar parses the head now, padded to its place: a missing `as`
is PW0019, what stands before `as` or the key is one error and the rest is
read on, and every reader reads the head's parts
(`just e14-each-heads`).

**ADR-0241, 2026-10-07: an optimistic transition's value is the value
typer's** (a correction, found with ADR-0240). PW0331 read the transition's
type through the older typer, which has no answer for a literal, and no
answer was no violation: `optimistic Thing(x) as t => "no"`, where `Thing`
holds an `Int`, checked. The value typer, which types every expression,
relates each transition to its own target's value now, as PW0331; the older
comparison is retired (`just e14-transition-values`).

**ADR-0240, 2026-10-07: a clause is read once** (a correction, found with
ADR-0237). The resource graph split a key clause's text at its commas while
lowering read it with the grammar. A parenthesis inside a key's string ended
the key early, so `emits Searched(")"), Other(1)` had no edge to `Other`,
which nothing declares, and checked. The graph reads the keys lowering made
now, its labels unchanged, and PW5100 points at the key. An interface's
clause terms were lowered by nothing. They have an arena of their own now,
and every rule that reads a term reads them: an unbound listener key on a
query with no body is PW5104, as on one with a body
(`just e14-clauses-read-once`).

**ADR-0239, 2026-10-07: every code the compiler writes is registered,
once** (a correction, found with ADR-0237). The declaration rules wrote
PW0101 and PW0102, which the registry did not have, so their diagnostics had
no symbol; the parser wrote PW0102 too, for a `for` with no `in`. A reader's
value in a shared cache was two errors under two numbers: the declaration
rule's PW0100, an alias, and the label algebra's PW5001. PW0101 and PW0102
are registered, the parser's is PW0018, and the declaration rule is retired
for PW5001. The registry's tests read the compiler's own source both ways
now, as rustc's `tidy` does (`just e14-registered-codes`).

**ADR-0238, 2026-10-07: a command speculates on several entries, a page on
those it shows** (the rest of the owner's first Twitter gap). A command's
`optimistic` clause named one entry, and a page that called the command
without showing it was refused, so the thread page had no Like button. A
clause has an arm for each entry now, each typed against its own: `like`
speculates on the timeline and on the thread. A page speculates on the arms
whose targets it shows, and waits for the server on the rest. The thread
page shows a like before the server answers, in three engines
(`just e14-speculated-arms`).

**ADR-0237, 2026-10-07: what lowering parses, it reports** (a correction,
found writing ADR-0238). The grammar keeps a clause's value, a string's hole
and a block marker's expression as text, and lowering parsed each and dropped
the parse's errors. So `invalidates Cart(s) Order(s)` checked and invalidated
no order, `"sum {a b}"` rendered `a`, and `{#if flag other}` was decided by
`flag`. Each is refused now at its place, as PW0016 `read_whole` or PW0017
`optimistic_clause`, codes registered in the syntax range. A missing comma or
arrow is reported and read past, as rustc reads an omitted separator. An
`{#each}`'s head, read by splitting its text in five places, and two
statements on one line are their own ADRs (`just e14-read-whole`).

**ADR-0210's urgent defect 3 is not one** (verified 2026-10-07; it was
marked unverified). A page's handler calling a command whose body is `todo`
is refused at build: "`feed.app.Home` depends on `feed.app.like`, whose body
is a placeholder (`todo`)". `pw check` passes it: a placeholder is refused
once a build depends on it (ADR-0034).

**ADR-0236, 2026-10-07: a speculation on the entry a page's parameter keys**
(ruling 0122-d, the first of the owner's Twitter gaps). A target keyed by a
command's parameter, `optimistic Thread(to)`, matched no page: the thread page
binds `Thread(id)`. It matches now when every handler on the page passes the
page's parameter to that command parameter unchanged; any other flow is refused
by name. A speculated entry is named by the page's parameters its key reads, so
two open threads are two entries. The feed's thread page shows a reply before the
server answers, its count and "No replies yet." with it, and takes back one whose
request fails, in three engines: ADR-0233 to ADR-0235's browser tests
(`just e14-speculated-routes`).

**ADR-0235, 2026-10-07: what a speculated value computes, the page's module
computes** (ruling 0073-a, and a gap beside it). A block's subject computed
from a value a page speculates on was refused, and a value computed from one
inside a block was skipped: a count inside a block another query decides kept
the server's beside one a press changed, and nothing refused it. A block
whose subject is computed from the value is a region now, its subject and the
values inside it the module's; one no region renders is refused by name
(`just e14-speculated-values`).

**ADR-0234, 2026-10-06: a view's instance given a speculated value is
rendered again with it** (found building the thread page's optimistic reply).
A view that contains itself, given a value a page speculates on, was no region
of the speculation: a press would have changed the page's count and left the
thread as the server rendered it, and nothing refused it. Its instance is a
region now, rendered as a block is; one given what the browser does not hold,
or nested where no region renders it, is refused by name
(`just e14-speculated-instances`).

**ADR-0233, 2026-10-06: a speculation on a value of a type that contains
itself** (ADR-0205 §5, for a speculation's value). The server wrote each
value a page speculates on nested, knowing no type, and a page's module could
not decode one whose type contains itself, so a thread was never speculated
on. The server writes it by its query's declared type now, as its nodes,
which the module reads as a handler reads a signal's. A component's comment,
written so, is the graph a model writes, for 100 random trees
(`just e14-speculated-graphs`).

**ADR-0232, 2026-10-06: an opaque value's representation is read in a
template** (found by ADR-0231). `{p.id.value}` checked and built, then its
page was answered 503: the template read `.value` as a field of the text. A
template reads it by the opaque value's own path now, as the checker types
the base, in a page's body and in each view it composes; a record's field
named `value` is a field (`just e14-representations`).

**ADR-0231, 2026-10-05: a page's parameter is rendered on every page**
(found building the feed's replies). A page that binds a query was rendered
without its parameters: a title, an attribute or a handler's captures that
read one built, then every request for the page was answered 503, and a text
part that read one was refused at build. Each renders now, in the document
and in each block and row a host renders again. The feed replies: the thread
page's form passes the page's `id`, and the reply reaches every reader of
the thread, and no timeline, in three engines. Found: an opaque value's
representation, `{p.id.value}`, built and did not render (ADR-0232)
(`just e14-page-parameters`).

**ADR-0230, 2026-10-05: a condition is a `Bool`, or tested non-empty**
(ruling 0071-a). A number and a record were the renderer's truth and checked
clean: `{#if balance}` was true for a negative balance, and said neither
`> 0` nor `!= 0`, and a record was always true. A condition, or a boolean
attribute, is a `Bool`, or a `List` or `String` tested non-empty now; a
number is PW0609 with the repair `n > 0`, which ADR-0229 builds. No program
tested either. Corpus C17; generality is 44 / 44 (`just
e14-condition-truth`).

**ADR-0229, 2026-10-05: a computed condition decides its block** (ruling
0073-a for a block's subject; 0071-a's repair builds). `{#if !b}` and
`{:else if n > 0}` were refused at build. A subject is read by a path the
compiler names now: a host computes one from a query's value and renders its
block again when the value changes, and the browser one from a signal's,
rendering its block again as the signal changes. A value computed inside a
block a host renders is the host's. The feed's draft too long says "Too long
to post.", and a thread with no reply "No replies yet.", in three engines
(`just e14-computed-conditions`).

**ADR-0228, 2026-10-05: a value computed in a row is the row's** (ruling
0073-a in a row and from a speculated value). A value computed in a loop's
row was refused at build. Now its path is named from the row's item, a host
computes it for each row as a member read of the item, and the speculation
module for each row it renders. The feed's rows say "2 likes", and a like is
shown before the server answers, in three engines. Found: the feed's
`i.author`, a field, was charged `fn author`'s `database.read<User>`, and
`i.id == post`, two opaque values, did not build (`just e14-computed-rows`).

**ADR-0227, 2026-10-05: a value computed from a signal is the browser's**
(ruling 0073-a, the browser's part). A value computed from a signal was
refused at build. Now the host renders its first value, running the
component ADR-0226 lifts with the signal's first value, and the build
compiles the same function into the page's module, which the browser loads
when the signal first changes. The feed's draft says "280 left" as it is
typed, and its post button is disabled while there is no post to send, with
scripts off at their first values (`just e14-computed-signals`).

**ADR-0226, 2026-10-05: a value the template computes compiles** (ruling
0073-a, the host's part). A template read each value by path, and
`{counted(List.length(thread.replies), ..)}` or `disabled={..}` checked and
did not build. Now a text hole or an attribute's whole value may compute
from one query's value at the top of a page. The compiler lifts it into a
function, a component of its own, read by a path it names. The host runs it
when the page renders and again when the value changes, sending what
changed alone. It performs nothing (PW0334, revision 2). The feed's thread
page shows "1 reply · 2 likes", with scripts off too, and another session's
like reaches an open thread. Corpus C16; generality is 43 / 43. Found: a
member an imported module lacks, `String.nope(s)`, checked clean, and two
corpus files called one (`just e14-computed-holes`).

**ADR-0225, 2026-10-05: a `String`'s length is an invariant** (the feed's
length limit). An opaque type's invariant was bounds on an `Int`, and a
post's text could be empty or a book. `opaque type PostText = String where
String.length(value) >= 1 & String.length(value) <= 280` now: every
construction is shown to hold it at build, by literals, tests that narrow a
length, and bounded values; the contract states the measure; the host counts
code points, as the language does. In three engines a 281-character draft is
not sent and 280 emoji are (`just e14-string-invariants`).

**ADR-0224, 2026-10-05: a longer read is not applied over a commit it did
not see** (found by ADR-0222). "Load more" read the timeline outside the
session's hold and applied it after: a post committed in between was sent,
then undone by the longer list read before it. A keyed read applies in the
session's hold now, and reads again when a change reached its document while
it read, the last time inside the hold (`just e14-keyed-race`).

**ADR-0223, 2026-10-05: a streamed region is filled when its whole arm has
arrived** (found by an intermittent `slots.spec.mjs`). The runtime applied a
`<template for>` as soon as it saw one, and a response that arrived in parts
gave the region the part parsed so far: one recommendation of two, the rest
lost with the template. It waits for the comment the renderer writes after
each template now, as the platform waits for the end tag. The tests that
slept 2.5 s against a 3 s timeout hold the recommender instead (`just
e14-whole-fills`).

**ADR-0222, 2026-10-05: a post is shown before the server answers** (the
feed's optimistic posting). Speculation was the store's cart's: the server
sent no other value, and the feed's timeline has a key, its length, a post
cannot name. A target may leave a key unnamed (`_`, ruling 0105-a), and the
server sends any value a page speculates on, read with what it shows and
sent when it changes. A commit's answer names the version that includes it.
In three engines a held post shows first, by "You", and becomes the
server's; one that fails is taken back (`just e14-optimistic-posts`).

**ADR-0221, 2026-10-05: a form control's value is written where HTML reads
it** (found by the feed). `bind:value` on a `<textarea>` was written as a
`value` attribute, which a textarea does not have: a first value showed
nothing until a script ran, and nothing with scripts off. It is the
textarea's text now, escaped, a leading newline doubled. PW5036 refuses a
`<select>`'s value, which is the option it marks `selected`, and a
textarea's value no change would set again. Corpus C15; generality is 42 /
42 (`just e14-form-controls`).

**ADR-0220, 2026-10-05: the feed is served in browsers** (the feed, third
step). The first browser to open it was told to reload for ever. A stream's
request drained the session, and the drain regenerated the store's cart for
every program, on a page with no cart. A layer with no session entry drains
nothing now. Every program's page carried the store's menu style, and a
guest's post read "You" to every reader. The feed runs in three engines on
hosts of its own: posting, a post reaching another open reader, likes, a
thread, a 404 (`just e14-feed`).

**ADR-0219, 2026-10-05: what a commit drops reaches every session that
reads it** (the feed, second step). A commit told the session that made it,
and another reader's open timeline saw a post when the page was next
loaded. A commit's dropped queries, whole or by a key sessions share, are
its write set, and an open page's bindings its read set. Every other session
whose page reads one is read again and sent the change, in its own hold,
after the author is answered (`just e14-cross-session`).

**ADR-0218, 2026-10-05: a host serves any program, and its data is the
deployment's** (the feed reference app, first step). The development server
served the store alone, and a second program failed at `from_build`. The
split is made in place, in steps, every test green after each: the store's
state, its reads, a command's staging and its grants are `StoreData` now,
and a build that imports what the layer does not supply is refused at start.
Then the feed: a `DataLayer` the host chooses by the build's imports, the
feed's own in memory, and `examples/feed/app.pw` served and committing a
post to the session's open timeline.

**ADR-0217, 2026-10-05: what a handler at the top of the page captures is
set again when it changes** (urgent defect 2). A row's captures were patched
with the row (ADR-0172); a top-level handler's were in no plan. A button on
the cart's page capturing `cart` kept the cart the page was first rendered
with, after a commit and during a speculation, and a press sent that. The
page's plan lists such an element now, the server patches its captures, and
a speculation renders them in the browser (`just e14-top-captures`).

**ADR-0216, 2026-10-05: every clause belongs to a declaration that reads
it, and a code body admits none** (rulings 0092-b and 0047-a's interim;
urgent defect 5). PW5105 placed the graph's four heads, and `freshness` on a
command, `retry` on a function and `cache nothing_y` in a function's body
all checked, read by nothing. Every head has a place by design, and the
table fails closed; a policy word in a code body is a name (`just
e14-clause-heads`).

**ADR-0215, 2026-10-05: a query's `retry` reaches its runtime as declared**
(ruling 0089-b; urgent defect 4). A page's plan carried a query's attempts
alone, so `jitter = false` ran with jitter and `fixed` ran as exponential.
The plan carries whether the delays vary, and the cache honours it; `fixed`
left `retry`'s operators, since fixed delays keep clients that failed
together retrying together (`just e14-query-retry`).

**ADR-0212 to ADR-0214, 2026-10-05: three of ADR-0210's soundness
defects.**
- A `query` reads a query or a resource, and a `subscription` a subscription:
  `query helper(n)` over a `fn` checked (PW5108; `just e14-query-reads`).
- `List.maximum` gives +0 over −0, as IEEE's `maximum` does; it kept the
  first, and its test compared by value (`just e14-float-maximum`).
- An opaque type's module declares no member named `value`, which shadowed
  the representation: `LayoutSnapshot`'s accessor is `measured` (PW0626;
  `just e14-opaque-value`).

**ADR-0211, 2026-10-05: a path that leaves a loop's body leaves the
function, and owes its releases** (ruling 0045-a; ADR-0210's urgent defect
1). A `for` loop had no exits, so `let tx = Database.begin(); for s in
sessions { if s == "" { return Ok(()) } }; tx.commit()` checked with the
transaction left open on that `return`, as did a failing `?` in the body;
and the correct program, rolling back then returning, was refused. A `return`
or failing `?` in a pass now owes the release, and a release on a path that
leaves before the pass ends runs once. 3 mutants (`just e14-affine-loops`).

**ADR-0210, 2026-10-05: the owner's rulings on the pre-delegation marks,
ADR-0031 to ADR-0122**, relayed by the session "Web Pleris capabilities and
limitations" as ADR-0195's were: 85 marks, 26 overruled, 30 confirmed, 29
settled, and ten soundness defects its probes found. Each is built in its
own ADR, the defects first (NEXT 24).

**ADR-0209, 2026-10-05: a command computes the entries it invalidates**
(ADR-0195's ruling 11, completed). The server read each `invalidates` key's
text, `current_session()`, and dropped every entry of the query for anything
else: a feed's like would drop every post. Each query a command invalidates
is now a function of the platform's invalidations,
`pw:host/invalidations#store-page-cart`; the command calls it before its body
with the values it computed, and the server drops that entry once the writes
commit. The server evaluates no key of a command (`just
e14-command-invalidations`).

**ADR-0208, 2026-10-05: a command computes its events, and the outbox
commits them with its writes** (ADR-0195's ruling 11's other half). The
server computed an event's values from its key's text, `current_session()`
alone, and refused `emits ItemChanged(item)`. Each event is now a function
of the platform's outbox, `pw:host/outbox#cart-changed`; the command's
component calls it before its body with the values it computed, and the
host stages them and commits them with the writes, or not at all. Emitting
is the effect `outbox.write`, which a node grants where it keeps an outbox:
without it a command is refused, rather than its events lost. Found on the
way: an `Int` key reached a query's key as a `String` and missed its entry,
and an event carrying `""` reached every entry. **Correction:** the
committed component contracts were last emitted at ADR-0181, and nothing
compared them; the order's contracts were in none. They are compared now.
`invalidates` keys are
still read by the server, which is sound and next (`just
e14-command-events`).

**ADR-0207, 2026-10-05: a data source states what it guarantees, and
nothing asks it for more** (the owner's priority 21; ADR-0195's ruling 11).
`source X  holds A, B  transactions …  reads …  changes …`: the isolation a
command's writes commit with, what its reads may promise, and whether it
tells what changed. It cannot be inferred: one PostgreSQL is serializable and
another read committed, and a standalone MongoDB has no change streams. A
query asking a consistency its source does not read, a command writing two
sources, asking more isolation than its source gives, emitting events its
source commits in no transaction and tells no change of, or idempotent where
it commits in no transaction, is refused (PW0344-PW0349). A resource no
source holds is the host's SQLite database's, which gives each. The store
states its own, and declared as a search index is, it is refused 21 times.
Whether a deployment's database gives what its source states is not checked
yet. 17 mutants (`just e14-data-sources`).

**ADR-0206, 2026-10-05: a case has one name where values are rendered, its
WIT case's.** Correction, found building ADR-0205: the browser's wire, the
build and a handler name the language's four cases `some`, `none`, `ok`,
`err`, and the template and a host's values named them `Some`, `None`, `Ok`,
`Err`. So a page matching on an `Option` or a `Result` a signal holds found
no arm and did not render; no page here did that. Every case is named as its
WIT case is now. The committed kiokun build is rebuilt, and its currency
test compares every file `pw build` writes: it compared four kinds, so the
recipe that rebuilds it failed it. 5 mutants (`just e14-one-case-name`).

**ADR-0205, 2026-10-05: a value of a type that contains itself crosses the
browser's wire as its nodes** (ADR-0194's next step). `{ "$graph": [node,
...] }`, each value of the type inside a node `{ "$node": k }`, in ADR-0194's
level order, so the JSON is as shallow as the type and a component reads the
same indices. The build writes a signal's first value so; a handler reads a
signal's tree and writes one, 20,000 deep under Node, and sends one to a
command, which the host passes to the component as its nodes, 50,000 deep.
The renderer reads any graph, refusing one that is not a tree, and reads,
drops, clones and compares its values without recursion: 100,000 deep on
256 KiB.
What a host writes knowing no type, a capture, a command's error or a
speculation's value, stays refused by name. 19 mutants (`just
e14-graphs-on-the-wire`).

**ADR-0203, 2026-10-05: a view that contains itself is an instance of its
own template, made at run time** (ADR-0130, ruling 2). A reply thread's view
shows each reply as itself, as deep as its data: each use is an `instance`
part, rendered in a frame of its own, with addresses of its own. One with no
block on the way back to it, holding a signal or showing a stream, is
refused (PW5020). The
renderer writes each instance after the markup around it, not on its stack:
rendered one inside another, a chain of 247 overflowed a 4 MiB thread in a
debug build. It refuses a page nesting more than 500 elements, under the 512
Blink's and WebKit's parsers nest. The browser reads each instance in its
own template, binds its handlers in every instance, and renders a block
holding instances again; the host renders an instance a query gives again,
whole. `examples/demo/thread.pw` in three engines; 27 mutants (`just
e14-view-instances`).

**ADR-0204, 2026-10-05: an element holds only the children HTML permits, as
the page holds them.** Correction, found by ADR-0203's first test: PW5012
read only the elements written directly inside. `<ul><Thread /></ul>` was
refused though `Thread` renders an `<li>`, and a `<div>` row of an `{#each}`
in a `<ul>` passed; one of the compiler's own test fixtures had such rows. A
block's rows and branches, and what a view renders at its top, are read now.
8 mutants (`just e14-rendered-children`).

**Correction, 2026-10-05: a mutant survived at 7d3234b.**
`docs/evidence/E14/recursive-types.txt` recorded 17 of 18: "a list holds its
elements in place" changed `contains_itself_in_place`, which nothing had
called since ADR-0202 moved boxing into the encoder. It and the three
functions only it reached are removed, and the mutant retired with them
(`a2fc80f`).

**ADR-0202, 2026-10-05: a type that holds itself in place is boxed, and
crosses as its nodes.** ADR-0194 compiled a type that contains itself through
a list, and refused `next: Option<Node>` and `Add(Expr, Expr)` by name. Each
value of such a type is now the address of its cell: built, read and matched
through it by the encoder, with the IR unchanged. At a boundary a box is a
`u32` in its node and an option of one an `option<u32>`, in ADR-0194's level
order; a chain 50,000 deep crosses both ways, and malformed nodes trap. Two
types that hold each other in place, and a generic one, compile inside a
component. 17 mutants (`just e14-boxed-types`).

**ADR-0201, 2026-10-05: a case written alone is the case of the type
expected where it is written** (ADR-0195, ruling 5, its second half). Two
types with an `Empty` made every bare `Empty` PW0022, even in `fn f() ->
Shape { Empty }`. The type expected where a case is written now chooses, at
each position the program states one. The typer decides it once, owns
PW0022 for it, and the backend asks it; the name check and
`unresolved_uses` no longer decide it. 14 mutants (`just
e14-expected-cases`).

**ADR-0200, 2026-10-05: `()` is the unit value, and nothing the compiler
cannot read checks** (found building ADR-0199).
- **Correction:** `()` lowered to an error node, which the checker types as
  anything: `fn f() -> Int !{} { () }` checked, and the build refused it. It
  is typed `Unit` and built now (PW0606 for the `Int`).
- **Correction:** `check_sources`, which the tests call, reported no syntax
  error. It does what `pw check` does now, and four tests that had passed on
  programs that do not parse are corrected.
- **Correction:** an unclosed `{#if}` swallowed its element's close tag, so
  `pw check` reported the end of the file, twice. The block ends with its
  element, and PW5019 names it.
- An error node in a file that parses is refused (PW0015). A gate holds every
  `.pw` file in the repository to it. 8 mutants (`just e14-unit-value`).

**ADR-0199, 2026-10-05: a function or a command named as a handler is the
lambda that calls it** (ADR-0195, ruling 12). `on:submit={save}` was checked
against its event and refused when built. Now it is `(e) => save(e)`:
compiled as that lambda, with an identity from what the name resolves to,
and held to the event, its answer (PW0618), idempotency (PW0338) and what it
performs in the browser.
- **Corrections:** a handler named through a module, `on:press={other.rename}`,
  was never checked against its event; a local's value, a page, a type or a
  case as a handler checked, and only the build refused them. Each is
  refused when checked now (PW0602, PW0614).
- **Found beside it:** `()` as a value is an expression that did not parse,
  and nothing reported it: `fn f() -> Int !{} { () }` checks. ADR-0200.
- No program changes; the store's 66 artifacts are byte-identical. 17 mutants
  (`just e14-named-handlers`).

**ADR-0198, 2026-10-05: a bare case with a payload is the case of the one
type that has it** (ADR-0195, ruling 5, its first half). `Empty` alone was
typed and `Circle(3)` alone was PW0021. Now both resolve by one rule: the one
visible type with the case. A call's payload is checked against the case's
fields, the component and the JavaScript module build it, and several
candidate types are named (PW0022). Resolution from the expected type, the
ruling's other half, is not built yet. 4 mutants (`just e14-bare-cases`).

**ADR-0197, 2026-10-05: a pattern tells a case from a binding by its
capital** (ADR-0195, ruling 2). Six analyses each decided whether a bare
pattern name was a case, and no two agreed: a misspelt case, `Circel`, bound
a name and matched every value, as rustc's E0170 does.
- The parser decides once, by the first letter: an uppercase name is a case,
  and any other binds. Every later reader reads its decision.
- A capitalized name its type lacks is PW0608, and a case named in lowercase
  is PW0625.
- No program changes; the store's 66 artifacts are byte-identical.

Corpus C14: R-055, R-056 and A-030; **generality is 41 / 41**. 5 mutants
(`just e14-case-names`).

**ADR-0196, 2026-10-05: only a word that begins a statement or an
expression is reserved** (ADR-0195, ruling 3). `let return = n` checked, and
the binding's next use read as a `return`; `let match = n` was a parse error
naming the wrong invariant.
- PW0013 now refuses every such word as a binding: a `let`, a parameter, a
  loop's or a pattern's.
- A declaration may not be named by a word that begins an expression; a
  statement word stays allowed, since a call by one reads as a call.
- The repair suggests `match_`. Every other keyword names what a program
  likes.

6 mutants (`just e14-reserved-words`).

**ADR-0195, 2026-10-05: the owner's rulings on fifteen open questions.**
The owner ruled on fifteen "(ruling needed)" items, relayed by another
session. Each is recorded, checked against a primary source and confirmed;
`%` on a Float is refined by `rem_euclid`'s own contract. Each is built in
a later ADR of its own, in the order the record sets: the ones that fix a
wrong value first (an invented zero price), the rest with the app layer or
after it.

**ADR-0194, 2026-10-05: a type that contains itself compiles, and crosses
a boundary as its nodes.** This is the first item of the app layer the owner
put before the AI benchmark. A reply thread could be declared and checked,
and ADR-0059 refused it in the backend.
- Through a list, such a type is laid out by its type inside a component.
- At a boundary it crosses as its nodes, in level order: `list<node>`, each
  list of itself a `list<u32>` of indices. Neither side recurses on the
  value, so a chain 50,000 deep crosses both ways, and malformed nodes trap.
- A host holds the value nested, no deeper than 128.
- PW0624 refuses a type no finite value has.
- A type held in place, the browser's wire and views that contain themselves
  come next.

Corpus C13: R-054 and A-029; **generality is 39 / 39**. 21 mutants
(`just e14-recursive-types`).

**ADR-0193, 2026-10-04: an order is placed, and its page follows it.**
The store's flow ended at a cart.
- The cart's page places the cart as an order, in one commit with the emptied
  cart. An empty cart places nothing.
- The order's page, at `/order`, shows where the order is, in a live region.
- A change the store makes, which is no command, now reaches the session's
  open pages, against the order's own entry. Until now only a command on the
  cart reached one.

The store's program is a delivery's flow: the stores, a store, its cart,
the order, each kept current while it is open. 7 mutants
(`just e14-orders`).

**ADR-0192, 2026-10-04: the stores, as the home page.** A delivery site
starts with its stores, and the store's program had no list.
- `Stores.list()` is a host operation of the store's data layer, and
  `StoreList()` a public query over it.
- `HomePage`, at `/`, links each store to its page and counts the session's
  cart beside them, kept current. `/` was store 47's page, an alias from E7.

The store's program is three pages now, the home page, a store's and the
cart's, each linked to the next. 2 mutants (`just e14-home`).

**ADR-0191, 2026-10-04: every page speculates from its own module.** The
compiler wrote every page a speculation module, and the server read the
store's alone, so the cart's page showed a press only when the server
answered.
- Each page now carries its own module, and a change's speculated value is
  sent to each document whose page speculates on it.
- The cart's page moves a line's quantity before the server answers, and
  restores a refused press, in three engines.

4 mutants (`just e14-page-speculation`).

**ADR-0190, 2026-10-04: every page that binds a query is served at its
route** (E14-Q's next slice). A store like DoorDash's is several pages that
read queries. Only the store's page could: the server read its plan and
template wherever a page's values were read, and refused any other page that
bound a query.
- Each document records its page. It is read, rendered and kept current by
  its own plan and template, and a change is derived for each document of a
  session from its own plan.
- The menu's public fragment and the speculation stay the store's page's.
- The store gains a second page, its cart at `/cart`, and links to it. A
  press on either page reaches the other while both are open, in three
  engines.

9 mutants (`just e14-pages`). Not yet: public data other than the menu
kept current. A page other than the store's speculates since ADR-0191.

**ADR-0189, 2026-10-04: a `<link>` is written where HTML allows it.**
`<link rel="canonical">` and `rel="icon"` written in markup checked and
built. They went into the body, where HTML does not allow them and nothing
reads them.
- PW5035: a `<link>` in markup has only body-ok relations (`stylesheet`,
  `preload`, `modulepreload`, `prefetch`, `preconnect`, `dns-prefetch`,
  `pingback`), or is an item's property.
- The head stays the host's, but for a page's title and metadata.

Corpus C12: R-053 and A-028; **generality is 38 / 38**. 9 mutants
(`just e14-links`).

**Correction, ADR-0188, 2026-10-04: E7's gate item 7b had failed, unseen,
since ADR-0172 at the latest.** Its test bounds what the store's page
downloads to run, the runtime's script and the resume's WebAssembly, at
128 KiB. It ran only in `just e7-performance`, which had not run since
2026-08-07. E7 recorded 102,693 bytes; on 2026-10-04 there were 141,339, as
the script grew from 28,471 bytes to 84,822.
- Each file is now bounded as a static host sends it, compressed with Brotli
  at quality 11: 64 KiB for activation, which is 44,129 bytes today, and
  128 KiB for the renderer's WebAssembly, fetched to render a block again,
  which is 82,888. Uncompressed and gzip figures are reported beside them.
- The byte counts run in every browser suite, from
  `e2e/runtime-size.spec.mjs`. Controls grow the script and the renderer by
  bytes that do not compress, and each bound fails.
- E7's record is recorded again, the first time since 2026-08-07.
- For the owner: minifying the runtime would about halve the script as sent,
  and needs a minifier this session did not download.

**ADR-0187, 2026-10-04: nothing the store contains is on screen when its
page is first laid out.** Lighthouse on a phone measured a cumulative layout
shift of 0.136. Every menu item was contained (`content-visibility: auto`)
with a 42 px placeholder. Items are 125 px tall, so when the browser rendered
the three on screen, the cart moved 249 px. That happened before the first
paint, and was reported all the same.
- An item is contained when 28 items precede it in its list, or 16 lists
  precede its list: at least 2,400 px down. There is no shift in Chromium at
  a phone's, a desktop's or a 2,400 px-tall size, for the store and for
  menus of a thousand items or forty categories.
- The placeholder is 7.75em, within 5% of an item in three engines. With
  42 px a thousand-item page grew by 64% as it was scrolled.
- The style is in the head, where HTML puts it.

E7's gate item 10 holds: 972 of a thousand items are contained, and their
first layout takes 2.3 ms as served, against 15.2 ms uncontained. 6 mutants
(`just e14-stable-layout`).

**Found on the way: E7's gate item 7b had failed, unseen.** The bytes the
store's page downloads to run passed its 128 KiB bound after ADR-0152, and by
ADR-0172 at the latest. Its test runs only in `just e7-performance`, which had not run
since 2026-08-07. ADR-0188 is the ruling.

**ADR-0186, 2026-10-04: a page states its description.** Lighthouse
13.4.1 on the store's page gave SEO 75: it had no meta description.
- A page writes `<meta name="description" content={store.description} />`,
  or Open Graph's `<meta property="og:title" …>`, at the top of its view.
  Its host writes it into the head as the page is served. It is optional.
- PW5034 follows HTML:
  - one `description`, `color-scheme`, `application-name` and
    `theme-color`, compared ignoring case;
  - no `media` or `lang`, which the head would drop;
  - the host's charset and viewport refused.
- A `<meta itemprop>` is microdata, written in the body where it is.

Corpus C11: R-052 and A-027; generality 37 / 37. 34 mutants
(`just e14-metadata`).

**Correction to ADR-0183, the same day.** A static page that stated its
title shipped the browser runtime, against charter §14 M7 gate 2. It ships
none now.

**ADR-0185, 2026-10-04: ids, and the ARIA that names them, checked at
build** (charter §8.2). Until now these were the browser's to find, on a
page someone read (ADR-0182's audit).
- PW5031: an id names one element of its page, and none is written inside a
  loop.
- PW5032: a reference, from `aria-describedby` to `for`, names an element
  the page shows whenever the referrer is shown.
- PW5033: ARIA attributes, values and roles are WAI-ARIA's. `aria-labeledby`
  is refused, and `aria-labelledby` offered.

A field left unnamed by such a mistake is not reported again by PW5014.
Corpus C10: R-049 to R-051 and A-026; generality 36 / 36. 13 mutants
(`just e14-ids`).

**Correction, the same day.** An id written with holes, `id="line-{x}"`,
was taken as one that may be anything. So a declaration with one let every
reference to nothing through, and A-026 is one. It is read as its pattern
now.

**ADR-0184, 2026-10-04: what a cache may keep holds nothing of a
session's** (charter §15.6 tests 2 and 13, the audit's tenth and last gap).
Reading the running store's output found two ways one person's data could
reach the next through a cache:
- the store's page went out with no `Cache-Control`;
- a fresh session's cookie went on whatever it asked for first, a build's
  file included (RFC 9111 §7.3: a cookie does not stop a cache).

A session's response now says `private, no-store`, and a build's file names
no session. What the query runtime keeps for every reader, and the
materializer's public fragments, are tested the same whether or not two
sessions filled their carts, and name no session and no key. 6 mutants
(`just e14-shared-output`). Every gap of the audit of charter §15 is closed.

**ADR-0183, 2026-10-04: a page states its title** (WCAG 2.4.2, which
ADR-0182 left open).
- **The language.** `<title>{store.name}</title>` at the top of a page's view.
  The host writes it into the document's head, and a change to what it
  reads is set as `document.title`. Store 47's page is "Blue Bottle", where
  every store's page was "Store".
- **The rules.**
  - PW5029 refuses a page served at a route that states no title.
  - PW5030 refuses a title anywhere but once at the top of a page's view, as
    text and values.
  - A title that reads a signal, or a value a press speculates, is refused
    at build.
- **Corpus C9.** A-025, R-047 and R-048 are added, and four routed fixtures
  are given a title. **Generality is 33 / 33.**

16 mutants (`just e14-titles`).

**ADR-0182, 2026-10-04: keyboard and screen-reader semantics remain valid**
(charter §15.6 test 14, the audit's ninth gap).
- **The spec.** `e2e/accessibility.spec.mjs` reads the store in Chromium,
  Firefox and WebKit by axe's and WCAG 2.2 AA's rules, as served and after
  each kind of change. It also tests:
  - the keyboard's order and presses, and focus shown;
  - the whole accessibility tree;
  - live regions kept and each change said once;
  - reflow at 320 pixels, a phone's width, and reduced motion.
- **What it found, and what changed.**
  - One Add said "Items in cart: 1" up to four times. The runtime now writes
    a part only when what it shows changes.
  - No page had a viewport. Every page now does.
- **Left open.**
  - The store's title was "Store" for every store (WCAG 2.4.2): met by
    ADR-0183.
  - Installing axe-core is recommended to the owner.

14 mutants (`just e14-accessibility`).

**ADR-0181, 2026-10-04: a menu is grouped by its category, and a list inside
a row is changed where it is** (charter §15.1, the end of the audit's eighth
gap). `MenuItem` declares its `store_id` and its `category`, and the store's
`Menu` answers `MenuSection`s, each a heading and its items on the page. To
get there:
- the plan reads a loop inside a loop (`menu.*.items`);
- the renderer derives a keyed list's change, a list inside a row diffed
  where it is, which the server did for a list at the top of a page alone;
- every menu change is derived from the menu's values, E7-P's operations
  included, whose items keep their nodes inside their category.

A menu's version and a cart's identity stay the runtime's, ruled rather than
added as second fields. 7 mutants (`just e14-menu-categories`).

**ADR-0180, 2026-10-04: a delivery estimate is a range, and says when it
was made** (charter §15.1). `DeliveryEstimate { min_minutes: PositiveInt,
max_minutes: PositiveInt, generated_at: Instant }`, and the platform's
`clock` declares `Instant`. The slot says "Delivery in 25 to 35 min", in
words, since screen readers read an en dash in a range unreliably. An
estimator's answer of 0 minutes is refused by the host (ADR-0179), and the
slot says the estimate is unavailable. 6 mutants (`just
e14-estimate-range`).

**ADR-0179, 2026-10-04: an opaque type states its invariant, and every
construction and every boundary holds it** (charter §15.1's `PositiveInt`,
§7.1's "explicit decoding at every external boundary").
`opaque type PositiveInt = Int where value >= 1`.
- PW0622 refuses a construction the build cannot show holds it. The value
  analysis bounds the `Int` it is given, and a test narrows it.
- The contract states where each boundary must hold it: the host checks a
  command's arguments as it decodes them, and a data layer's answer before
  the component reads it.
- `fewer` answers an `Option`: one fewer than one is none.

25 mutants (`just e14-invariants`).

**Correction, ADR-0179, 2026-10-04: a forged quantity reached the cart**
([ADR-0179](DECISIONS/ADR-0179-an-opaque-type-states-its-invariant.md)).
Measured through the development server's command path: a request with a
quantity of 0 committed a line of `espresso × 0`, and one with −3 left
`espresso × −3`. Now each is refused before the command runs, by name.

**ADR-0178, 2026-10-04: whether an item can be ordered is shown before the
press, and a change to it reaches every page open** (charter §15.1 and
§15.2, the first part of the audit's eighth gap). `MenuItem` declares
`available`, which the data layer fills with what the command reads. A
sold-out row says "Sold out" and has no Add. A stock change is
`InventoryChanged(store, item)`, which the store's `Menu` query now hears,
and `/bench/stock?tell=true` tells every page open, which renders that row
again where it is. 14 mutants (`just e14-availability`).

**Correction, ADR-0178, 2026-10-04: a new version of the shared menu could
leave the pages open a version behind**
([ADR-0178](DECISIONS/ADR-0178-whether-an-item-can-be-ordered-is-shown-before-the-press.md)).
- ADR-0150 rendered the menu's fragment again for a document whose menu
  read differed, and told no page open. Later changes were derived from the
  new version, which those pages did not have.
- Nothing changed the menu without an event until availability did. A page
  open across an untold sale then kept the item's Add, even through a later
  told change of it.
- Now every new version reaches every page that shows it, as the whole
  difference from what it showed.

Also found: with the Add inside a block, a rename would have rendered the
row again. The renderer now looks into a block that decides as it did. And
the value analysis decided nothing of a listener's `_`, which the store's
`Menu` now writes.

**ADR-0177, 2026-10-04: a public read whose origin fails is answered with
the last value kept** (charter §15.6 test 18, the audit's seventh gap).
`fallback last_known_good` was checked for its value and read by nothing,
so the store's page answered 503 when its store's origin failed, though the
last store was kept. Now PW0343 keeps the fallback to public data, the query
runtime answers a public read whose origin failed with its last value, and
the store's `Store` and `Menu` declare it. `/bench/store?fail=next` fails
the store's origin once. 11 mutants (`just e14-last-known-good`).

**Correction, ADR-0176, 2026-10-04: a materializer failure left a page a
line short, silently**
([ADR-0176](DECISIONS/ADR-0176-a-regeneration-that-fails-sends-nothing-and-is-tried-again.md)).
Measured with this ruling's control: after a failed regeneration, two
presses showed one line, with no error and no reload. Three causes:
- the failed regeneration's frames went out at a version that had not
  moved, which the page ignores while the server took it to show them;
- the commit's answer named that version, so the page dropped its line;
- the stale entry was never tried again.

Now a failed regeneration sends nothing, its entry is tried again at the
session's next drain (which a page's subscription request makes), and the
answer names a version a later regeneration passes. `/bench/materializer
?fail=next` is §15.5's last control. 6 mutants
(`just e14-materializer-failure`).

**ADR-0175, 2026-10-04: a network error and a forced reconnect, made by the
server** (charter §15.5, part of the audit's sixth gap).
`/bench/drop?next=command&at=before|after` closes the session's next command
connection with no answer; `/bench/reconnect?for=ms` ends its subscriptions
and refuses new ones for a while. A press survives either drop as one
mutation, and a page cut off hears what changed meanwhile. 10 mutants
(`just e14-connection-faults`).

**Correction, ADR-0174, 2026-10-04: the one test of a rolled-back command
could not fail**
([ADR-0174](DECISIONS/ADR-0174-a-store-delay-a-cart-delay-and-a-one-shot-database-error.md)).
`e2e/resource-path.spec.mjs`'s "a rolled-back command produces no browser
update" posted `/command/add_and_fail` through a request context that is
another session, so the page could not have heard its change whatever the
server did. Since ADR-0172 that add also failed on its arguments. The test
now arms the page's own session (`/bench/fail?next=write`), and the page's
own press meets the failure. Also: every document read its cart twice, once
in a drain that changed nothing.

**ADR-0174, 2026-10-04: a store delay, a cart delay, and a one-shot database
error** (charter §15.5, part of the audit's sixth gap).
`/bench/store?delay=` slows every reader's store; `/bench/cart?delay=` one
session's cart; `/bench/fail?next=write|read` fails the session's next cart
write or read once. 10 mutants (`just e14-test-controls`); the controls'
tests pass in three engines.

**Correction, ADR-0173, 2026-10-04: PW0312 let `transport_only` retry a
command that is not idempotent**
([ADR-0173](DECISIONS/ADR-0173-a-command-is-sent-again-where-no-answer-came.md)).
It refused a retried mutation without `idempotent_by` except under
`transport_only`, as though a transport failure said the request never
arrived. A browser's `fetch` fails alike whether the request never left or
its answer was lost after the server committed. A command that is retried
is idempotent now, whatever it retries on; R-014's expected message already
said so. `retry none`, which PW0312 also refused, is not a retry.

**ADR-0173, 2026-10-04: a command is sent again where no answer came** (the
audit's fifth gap, §15.4). The compiled handler passes the command's `retry`
clause where it sends it, and the runtime sends the request again with the
same interaction, as many times as the clause allows; the host answers it
with the first's outcome (ADR-0121). An answer of any kind is never sent
again. The store's commands declare `retry transport_only(max = 2, jitter =
true)`. 10 mutants (`just e14-command-retry`); the retry tests pass in three
engines.

**Correction, ADR-0172, 2026-10-04: a press could be shown twice, and a
session's changes could cross**
([ADR-0172](DECISIONS/ADR-0172-the-cart-lists-its-lines.md)).
- **What was wrong.**
  - **A speculation could be shown twice** (ADR-0122's, since 2026-10-02).
    When a commit's new value reached the page before the command's answer,
    the page held the press and still showed it pending: only the answer
    could drop it. The count read 3 where 2 was right until the answer came.
  - **Two changes of one session could be sent at once.** A command's
    commit, a second command or a document being served could each derive
    patches against the same shown document. One patch set then addressed a
    document the other had already changed, and the page lost a line.
- **Now** the value a page is sent names the presses it includes
  (`applied`), and the page drops them; and a session's changes come one at
  a time. Found by the per-line cart's browser tests, and pinned by two that
  hold an answer back after the server has committed it.

**ADR-0172, 2026-10-04: the cart lists its lines, and a speculation reaches
every part that reads it** (the audit's fourth gap, §15.3).
- Each line shows its name, its quantity between − and +, its total, and
  Remove; the cart shows its subtotal, an empty message, and a fees note.
  `increase_in_cart`, `decrease_in_cart` and `remove_from_cart` are
  optimistic and idempotent, as `add_to_cart` is. A line records its item's
  name and price when it is made.
- `add_to_cart` takes the item the page showed, so a new line has its name
  and price before the server answers. A browser sends records and lists;
  a host reads them by the artifact's types, and the data layer records the
  store's own price, never the request's.
- The browser renders again each attribute, block and loop at the top of the
  page that reads a speculated value, with the server's renderer compiled to
  WebAssembly. A kept row's parts are set where they are, so focus stays; a
  row that goes passes focus to the next line's control, or to the cart's
  heading. A region that reads what the browser does not hold is refused.
- 41 mutants (`just e14-cart-lines`). The cart's 7 browser tests pass in
  Chromium, Firefox and WebKit.

**Correction, ADR-0171, 2026-10-03: an attribute at the top of a page kept
its first value**
([ADR-0171](DECISIONS/ADR-0171-an-attribute-that-reads-a-query-is-set-again.md)).
A host set a text part, a list's rows and a block again when a query's value
changed, and no attribute at the top of a page: `hidden={cart.lines}` stayed
as rendered. The plan now names each one, and a host sets it as the page's
render writes it. 6 mutants (`just e14-query-attributes`).

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

`4310769` (2026-10-07, ADR-0232 to ADR-0234, and the two survivors at
`c7f999b` accounted for): `just ci` passes locally, the workspace's 2110
tests pass, and the browser suite passes 773 in three engines, with 13
skipped. Recorded at that commit, each new: `representations.txt` (ADR-0232)
4 of 4 mutants killed, `speculated-graphs.txt` (ADR-0233) 8 of 8 and
`speculated-instances.txt` (ADR-0234) 3 of 3. **The survivors are killed**:
`computed-holes.txt` 23 of 23, its duplicate retired, and
`E10/template-operands.txt` 11 of 11. Recorded again, every script with a
mutant within 30 lines of what these changed, all killed: `cart-lines.txt`
41, `command-answers.txt` 16, `command-events.txt` 4,
`computed-conditions.txt` 12, `computed-rows.txt` 12, `events.txt` 12,
`form-controls.txt` 12, `graphs-on-the-wire.txt` 19, `handler-failures.txt`
6 and 7, `E10/handlers-compute.txt` 10, `handlers-by-file.txt` 2,
`handlers-resumable.txt` 7, `metadata.txt` 34, `nested-lists.txt` 6,
`not-found.txt` 17, `optimistic-posts.txt` 11, `optimistic.txt` 6,
`pages.txt` 7, `patch-set.txt` 14, `provide.txt` 29, `query-attributes.txt`
6, `row-reads.txt` 21, `signals-render-again.txt` 8, `streams.txt` 26,
`E10/strings.txt` 16, `E10/templates.txt` 16, `E10/template-values.txt` 11,
`titles.txt` 16, `top-captures.txt` 5, `view-instances.txt` 27 and
`views-compose.txt` 17. Two of `view-instances.txt`'s are killed by not
building, as at `2cbd5d8`: they control nothing, and are to be written so a
test kills them.
`c7f999b` (2026-10-06, ADR-0227 to ADR-0231): `just ci` passes locally, the
workspace's 2099 tests pass, and the browser suite passes 773 in three
engines, with 13 skipped. Recorded at that commit, in `docs/evidence/E14/`,
each new: `computed-signals.txt` (ADR-0227) 16 of 16 mutants killed,
`computed-rows.txt` (ADR-0228) 12 of 12, `computed-conditions.txt`
(ADR-0229) 12 of 12, `condition-truth.txt` (ADR-0230) 6 of 6 and
`page-parameters.txt` (ADR-0231) 8 of 8. Recorded again, their scripts
re-anchored: `row-reads.txt` 21 of 21, `cart-lines.txt` 41 of 41,
`optimistic.txt` 6 of 6, `provide.txt` 29 of 29, `query-blocks.txt` 9 of 9,
`signals-render-again.txt` 8 of 8, `pages.txt` 7 of 7 and `feed.txt` 5 of 5.
**And two survivors, which the ADRs' own runs missed, since each re-ran only
the mutants it re-anchored:**
- **`computed-holes.txt` 23 of 24: "a host computes a value in a block"
  survives**, likely since ADR-0229 made such a value the host's;
- **`E10/template-operands.txt` 10 of 11: "a sum type has a truth"
  survives**, likely since ADR-0230 rewrote a condition's truth.

Under investigation; each fix records its file again.
`4e198b9` (2026-10-06, ADR-0221 to ADR-0226): `just ci` passes locally, the
workspace's 2075 tests pass, and the browser suite passes 761 in three
engines, with 13 skipped. Recorded at that commit, in `docs/evidence/E14/`,
each new: `form-controls.txt` (ADR-0221) 12 of 12 mutants killed,
`optimistic-posts.txt` (ADR-0222) 11 of 11, `whole-fills.txt` (ADR-0223) 3
of 3, `keyed-race.txt` (ADR-0224) 1 of 1, `string-invariants.txt`
(ADR-0225) 12 of 12 and `computed-holes.txt` (ADR-0226) 26 of 26. Recorded
again, their scripts re-anchored or their tests changed: `bind.txt` 14 of
14, `row-reads.txt` 23 of 23, `patch-set.txt` 14 of 14, `invariants.txt` 25
of 25, `slots.txt` 7 of 7, `feed.txt` 5 of 5, `cross-session.txt` 8 of 8 and
`E10/template-values.txt` 11 of 11.
`1a00c28` (2026-10-05, ADR-0218 to ADR-0220): `just ci` passes locally, the
workspace's 2045 tests pass, and the browser suite passes 737 in three
engines, with 13 skipped. Recorded at that commit, in `docs/evidence/E14/`:
`cross-session.txt` (ADR-0219) 8 of 8 mutants killed and `feed.txt`
(ADR-0220) 5 of 5, both new. `row-reads.txt` 23 of 23: the survivor at
`ebed28c` is killed by the test of a member read in a row of a list no query
gives. Recorded again, the host serving any program since ADR-0218:
`command-invalidations.txt` 4 of 4, `metadata.txt` 34 of 34,
`stable-layout.txt` 6 of 6, `availability.txt` 13 of 13, `cart-lines.txt`
41 of 41, `command-answers.txt` 16 of 16, `estimate-range.txt` 6 of 6,
`home.txt` 2 of 2, `last-known-good.txt` 11 of 11, `menu-categories.txt` 7
of 7, `orders.txt` 7 of 7, `query-values.txt` 7 of 7, `slots.txt` 7 of 7,
`test-controls.txt` 10 of 10 and `accessibility.txt` 14 of 14.
`ebed28c` (2026-10-05, ADR-0217): `just ci` passes locally, the workspace's
2035 tests pass, and the browser suite passes 728 in three engines, with 13
skipped. Recorded at that commit, in `docs/evidence/E14/`: `top-captures.txt`
(ADR-0217) 5 of 5 mutants killed, and `query-attributes.txt` 6 of 6. **And
`row-reads.txt` 22 of 23: "a member read in text no host computes passes
silently" survives**, killed before ADR-0217. Under investigation; its fix
records the file again.
`292d193` (2026-10-05, ADR-0215 and ADR-0216): `just ci` passes locally, the
workspace's 2031 tests pass, and the browser suite passes 728 in three
engines, with 13 skipped. Recorded at that commit, in `docs/evidence/E14/`:
`query-retry.txt` (ADR-0215) 4 of 4 mutants killed and `clause-heads.txt`
(ADR-0216) 5 of 5; and again, their scripts changed with them,
`command-retry.txt` 9 of 9 (`fixed` retired), `E10/clause-places.txt` 5 of
5, and `E10/handlers.txt`, the handlers emitted again without `backoff`.
`a518902` (2026-10-05, ADR-0210 to ADR-0214): `just ci` passes locally, the
workspace's 2025 tests pass, and the browser suite passes 728 in three
engines, with 13 skipped. Recorded at that commit, in `docs/evidence/E14/`:
`affine-loops.txt` (ADR-0211) 3 of 3 mutants killed, `query-reads.txt`
(ADR-0212) 3 of 3, `float-maximum.txt` (ADR-0213) 2 of 2, `opaque-value.txt`
(ADR-0214) 2 of 2; and again, their scripts re-anchored, `E10/affine.txt` 7
of 7 and `E10/slices.txt` 10 of 10.
`feea53b` (2026-10-05, ADR-0209): `just ci` passes locally, the workspace's
2015 tests pass, and the browser suite passes 728 in three engines, with 13
skipped. ADR-0209's evidence is `docs/evidence/E14/command-invalidations.txt`,
recorded at that commit: the command's component, the store's commands
against their references, the server's drop, the store's artifacts as the
compiler emits them, and 4 of 4 mutants killed. Recorded again at that
commit, the server's command path having changed: `command-events.txt` 4 of
4, `E10/committed-events.txt` 4 of 4, and `menu-changed.txt` 5 of 5, its
anchor moved where the menu's events reach the queries.
`63ce91d` (2026-10-05, ADR-0207 and ADR-0208): `just ci` passes locally, the
workspace's 2012 tests pass, and the browser suite passes 728 in three
engines, with 13 skipped. ADR-0207's evidence is
`docs/evidence/E14/data-sources.txt`, recorded at that commit: its 9 tests,
the store checked with its source, and 17 of 17 mutants killed. ADR-0208's
is `command-events.txt`, at that commit: the command's component, the
outbox's values, the server's 3 tests, the store's artifacts as the compiler
emits them, and 4 of 4 mutants killed; `E10/committed-events.txt` records
the same controls, ADR-0104's four of the server's evaluation retired.
**Correction:** at `558e35b` one of them survived, `CartChanged` committed
whatever the command computed; ADR-0208 had deleted the half of ADR-0104's
test that killed it. `63ce91d` restores it, building the store with
`add_to_cart`'s `emits` removed.
`2cbd5d8` (2026-10-05, ADR-0205 and ADR-0206): `just ci` passes locally, the
workspace's 1997 tests pass, and the browser suite passes 728 in three
engines, with 13 skipped. ADR-0205's evidence is
`docs/evidence/E14/graphs-on-the-wire.txt`, recorded at that commit: its
tests, the renderer's and the browser's, and 19 of 19 mutants killed.
ADR-0206's is `one-case-name.txt`, at that commit: its 2 tests, the
template's, the renderer's and the hosts', and 5 of 5 mutants killed.
Recorded again at that commit: `recursive-types.txt` 16 of 16, its refusal's
control retired with the refusal ADR-0205 lifted; `view-instances.txt` 27 of
27; `streams.txt` 26 of 26; `E10/template-sum-types.txt` 10 of 10; and
`E10/kiokun.txt`, its build rebuilt with the cases named as WIT names them.
`cabf851` (2026-10-05, ADR-0203 and ADR-0204): `just ci` passes locally, the
workspace's 1979 tests pass, and the browser suite passes 725 in three
engines, with 13 skipped. ADR-0203's evidence is
`docs/evidence/E14/view-instances.txt`, recorded at that commit: the
compiler's 9 tests, the renderer's and the in-browser renderer's, the thread
page's 5 tests in three engines, and 27 of 27 mutants killed. ADR-0204's is
`rendered-children.txt`, at that commit: its 5 tests, the corpus's, and 8 of
8 mutants killed. Recorded again at that commit: `recursive-types.txt` 17 of
17, its survivor at `7d3234b` retired with the code it changed; and, their
mutants re-anchored where the runtime's binding and the plan moved,
`query-blocks.txt` 9 of 9, `command-answers.txt` 16 of 16, `keyed-reads.txt`
10 of 10 and 1 of 1, `provide.txt` 29 of 29 and `runtime-recovery.txt` 3 of
3.
`7d3234b` (2026-10-05, ADR-0202): `just ci` passes locally, the workspace's
1960 tests pass, and the browser suite passes 710 in three engines, with 13
skipped. ADR-0202's evidence is `docs/evidence/E14/boxed-types.txt`,
recorded at that commit: the WIT's, the refusals' and the lowering's 7
tests, the components' 7 through the E8 host against a Rust model and
ADR-0194's 10, five queries over values held in place agreeing with their
JavaScript modules on 1,000 calls, and 17 of 17 mutants killed. Recorded
again at that commit: `E10/generics.txt` 9 of 9, and `E10/sum-types.txt` 17
of 17, its control retired with the branch it controlled (`a69a0cf`).
**Correction:** `E14/recursive-types.txt`, recorded again at that commit,
killed 17 of 18: "a list holds its elements in place" mutates
`contains_itself_in_place`, which nothing has called since ADR-0202 moved
boxing into the encoder. It is not committed here; the function goes, the
control is retired ("Remove the in-place check nothing has called since
ADR-0202"), and the file is recorded with ADR-0203.
`53ceb20` (2026-10-05, ADR-0201): `just ci` passes locally, the workspace's
1952 tests pass, and the browser suite passes 710 in three engines, with 13
skipped. ADR-0201's evidence is `docs/evidence/E14/expected-cases.txt`,
recorded at that commit: its 12 tests, the components against a model and
the JavaScript modules agreeing on 600 calls, and 14 of 14 mutants killed.
`bare-cases.txt` is recorded again at that commit, 3 of 3, one control
retired. **Correction:** `E10/sum-types.txt`, recorded again at that commit,
killed 17 of 18: "a bare case with a payload is not told its qualified form"
mutates a branch no program reaches since ADR-0198, which resolves a bare
call's owners before it. It is not committed here; the branch goes, the
control is retired, and the file is recorded with ADR-0202. ADR-0200's is at
`2578341`, where `just ci` passed and the workspace's 1949 tests did: its
evidence is `docs/evidence/E14/unit-value.txt`, recorded
at that commit: its 6 tests, every `.pw` file in the repository lowering
with no error node, the parser's test of a block left open and the
checker's, and 8 of 8 mutants killed. `handlers-resumable.txt` is recorded
at that commit too, 7 of 7: `42fdd5e` tests the build's refusal of an event
part with no code, which ADR-0199 had left untested. ADR-0199's is at
`afb63a3`, where `just ci` passed and the workspace's 1940 tests did:
ADR-0198's evidence is
`docs/evidence/E14/bare-cases.txt`, recorded at that commit: its 13 tests,
the components' 13, the JavaScript modules agreeing with their components on
87 queries and 17,400 calls, and 4 of 4 mutants killed. ADR-0199's is
`named-handlers.txt`, at that commit: its 10 tests, the browser's and the
resumable handlers' 6, and 17 of 17 mutants killed. **Correction:**
`handlers-resumable.txt`, recorded again at that commit since ADR-0199
re-anchored a mutant, killed 6 of 7. ADR-0199 had turned the test of the
build's refusal of an event part with no code into a test of the check, and
the refusal was left untested; the next commit tests it, and the file is
recorded there. ADR-0197's is at `6985f81`, where `just ci` passed and the
workspace's 1928 tests did: ADR-0196's evidence is
`docs/evidence/E14/reserved-words.txt`, recorded at that commit: the parser's
test of every reserved word and its repair, every program checking as
before, and 6 of 6 mutants killed. ADR-0197's is `case-names.txt`, at that
commit: its 6 tests and the sum types' 11, the compiled patterns, the corpus
at C14, and 5 of 5 mutants killed. Recorded again at that commit, as their
mutants were re-anchored on lines ADR-0197 moved: `E10/match.txt` 14 of 14,
`E10/keys-cover-reads.txt` 4 of 4, `E10/handler-captures.txt` 6 of 6,
`E10/built-pages.txt` 3 of 3, `E10/template-sum-types.txt` 10 of 10,
`E10/patterns.txt` 11 of 11, `E10/kiokun-mutants.txt` 11 of 11 and
`E10/sum-types.txt` 19 of 19. ADR-0194's is at `712546f`, where `just ci`
passed and the workspace's 1922 tests did: its evidence is
`docs/evidence/E14/recursive-types.txt`, recorded at that commit: the
checker's, the WIT's and the lowering's 7 tests, the components' 10 through
the E8 host, five queries over trees agreeing with their JavaScript modules
on 1,000 calls, the corpus at C13, and 21 of 21 mutants killed. Recorded
again at that commit, as their mutants were re-anchored or retired:
`descriptions.txt` 8 of 8, `E10/wit-names.txt` 5 of 5 and
`E10/sum-types.txt` 19 of 19. ADR-0195 records rulings and has no evidence
of its own. ADR-0193's is `orders.txt`, at `9e53158`: an order placed from
the cart and followed on its page as the store moves it along, in three
engines with its audit, and 7 of 7 mutants killed, and `accessibility.txt`
recorded again at that commit, 14 of 14. ADR-0192's is `home.txt`, at `ee7bede`: the home page at `/`, in three engines with its audit, and 2 of 2
mutants killed. Recorded again at that commit, as their specs or tests
changed: `stable-layout.txt` 6 of 6, `titles.txt` 16 of 16 and
`accessibility.txt` 14 of 14. ADR-0191's is `page-speculation.txt`, at
`4f19028`: the cart's page shows a press before the server answers, in
three engines, and 4 of 4 mutants are killed. `pages.txt` is recorded again
at that commit, 7 of 7, its two speculation mutants now ADR-0191's, and
`titles.txt`, 16 of 16. ADR-0190's evidence was `pages.txt` at `f2060c6`,
with 9 of 9 mutants killed; and recorded again at that commit, as their
mutants were re-anchored on lines ADR-0190 moved: `accessibility.txt` 14 of
14, `titles.txt` 16 of 16, `metadata.txt` 34 of 34, `document-reads.txt` 4
of 4, `not-found.txt` 17 of 17, `query-values.txt` 7 of 7, `slots.txt` 7 of
7 and `store-signals.txt` 2 of 2. ADR-0189's is `links.txt`, at `699900d`:
9 of 9 mutants killed and the corpus at C12. ADR-0188's is
`runtime-size.txt`, at `673ae11`: the sizes as served and as sent, and 2 of
2 controls failing their bounds. E7's record,
`docs/evidence/E7/performance.txt`, is recorded again at that commit, the
first time since 2026-08-07: gate items 7 to 10 pass. ADR-0187's is
`stable-layout.txt`, at `bdb3330`: the page in three engines, E7's gate item
10 alone in Chromium, and 6 of 6 mutants killed. ADR-0186's `metadata.txt` and ADR-0182's `accessibility.txt` are
recorded again at that commit, their mutants re-anchored on the moved
style: 34 of 34 and 14 of 14 killed. ADR-0183's `titles.txt` is at
`a8b95af`, 16 of 16 killed. ADR-0185's is `ids.txt`, at `f19ce5a`,
with the corpus at C10; ADR-0184's is `shared-output.txt`, at `3e35870`;
ADR-0182's is `accessibility.txt`, at
`bbb4a1b`; ADR-0181's is `menu-categories.txt`, at
`bb63cb5`; ADR-0180's is `estimate-range.txt`,
at `6ef776a`; ADR-0179's `invariants.txt`, at `30f5b45`; ADR-0178's `availability.txt`, at `344f331`; ADR-0177's `last-known-good.txt`, at `257b1a5`; ADR-0176's
`materializer-failure.txt`, at `2c5366e`;
ADR-0175's `connection-faults.txt`, at `bbaf6e9`; ADR-0174's
`test-controls.txt`, at `d22928f`; ADR-0173's
`command-retry.txt`, at `99ddad2`; ADR-0172's `cart-lines.txt`, at
`a7314bc`; ADR-0171's `query-attributes.txt`, at `f8bd455`; ADR-0170's
`nested-lists.txt`, at `201ca0e`; and ADR-0169's `row-reads.txt`, at
`36d263c`.

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

1. **Accessibility** (test 14, the audit's ninth gap): an automated audit of
   the store's page in three engines.
2. **Tests 2 and 13** (the audit's tenth gap): the running store's shared
   output holds no session or secret field.
3. **E14-E's design**, ready for the owner's choice of models and budget.

Owner decisions before E14-E (agent runs): which models, the budget, and how
Pleris is taught to an agent (`docs/milestones/E14.md`).

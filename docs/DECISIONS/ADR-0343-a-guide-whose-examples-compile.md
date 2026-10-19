# ADR-0343: A guide whose examples compile

- Status: accepted under the owner's delegation of 2026-10-02, with the
  integrator's rulings of 2026-10-15 (below); merged on train-9 (track
  `guide`, from `49967631`).
- Date: 2026-10-15.
- Charter: §1 item 17 ("AI-friendly development: a small regular language,
  deterministic compiler feedback ..."), §3.3 (never hide uncertainty).

## Context

Pleris has no guide. What a developer from outside, or an AI agent handed
the docs, can read today is 334 ADRs and the builders' status documents:
evidence written for the people building the compiler, not a way to learn
the language. The owner's queue names "developer experience: docs and a
tutorial for a developer from outside; editor support; `pw new`".

A guide's examples are the part of it that rots first. The language changed
under every one of the 334 ADRs, and a page of examples nobody compiles
would teach syntax the compiler stopped accepting, which is worse than no
guide: a newcomer, and an agent above all, copies an example exactly. So the
guide is written with a mechanism that makes a wrong example a red build.

## Decision

### 1. The guide lives in `docs/guide/`, and its Pleris is checked

`docs/guide/*.md` is the guide. Every fenced code block in it says what it
is in its info string, and a test reads every one:

| info string | what it is | checked |
|---|---|---|
| `pw` | a whole program in one file | `pw check` reports nothing, and `pw fmt --check` passes |
| `pw fails=PW0606` | a whole program the compiler refuses | `pw check` exits 1, and the codes it reports are exactly the ones named (comma-separated) |
| `pw file=guestbook/app.pw` | part of the page's program | the page's `file=` blocks, each file's in order, are one program: checked clean and formatted |
| `text pw-check` | what `pw check --plain` prints for the `pw fails=` block just before it | printed exactly, less the closing `pw check: N file(s), ..` line |
| `pw gap=build` | a program `pw check` passes and `pw build` refuses: a gap in the checks the page says so about (§5) | checks clean and is formatted, and `pw build` refuses it |
| `text pw-build` | what `pw build` prints for the `pw gap=build` block just before it, which must have it | printed exactly |
| `pw gap=check` | a program `pw check` passes that the page says it should refuse (§5) | checks clean and is formatted; nothing is asked of the build |
| `sh`, `text`, `ts`, `tsx`, `svelte`, `json` | commands, output, or another language's code for comparison | not compiled |

A block with no info string, an unknown language or an unknown attribute is
refused by the test, so Pleris cannot enter the guide unchecked by being
left untagged.

**What "checked" means** is what the reference apps are held to in `just
test-compile`, applied to the guide:

- The program is the standard packages (`packages/pw-std/*.pw`,
  `packages/pw-platform-web/*.pw`) and the example's files: the unit set
  `pw check` accepts for the feed and kiokun. An example does not see the
  store's `examples/lib` or `examples/domain.pw`: an outsider's program does
  not have them.
- It is run through the real binary, `pw check --plain`, as
  `compiler/pw-cli/tests/related_files.rs` runs it ("the binary is run as a
  user runs it"), not through a library call a user never makes.
- A clean example reports no diagnostic at all, warnings included: the guide
  teaches nothing the compiler warns about.
- Every file is canonically formatted (`pw fmt --check`, ADR-0013,
  ADR-0276), as the corpus is: the guide teaches the canonical layout. A
  refused example that does not parse is the one exception: `pw fmt`
  formats a tree, and such a text has none.
- **A clean example also builds** (`pw build --out`): `pw check` passes
  programs the backend refuses. Found writing this guide, 2026-10-15: a
  `<title>` that reads a signal (refused under ADR-0137) and a template
  value computed from two signals (ruling 0073-a) each check clean and are
  refused by `pw build`. A guide that taught either would teach a program
  that does not run. A build costs about a second an example.

### 2. The tutorial is the app the server runs

The tutorial builds one app, a guestbook, in `file=` blocks. A page may say,
in an HTML comment the rendered page does not show,

```text
<!-- guide-program: examples/guestbook -->
```

and the test then holds the files its blocks assemble to be, byte for byte,
the `.pw` files in that directory: no more, no fewer, no different. That
directory is what the development server builds and serves, and what the
browser test drives. So the tutorial's text, the program checked, and the app
run are one set of files, and a change to any one without the others is red.

### 3. The diagnostic reference is generated from the registry

`docs/guide/diagnostics.md` is written by `scripts/guide_diagnostics.py`
from `compiler/pw-core/src/codes.rs` (ADR-0239's one registry): each code,
its symbol, the invariant it names in the registry's own sentence, its owner,
and the ADRs its entry's comment cites; then the retired codes and the old
numbers the corpus still writes (`DEPRECATED_ALIASES`). Two readings hold it
current:

- `scripts/tests/test_guide_diagnostics.py` (run by `just evidence-gates`,
  inside `just ci`) regenerates it and compares it with the committed file.
- The Rust test reads `pw_core::codes::ALL`, the registry as the compiler
  sees it, and holds the page to list every code with its invariant: a
  script that misread an entry would leave a code out, and that is red too.

So a code added, renamed or reworded without regenerating the page fails
`just ci`, and the page cannot claim a code the compiler does not have.

### 4. Where the test lives

`compiler/pw-cli/tests/guide.rs`, beside `related_files.rs`, because it runs
the `pw` binary (`CARGO_BIN_EXE_pw` is given only to the binary's own
package). `cargo test --workspace` runs it, so `just ci` does, with no recipe
and no change to the root justfile. It also holds the extractor to unit tests
of its own (fences, info strings, attributes), so a fence it misreads is a
failure of its own rather than an example silently skipped; and it counts the
opening ```` ```pw ```` lines a plain scan finds against the blocks it
extracted.

### 5. A gap in the checks is held, not only stated

The guide says what is true at its merge, and that includes where the
compiler falls short. A sentence that says "the check does not catch this
yet" is prose, and prose rots the day the compiler catches it: the guide
would then claim what is not so, which is the failure this ADR exists to
stop. So a gap is an example, with its own info string, and the test holds
the gap open:

- `pw gap=build`: a program `pw check` passes and `pw build` refuses, with
  the output `pw build` prints (`text pw-build`);
- `pw gap=check`: a program `pw check` passes that the page says it should
  refuse.

When the compiler closes a gap, the example goes red, with a message that
says the gap is closed, and whoever closed it makes the page true: a
`gap=build` example becomes a plain `pw` one or a `pw fails=` one, and the
sentence that introduced it goes. The gaps at this ADR's merge, each in
`docs/guide/tour-pages.md` ("Where `pw check` stops") and named in
`docs/guide/why.md`:

- a `<title>` that reads a signal checks clean, and `pw build` refuses it
  (ADR-0137);
- a template value computed from two signals checks clean, and `pw build`
  refuses it (ruling 0073-a);
- a computed list in an `{#each}` head checks clean, and `pw build` refuses
  it; and the effect check does not look in that head, which the same example
  shows, since it is a program the check passes with an effectful call in a
  view that claims `!{}`;
- an `<img>` with no `alt` checks clean (`gap=check`).

A fifth gap is a message and not a program: `&&` is refused with "expected
an expression, found `&`", which does not say that Pleris writes `&`. The
refused example of it carries the message (`text pw-check`), so a better
message turns that example red as well.

## Alternatives

- **Hidden lines, as rustdoc's `# `**: a block compiled with context the
  reader does not see. Rejected: the guide is read as Markdown on GitHub and
  in an editor, where nothing hides them, and an example a reader cannot
  compile as shown is the failure this ADR exists to stop.
- **Examples in files, included into the pages by a build step**: the guide
  would need a renderer the project does not have, and a page in the
  repository would not show its own examples.
- **Fragments allowed in `text` blocks**: a clause or an expression alone,
  unchecked. Rejected: every Pleris example in the guide is a whole program,
  short where it can be, so none is unchecked.
- **Checking through `pw_core::check::check_sources` in-process**, as
  `checking_source.rs` does for the corpus: faster, but it skips what `pw
  check` adds (the parse errors it renders, the exit status, the output a
  reader sees). The process costs about 0.2 s an example; the test runs them
  on several threads.
- **ADRs found by searching `docs/DECISIONS/` for each code**: more codes
  would get an ADR, but the page would then change whenever any ADR mentioned
  a code, and the integrator's numbering of an `ADR-XXXX` would turn a merge
  red. The registry's own comments are the source; a code whose entry cites
  no ADR says so.

## Consequences

- An example that stops compiling, a reformatting of the canonical layout, a
  diagnostic whose code or text changes, a tutorial step that differs from
  the served app, and a code registered without the reference regenerated
  each fail `just ci`.
- A change to the language that breaks a guide example is fixed in the same
  change, by its author: the guide is part of what the change touches.
- What is not checked: prose. A sentence about what a program does is held
  to nothing but review; the examples beside it are. What runs in a browser
  is checked only for the tutorial's app, by its browser test.

## Not claimed

- That the guide covers the language: it covers what a newcomer needs first,
  and says where it stops.
- That an example's behaviour at run time is checked: only the tutorial's is,
  in browsers.

## Next

- `pw new`, which writes the tutorial's first step as a project.
- An LSP on the check, which takes about 0.2 s for a small program.
- Each gap above, closed in the compiler, and its example turned into the
  plain or refused example the test then asks for.
- A data layer a program declares in Pleris, so that the tutorial's app does
  not borrow the feed's.

## The integrator's rulings, 2026-10-15

Each ruled by the integrator, 2026-10-15:

1. **The guide's test lives at `compiler/pw-cli/tests/guide.rs`**: accepted
   (ruled by the integrator, 2026-10-15). It is a new file in a package the
   track did not own, and it reads `docs/guide/*.md` from the repository's
   root.
2. **Port 7741, the guestbook's hosts at `PORT+210..212`** (`GUESTBOOK_PORTS`
   in `spikes/own-renderer/playwright.config.mjs`): accepted (ruled by the
   integrator, 2026-10-15).
3. **The track's own corrections are applied** (ruled by the integrator,
   2026-10-15): a handler arm that does nothing is `()`, and every `on:`
   lambda is resumable, its captures inferred, with no
   `resumable(captures = ..)` (ADR-0134, ADR-0200). Applied in
   `docs/guide/tutorial.md`, `docs/guide/errors.md` and `examples/guestbook`
   together, the tutorial and the app byte for byte equal (the test holds it).
4. **The checker findings are framework findings** (ruled by the
   integrator, 2026-10-15): the integrator takes them to the tracks that own
   them. The guide states what is true at its merge, as a current limit
   where a reader would meet one, and neither teaches around it nor claims
   what is not so. The three findings are `pw check` passing programs `pw
   build` refuses, an effectful call in an `{#each}` head escaping the effect
   check, and `&&` getting no hint that Pleris writes `&`. §5 holds each, and
   the guide's page of limits names them.

## Findings

Found writing the guide, corrections first:

- **Corrected during the work.** A handler arm that does nothing may be `()`
  (`draft = draft` was the feed's style, not a rule), and no `on:` handler
  needs `resumable(captures = ..)`. And the each-head hole is smaller than
  first written: `pw build` refuses the program for another reason (a
  computed list), so no program with it builds today; `pw check` alone
  passes it.
- **`pw check` is not the build.** Three shapes pass the check and are
  refused by `pw build` (§5). The build's refusals have no code and are not
  in the registry. A newcomer's green check is not a program that runs, so
  the guide's test builds every clean example.
- **No project file, no `pw new`.** To check one file, a newcomer names
  `packages/pw-std/*.pw packages/pw-platform-web/*.pw` and the file on the
  command line. An effect used means nothing until a package declares it.
- **An app's storage is Rust.** The development server holds each data
  layer as hand-written Rust (`feed.rs`, `store.rs`, `kiokun.rs`), chosen by
  the operations' prefix. The tutorial's guestbook borrows the feed's, and
  had to declare a type named `Post`, because the feed's grants name it and
  an effect row must name a grant. A newcomer cannot run a new storage
  without Rust.
- **Surprising syntax.** A predicate used as a boolean in markup
  (`hidden={!SignedIn}`, `{#if SignedIn}`) is how a page asks it of its
  reader (PW5048); `List.length` needs `import List` where the type `List`
  does not; a name in a pattern that starts with a capital is a case.
- **Stale reading for a newcomer.** `docs/SEMANTICS.md` still reads as the
  M0-era status ("Not yet enforced: no compiler exists" for pure rendering).
  `docs/vision/capabilities.md` is dated 2026-10-08 (ADR-0279).
- **Hazards in the harness.** Stopping a Playwright run with `pkill -f
  "playwright test"` leaves its web servers on their ports (78 of them), and
  the next run fails "port already used": kill by the servers' working
  directory.

## Evidence

`just e14-guide` records it (`docs/evidence/E14/guide.txt`, written by the
verification run). What the track ran itself, on 2026-10-15, before it:

- **The guide's test**, `cargo test -p pw-cli --test guide`: 15 tests, green.
  It checks 48 programs: 26 clean (no diagnostic, formatted as `pw fmt`
  writes them, built), 18 refused with exactly the codes they name (6 with
  the compiler's output shown, compared to what `pw check` prints), and 4
  gaps (3 build gaps with the build's output shown, 1 check gap); the two
  programs its `file=` blocks assemble (the tour's and the tutorial's), the
  tutorial's two files byte for byte `examples/guestbook/*.pw`; and the
  reference's 234 rows, each registered code with its invariant. The other
  11 tests are the extractor, the verdict, the page-level guards and the
  directory comparison, each held on its own.
- **The generator's test**, `scripts/tests/test_guide_diagnostics.py`: 7
  tests, green, in `just evidence-gates`.
- **The tutorial's app in a browser**, `e2e/guestbook.spec.mjs`: four tests
  (a signed-out reader's page; signing up, signing and removing one's own
  entry; the entry shown before the server answers, with the command's
  request held at the network, and no Remove until the server's row stands
  in its place; another reader's open page told of an entry and of its
  removal), in Chromium, Firefox and WebKit.
- **The mutation controls**, `scripts/guide_mutations.py`: **74 of 74
  mutants killed**, each by a test it names (a kill by another test, or by a
  build that fails, counts as a survivor). Twenty are ways the guide rots
  (17 in its pages, 3 in the reference), 39 are in the test's own checks and
  guards, 7 in the generator, and 8 in the tutorial's app: 5 killed by the
  browser spec in Chromium and 3 refused by a rule of the checker, named in
  `CHECKED` (PW5048 twice, PW5107). The first whole run killed 72: the
  survivors were a test that did not tell an indented fence from one never
  closed, and one that did not tell an output after the wrong block from one
  after no refusal at all. Each got the case that does, and the script ran
  whole again.

## Questions for the integrator

- `scripts/mutant_programs.py` (train-7) checks the mutants of `.pw` sources
  under `examples/feed`, `kiokun`, `kiokun-site`, `store`, `lib`, `domain.pw`
  and `packages/`. A mutant of `examples/guestbook` is outside its tables
  (`PROGRAMS`, `OF`), so it is skipped; `scripts/guide_mutations.py` keeps
  each of its `.pw` mutants a program (or names the rule that refuses it in
  `CHECKED`) on its own. Adding a `guestbook` entry to those two tables would
  put it under the gate; it is the integrator's script.
- The gaps in §5 belong to the tracks that own the checker and the backend:
  a refusal the check should make where the build makes one, the each head
  in the effect check, the `<img>` rule, and the message for `&&`.

## The integrator's answers

On train-9, 2026-10-15, and what the merge did:

- **Numbered ADR-0343**, merged on train-9 from `49967631` (the track's
  merge of train-8; its verify at `00471782` was green, its recipe's shard
  among the jobs). `ADR-XXXX` reads `ADR-0343` in the guide's test, the
  generator and its test, the mutation script, the recipe, the guestbook's
  build script and spec, the Playwright config's comment, and the guide's
  own README, which now names this file.
- **The reference was regenerated at the merge**, as §3 asks of whoever
  adds a code: ADR-0340, merged before this on the same train, registered
  PW0405 to PW0407 (199 codes), and `python3 scripts/guide_diagnostics.py`
  wrote their three rows. With it, the guide's test passed whole on the
  train (15 of 15), its examples among them under the new rules, and the
  generator's test (7 of 7).
- **The guestbook's hosts beside the reviews'**: the train's Playwright
  config holds both groups, `REVIEWS_PORTS` at PORT + 200 to 202 and
  `GUESTBOOK_PORTS` at PORT + 210 to 212.
- **The root README's link to the guide** (`b955a195`) is kept: a developer
  from outside finds the guide from the repository's first page.
- **`scripts/mutant_programs.py` and the guestbook** (the first question):
  not ruled at this merge, and open. `guide_mutations.py` keeps its own
  `.pw` mutants programs meanwhile.
- **The gaps** (the second question; the rulings' 4): `pw check` passing
  what `pw build` refuses is `track/check-is-build`'s, and the `{#each}`
  head's effects `track/each-head-effects`'; the `<img>` with no `alt`
  and the `&&` message have no track yet, and KNOWN_LIMITATIONS lists them
  with the others. Each is held by its example here until it closes.

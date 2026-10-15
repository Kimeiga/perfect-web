# ADR-XXXX: A guide whose examples compile

- Status: proposed (track `guide`), for the integrator to number.
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

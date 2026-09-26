# ADR-0106: `pw check` reports each file by its place

Status: accepted under the owner's instruction of 2026-09-26 ("keep working
until its perfect"). Date: 2026-09-26. Milestone: E10 (charter §16).

## Context

`pw check` names each file by its file name, `app.pw`, and kept each file's
diagnostics in a map keyed by that name. Two files may share a name: the
corpus has `examples/store/app.pw`, `examples/kiokun/app.pw`,
`examples/streamed/app.pw` and `examples/counter/app.pw`, and the platform
package has a `build.pw`. When two did, the second file's diagnostics
replaced the first's, and both files were rendered with the second's.

On 2026-09-26, at 27d1d9f, `pw check a/app.pw b/app.pw` printed
"no diagnostics" and exited 0, with a type error in `a/app.pw`. The error was
reported when `a/app.pw` was checked alone. The same collision printed a
probe named `build.pw` twice, once against the text of the platform's
`build.pw`.

Every `pw check` the justfile runs includes both `packages/pw-std/effects.pw`
and `packages/pw-platform-web/effects.pw`. The standard library's file was
never reported on: had it held an error, `just ci` would have passed. Checked
under a name of its own, it has none.

`pw build` and the emitters name a unit by its path, and were not affected.
The checker answers in the order it is asked, and labels each answer with
the name it was given, nothing more.

## Decision

**Each file's diagnostics are its own, by its place.** `pw check` pairs the
checker's answers with its inputs by position, never by name.

**A file is named by its path where its file name is another's.** Output
for a file whose name no other file shares is unchanged.

## Acceptance

- **`compiler/pw-cli/tests/same_name.rs`**, 2 tests, run against the `pw`
  binary as a user runs it. Each fails at 27d1d9f, the code at 584d4a2:
  - `a/app.pw` and `b/app.pw`, either one holding a type error. The error
    is reported, once, `pw check` exits 1, and the file is named by its
    path. The control: a file checked alone is named `app.pw`, as before.
  - One file given twice. The second copy also declares its module twice,
    so the two report one error and two, three in all. By name, each had
    the second's two.
- **Every recorded `pw check` output is unchanged.** The two `effects.pw`
  files have no diagnostics, and a file is named only in a diagnostic.
- **Mutation controls:** `scripts/same_name_mutations.py`,
  `just e10-same-name`, 2 mutants.

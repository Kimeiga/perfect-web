# ADR-0135: a handler's identity is its own file's

Status: accepted under the owner's delegation of 2026-10-02. Date: 2026-10-02.
Milestone: E14. A defect found writing ADR-0134, fixed.

## Context

`pw build` gives each event part the identity `resume_artifacts` derived
for its handler, through one map for the whole program
(`template_ir::Handlers`). The map was keyed by a declaration's index and an
expression's index, and both are counted within one file. Two pages of one
shape in two files had the same key.

So the second file's identity replaced the first's, and the first page's
button loaded and ran the second page's handler. Found with two pages that
differ only in what their handler adds: at `c958e80` page `a`'s button is
wired to page `b`'s module (`n + 2` where `a` writes `n + 1`). Every
multi-file program was exposed, and nothing in the build or in the browser
could see it, because the wrong identity is a real one.

## Decision

The map is keyed by the file as well: `(unit, declaration, expression)`.
The build enumerates its files when it records each identity, and template
lowering carries its file's index when it looks one up.

## Acceptance

- **`compiler/pw-core/tests/handlers_by_file.rs`**:
  `two_pages_of_one_shape_in_two_files_keep_their_own_handlers` fails at
  `c958e80` and passes after. Each page's event part names a module whose
  body adds that page's own step.
- **Mutation controls:** `scripts/handlers_by_file_mutations.py`, `just
  e14-handlers-by-file`, 2 mutants: the build and the template each
  dropping the file from the key.
- No program's artifacts change except where two files collided. The
  store's and the demo's handler identities are unchanged.

## Not claimed

Every other map keyed by a declaration or an expression was checked and is
built within one file (`values::relations`'s `typed`, `lexical`'s and the
label maps). None is shared across files.

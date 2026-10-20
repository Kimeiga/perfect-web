# ADR-0355: the committed components are the build's, every one

Status: accepted, the integrator's ruling of 2026-10-19 (train-11), under the
owner's delegation of 2026-10-02; applied by the integrator's delegate
assembling train-11. Builds on ADR-0125 (the store's committed page plan and
components, which the development server's tests run without linking the
compiler).

## Context

`docs/evidence/E10/<id>.wasm` holds the store's compiled components: the
development server's tests and pw-host's run them without linking the
compiler. Three lists named them, each kept by hand: `just e10-component`'s
(20 ids), the server's test loader's (20), and
`evidence_is_current`'s (14). The test held 14 of the 20 committed to the
compiler; of the six it left out, three were stale at train-10's head
(587657ff): `store.hours.StoreStatus`, `StoresOpen` and `CartHours`, last
emitted 2026-10-13, before scheduled-orders and reviews changed the hours
module (`CartHours` committed 31,114 bytes, built 16,628 now). No test saw
it. The store's build emits 55 components.

## Decision

- **The test's list is the build's**: `evidence_is_current`'s
  `the_committed_components_are_what_the_compiler_builds_now` builds the
  store as `pw build` does and holds `docs/evidence/E10/*.wasm` to exactly
  its components (`Build::components`, each `Built::Component`): each built
  one committed, byte for byte, and none committed that the build no longer
  emits. Never a list kept beside the build.
- **`just e10-component` commits what the build emits, whole**: `pw build
  --out`, then every `components/*.wasm` (the old ones removed first). The
  bytes are `pw emit-component`'s (checked equal for `StoreStatus` and
  `CartHours`, and for the 17 committed ones that were current). 55
  components are committed (600 KB), 35 of them new.
- **A control**: `a_component_the_build_adds_fails_the_check_until_it_is_committed_current`
  builds the store with one query more and shows the check naming it as
  not committed, then stale, then nothing once committed as built; and a
  committed component the build no longer emits named too.
- The server's test loader keeps its own list: it loads what its tests
  run, and a component it needs and lacks fails them loudly; staleness was
  the silent failure.

## Evidence

- Red before: with the old committed set, the derived check names 3 stale
  components and 35 built and not committed; green after `just
  e10-component`.
- `scripts/committed_components_mutations.py`: 5 of 5 mutants killed (the
  build's components not read; a component not committed, committed stale,
  or committed and no longer built, passing; a kept 14 of them checked).
  Recipe `e14-committed-components` (`just/committed-components.just`).

## Not claimed

- Other committed fixtures kept by hand lists: the store's template IR
  (`spikes/own-renderer/store-ir.json`, which `run.sh` rewrites) is not held
  here; `wip/host-map-all-2026-10-19` (a520282f) has a check of it for the
  integrator.

## A changed binary plans what reads it (the integrator's addition, 2026-10-20)

Train-10's landing found the other half of the defect: **neither the planner
nor the evidence fetch's stale check sees a change to a binary file**. `git
diff -U0` gives a binary no `+++` line and no hunk, only `Binary files a/x
and b/x differ`, so `ci_plan.parsed` never listed it: 8c5618c8, which
regenerated three of the store's components, planned no recipe and staled no
evidence, and the recipes that read them (72, with what else train-10
changed) were chosen by hand.

Now `ci_plan.hunks`, which the planner (`--changed`), `current`'s reach, the
fetch's stale check (`hunks(sha, None)`) and the evidence-count check all
read, lists each binary the diff names (`binaries`: changed, added, or
deleted, by its path before) and adds, as changed lines, **each line of a
text file that names it** (`read_by`, `with_readers`), at the head the range
ends at, or in the tree here: its path; its directory as a whole
(`root.join("<directory>")`, `cp ... <directory>/`); or its directory with a
name a glob or a placeholder makes it (`<directory>/*.wasm`,
`format!("<directory>/{id}.wasm")`, `$id`, `${id}`); never another file's
name, a directory inside or beside it, or a record (`docs/`,
`scripts/ci_seconds.json`, as `verified_base.RECORDS`). The planner's rules
then plan from those lines as from any change: the recipe whose own lines
name the binary, a test file that loads it and so each script and recipe
that runs that test, a mutant beside such a line.

8c5618c8 alone, planned on this tree's recipes: none before; now
`e10-build`, `e14-command-retry`, `e14-committed-components`,
`e14-descriptions`, `e14-last-known-good`, `e14-render-merge`,
`e14-row-reads`, `e14-telling`, `e9-values` (through
`evidence_is_current.rs`, pw-host's `display`, `host_answers` and
`list_arguments` tests, the server's loader lines in `main.rs`, and
`e10-component`'s own lines). Tests (scripts/tests/test_ci_scripts.py):
`test_a_changed_binary_is_named_where_a_line_names_it`,
`test_a_changed_component_plans_the_recipes_whose_tests_read_it` (with the
control that the component alone, as the diff gave it, plans nothing), and
`test_a_binary_changed_in_a_range_or_the_tree_here_is_changed_where_it_is_read`
(on a git of its own). Controls: eight in
`scripts/evidence_integrity_mutations.py` (2b), each killed by the plan
suite.

Not claimed: a binary read by a name the program builds at run time with
its directory nowhere on the line (a list of ids joined with a directory
named elsewhere) is found only by the line that names the directory; a line
naming an ancestor as a whole (`docs/evidence`) does not name it; and a
recipe whose own lines run a reader test without a mutation script is
planned by that test's change only where the planner already plans it.

## Numbered on train-11 (2026-10-20)

- **Numbered ADR-0355** (the integrator's ruling of 2026-10-20: a train is
  numbered before it is pushed, after master's ADR-0353, in train order),
  merged from train-11's own commits. `ADR-XXXX` reads `ADR-0355` in
  ADR-0356's cross-reference, as in this file's name and title.

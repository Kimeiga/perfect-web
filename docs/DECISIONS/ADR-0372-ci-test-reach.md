# ADR-0372: changed tests plan the checks that consume them

Status: accepted under the owner's delegation, 2026-10-20.

## Finding

Train-11 recorded two silent verification gaps. A change inside a crate
root's test module was treated as `main::` or `lib::`, neither of which
is a Rust test namespace. A changed Python test was never an input to the
mutation-script reach rule. Both gaps reproduced with failing planner
tests before the implementation. They also let old evidence appear current,
because planning and evidence freshness use the same reach functions.

## Decision

For an inline test-module change in `src/main.rs` or `src/lib.rs`, find the
owning package from its Cargo manifest and conservatively select consumers
that name that package's tests or workspace tests. This includes consumers
using bare test-name filters and avoids inventing a `main::` namespace.
Selecting more of that package is preferable to silently missing a witness.
Ordinary source lines still use the existing anchor reach rule.

Python test files under `scripts/tests/` reach mutation scripts and direct
recipe bodies that name the path, its dotted unittest module (including
class/method selectors), or its directory and filename separately as an
explicit discovery command does. Name boundaries prevent prefix neighbors
from counting. This is the existing static-reference model, not execution
of arbitrary test discovery or dynamic command construction.

## Validation

The regression tests cover both Rust root filenames, shell and argv package
selectors, workspace consumers, unrelated packages and package prefixes,
Python paths, dotted selectors, explicit discovery, and unknown tests.
Mutation controls remove each new script/direct-recipe reach edge.
Commands and results are recorded after execution; no remote gate is claimed
by the presence of this note.

## Limits

Package selection is conservative and may run more Rust recipes than strictly
necessary. Existing inline-module recognition is unchanged; custom Rust
module layouts and dynamically constructed Python test commands remain
outside this static-reference rule. No application language semantics change.

## Evidence consumer correction found during validation

The untouched master at 115895de fails the evidence-count gate once the
train-11 evidence is current. Its guide recipe hashes both mutation scripts
but runs only the producer campaign. The gate mistook the hash input for
execution. It also chose an intended test-method ID as the description
because that identifier was longer than the actual mutant description.
Both have red-before/green-after regression tests and independent controls.

Campaign discovery now tokenizes shell text and recognizes literal Python
invocations (including supported interpreter flags), preserving quoted
strings. Hashing or printing a filename is not running it. Method IDs beside
a mutation description are metadata; they cannot replace its description.
Actual second campaigns still require their own complete evidence blocks.
Dynamic shell execution remains outside this static-reference model.

## Bare module selectors in native producer inventories, 2026-10-20

The train-12 follow-up plan omitted `e14-mutation-bound` after a change to
`test_ci_scripts.py`. That campaign names `scripts/tests` and the exact bare
module `test_ci_scripts` in a tuple; it constructs the `.py` discovery pattern
later for the shared native producer. The existing rule sees only a filename
with its suffix or a fully qualified module and therefore misses this reader.

Extend the same static directory/name rule to exact bare module selectors,
including class/method suffixes, while retaining name boundaries. An unrelated
module with the same prefix or the same basename in another directory remains
unselected. A regression must require the actual campaign and its recipe, plus
synthetic bare/class/neighbor consumers. A control removes just the new edge.
No separate recipe allowlist is introduced: freshness and planning continue
using the same canonical function. Whole affected verification remains due.

[Native before/after and whole nine-control output](../evidence/tooling/ci-bare-module-reach.txt):
the real consumer was omitted before the correction; both focused cases and
all nine strict controls pass afterward, with no producer errors or memory
stops. Exact hosted follow-up and final combined gates remain pending.

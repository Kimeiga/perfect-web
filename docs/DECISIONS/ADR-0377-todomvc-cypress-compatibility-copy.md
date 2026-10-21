# ADR-0377: Execute an identified TodoMVC Cypress compatibility copy

Status: accepted by the integrator, 2026-10-20; E14. Combined gates remain
pending. Original vendored files remain
the seven upstream blobs recorded in `spikes/todomvc-cypress/UPSTREAM.md`.

## Finding

The combined device-backed app's original Cypress run passed 24 of 29 tests.
The pinned Cypress 15.21.1 spy has `resetHistory`, not the suite's `reset`;
the custom createTodo command ends with a logging `then`, leaving an aliased
DOM element from before reload. These are test-tool compatibility failures.
They do not establish that the unchanged execution passes.

## Decision

Generate a disposable, explicitly named `todomvc-cypress-compat-v1` project.
Validate all seven upstream Git blob hashes and the declared, installed
package and binary Cypress version first. Apply exactly two unique patches:
replace the one `invoke('reset')` with `invoke('resetHistory')`; preserve
createTodo's logging `then` and append a returned get/contains query for the
created todo. Keep all test bodies, storage checks, tables and configuration.
No application detection, private Cypress API or application change is added.

The generator holds the patch identity and patched outputs to fixed hashes,
writes a deterministic manifest, and revalidates the source and complete
staged inventory immediately before launch. `run.sh` names the compatibility
identity and manifest hash. Acceptance still requires all 29 tests, no failed,
pending or skipped tests, and native exit zero. The separate diagnostic is
still non-acceptance. Originals are unchanged; execution is explicitly adapted.

Public contracts checked: Cypress's [component version CLI](https://docs.cypress.io/app/references/command-line),
[query aliases](https://docs.cypress.io/app/core-concepts/variables-and-aliases),
and Sinon [spy resetHistory](https://sinonjs.org/concepts/spies/api/reset-history).
Native probes of this pinned Cypress establish the concrete compatibility
behavior. Future upstream or Cypress changes require a new reviewed identity.

## Validation and limits

Eleven focused tests and all eight strict frozen-inventory unittest controls
pass. The source-drift control starts with an unpatched original file so it
proves an actual omitted refusal. The unique-anchor regression is a guard
check; source and output hashes also reject ambiguous changed inputs. Fixed
output-digest validation is a separate explicit identity guard.

The final launcher passes 29/29 with zero failures, skips or pending tests and
native exit zero. Two copied built-app controls retain all other bytes and
pass `node --check`; HTTP-loaded runtime SHA-256 is checked before Cypress.
Removing the real storage write produces the actual `setItem` assertion and
acceptance refusal. Later timeout/hook cascades leave that run incomplete
(3 pass, 3 failures, 23 skipped); they earn no mutation-kill credit. Removing
the reload read yields 28 passing tests and only `should persist its data`
failing with a native assertion; acceptance refuses it. These are negative
acceptance observations, separate from the eight strict guard controls.

The first write-loss probe was ineffective: the launcher's shell PID cleanup
left the previous actual host alive, and a new failed bind reached that old
app. Its apparent 29/29 is retained as invalid negative evidence. The launcher
now explicitly execs the host, rejects an already occupied port, checks the
owned PID during readiness, and kills and waits for it on exit. TERM/INT exit
through that cleanup; host logs use the run's OUT path. All three final native
runs verify the captured host PID is gone and their staged project removed.
The existing launcher's owner adds occupied-port and cleanup regressions in
its separately owned test file; combined validation must cover them.

Raw native outputs, manifest, exact copied-app edits and source hashes:
[compatibility proof](../evidence/tooling/todomvc-compat-native.txt).
Whole existing producer controls and exact combined CI remain pending. The integrator's
earlier scratch 29/29 and the failed first probe are diagnostic history only.
Historical failed unchanged runs remain failed. This adapter establishes
neither an unchanged-suite pass nor broader TodoMVC correctness.

## Metadata import stays inert

The follow-up planner exposed an import-time bug before dispatch: importing
`todomvc_compat_mutations` to count its eight controls also imported
`mutation_baseline`. That started a watcher and appended a memory-bound line
at process exit. The native TodoMVC plan therefore wrote two stdout lines,
while the workflow redirects this stream into `GITHUB_OUTPUT`, whose entries
must be output parameters ([GitHub workflow commands](https://docs.github.com/en/actions/reference/workflows-and-actions/workflow-commands#setting-an-output-parameter)).

Import the runtime only inside `main`, as its own module contract requires.
The actual campaign still starts the same guard and keeps all eight controls.
A fresh native planner regression requires exactly its one `shards=` JSON
parameter, and a control restores the erroneous eager import. Existing
planner tests selected older recipes and did not exercise this new metadata.

[Native red/green and complete control output](../evidence/tooling/todomvc-metadata-import.txt):
the new planner regression passes, all ten changed-test/planner controls and
the unchanged eight compatibility controls pass. The real campaign still
reports its memory guard; metadata planning no longer starts that runtime.


## Landing (2026-10-21)

Landed on `master` with train-12 at `e7664d2eb01aa8c6e91df594bd7270a84a8bf8de`
after exact-commit CI ([39584219053](https://github.com/studiokimeiga/perfect-web/actions/runs/39584219053))
and automatic track verification ([39575516650](https://github.com/studiokimeiga/perfect-web/actions/runs/39575516650)).
Dated statements above that call landing or combined gates pending describe
the candidate before it landed and are kept as written. The
[acceptance record](../diagnostics/train-12/acceptance-record-2026-10-21.md)
states what the landing's evidence does and does not prove.

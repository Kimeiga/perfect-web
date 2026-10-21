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

# Todo toggle-all state witness (2026-10-21)

The hosted run `verify-39555014640` reported an incomplete native TodoMVC mutation witness: 7/43 kills and one refused case for the unchanged mutation “the toggle-all completes only when every todo is active.” Its Chromium action failure was in the row-identity test after a row was toggled and toggle-all was immediately checked then unchecked without an assertion establishing the mixed list before the first action or all-completed rows before the second. The exact hosted evidence and raw native log are retained here as `hosted-failure-todomvc.txt` and `hosted-native-log.txt`.

The repair keeps both original toggle-all actions and the held-row identity checks. The mixed-state test now asserts which rows are active/completed and that toggle-all is unchecked before using click, then checks all rows and the checkbox state. The row-identity test similarly checks mixed state before the first click, all completed rows and checked boxes before the second click, and each row's cleared state and unchecked boxes afterward. This avoids `.check()` / `.uncheck()` implicit post-action state verification on a checkbox that rerenders as a consequence of the same action. The neighboring toggle-all tests were inspected; this mixed-state `.check()` pattern also occurred in test183 and is repaired there. Other checks use a precondition or assert the semantic result before another dependent action.

The final exact mutation was selected by unique name in `reproduce.py` (match count asserted to equal one); it resolved to one-based slot 8. Baseline and restored browser receipts each contain the exact same 50 IDs (`results-final/expected-50-ids.json`). All strict results are retained in `results-final/`, with command output files, complete assertion receipts, expected inventory, and build outputs.

`reproduce.py` emitted these native statuses and strict validations:

- Baseline host: native exit 0; strict receipt valid, 4 passed / 0 failed.
- Baseline Chromium: native exit 0; strict receipt valid, 50 passed / 0 failed.
- Original named operator 8: native exit 1; strict receipt valid and complete, 48 passed / 2 assertion failures. Both failures are semantic class assertions in the two mixed-state toggle-all tests; neither is a locator/action timeout.
- Restored host: native exit 0; strict receipt valid, 4 passed / 0 failed.
- Restored Chromium: native exit 0; strict receipt valid, 50 passed / 0 failed.

The producer's raw stdout/stderr and per-test assertion output are in each results subdirectory. The terminal summary is transcribed from the harness output; it is not treated as a native output artifact. This focused proof took about 36.4 seconds. It did not run all 43 mutations.

Two earlier setup attempts are retained under `setup-incomplete/`: one had the warm target cache but no worktree target link and failed before browser launch when the wasm copy was missing; the next had a root dependency link but not the required `spikes/own-renderer/node_modules` link and failed before browser launch on the TodoMVC CSS check. They are not validation. A diagnostic run selected the wrong numeric index (operator 7, “the counter counts the completed”) and strictly recorded a valid 47/50 receipt; it is preserved under `setup-incomplete/wrong-mutant-index-operator7-counter` and is not evidence for the named operator. Two additional named-operator runs at the preceding test source are retained under `setup-incomplete/pre-final-source-proof-*`; they are superseded by the `results-final` run after per-row negative assertions and the checked/unchecked toggle-all expectations were added. None of these setup or superseded attempts counts toward final validation.

Input SHA-256 values for the final valid proof:

- `spikes/own-renderer/e2e/todomvc.spec.mjs`: `a02de954856f40227d757319aa4fa3f9ef5bdf0f504cee7a97290b5de1a7353d`
- `scripts/todomvc_mutations.py`: `54389c924615a85a9ffa7eb0fb801a11718dc4d266ffd0be78ac93c3e0dc5d5e`
- `examples/todomvc/app.pw`: `39e6a335f3b6ca326081a4fc2e64e44485d74480edf398c57cb87f72e88a26eb`
- `scripts/browser_test_results.py`: `044bfc4106f0b718f92e8fa08e7d2d07670e1732c334c8a6bac2c3f06341b90c`
- `scripts/cargo_assertion_results.py`: `5700a7c5ddcebe87c850979fc09fbce3295518e547f93d456883916799566d1f`
- `scripts/mutation_source.py`: `a76a0225719d14095a60fa5081e574f5b59b013d64c1806ba68351f99213d23b`
- `scripts/producer_process.py`: `7772f6d149b1c0fa113a893e9c1fccfbe8690dca36d854d327ea8536b11a8180`
- `spikes/own-renderer/assertion-result-reporter.mjs`: `ba3bb0900f0c50bb835a2cdaf09e2bb3fc626e7472344007750426fee1e0d265`
- `reproduce.py`: `500cafdcf479da372550d13a76f7d624d151a540f310c44b12c34f545ddd54cf`
- `results-final/expected-50-ids.json`: `31b2264caaf5ec6d55fc4603a7df77888b565f4481ceb8f18a2ba59a2774facc`
- Baseline, operator, restored receipt hashes: `39567732c74dc47d39ce8819d8446a80603ca6596cd2c3034a339c17da70cf36`, `a5294331d243bc032cf6d55e413d0c3cdd7022dad2a1ba1edb320239f1e856f3`, `2b324bc5f56d9140934122e7ab8f35e90eacb83308f4a065baa0102b7c3e31e1`.

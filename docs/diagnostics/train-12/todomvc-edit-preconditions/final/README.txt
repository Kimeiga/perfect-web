Focused native TodoMVC edit-witness proof

The proof harness used the command:
  python3 /tmp/train12-todomvc-edit-preconditions.py

Exact harness source is preserved as reproduce.py. Its host-specific paths and
settings are intentionally preserved from the run. Its stdout, captured by the
exec session in two output batches, is transcribed in captured-harness-stdout.txt.
The native server and browser statuses and strict validator results below are
those emitted by that harness; they are not derived from reporter output.

Source workspace during the run:
  /home/haki/code/pw-wt/todomvc-toggle-all-visible-precondition
The resulting source was integrated as a130a860d1fcd4633dc3def51a98d340a5f8a685. The same spec bytes are in this worktrees local commit 4ec0337160d99c80e1b85e3cb7e2ea6e0c112d94.
The proof completed in 49.709920764900744 seconds. The two program mutants
were the unchanged entries 16 and 30 in scripts/todomvc_mutations.py. Each
mutant passed the harness's pw check assertion, then ran all 50 Chromium tests.
The captured stdout contains native browser exit status 1 plus strict parsed
valid=true, 35 passed, 15 failed. The receipts have complete=true and preserve
the same exact IDs as baseline and restored. The four host tests were run at
baseline and restored only; both native statuses were zero and both strict
receipts were valid 4 passed / 0 failed.

The harness prints no separate native status line for pw check. The saved stdout
says 23 files, no diagnostics, and the script asserts returncode == 0 before
continuing. This README does not claim a separately captured exit-status line
for that check. Build steps are also guarded by returncode assertions in the
script.

ID linkage: expected-50-ids.json is the exact inventory returned by the baseline
browser validator and used for each following parse. It contains 50 IDs. Each
assertions.json contains 50 unique IDs; the final summary's expected inventory
hash and the per-file hashes below allow comparison without relying on counts
alone.

Input SHA-256 values (baseline/restored source and validation inputs):
  spikes/own-renderer/e2e/todomvc.spec.mjs
    f91d50de63d7e725ce2bd3f462f8458a69b068a1a713c835cafff4387c6bcb1b
  scripts/todomvc_mutations.py
    54389c924615a85a9ffa7eb0fb801a11718dc4d266ffd0be78ac93c3e0dc5d5e
  examples/todomvc/app.pw
    39e6a335f3b6ca326081a4fc2e64e44485d74480edf398c57cb87f72e88a26eb
  scripts/browser_test_results.py
    044bfc4106f0b718f92e8fa08e7d2d07670e1732c334c8a6bac2c3f06341b90c
  scripts/cargo_assertion_results.py
    5700a7c5ddcebe87c850979fc09fbce3295518e547f93d456883916799566d1f
  scripts/mutation_source.py
    a76a0225719d14095a60fa5081e574f5b59b013d64c1806ba68351f99213d23b
  scripts/producer_process.py
    7772f6d149b1c0fa113a893e9c1fccfbe8690dca36d854d327ea8536b11a8180
  spikes/own-renderer/assertion-result-reporter.mjs
    ba3bb0900f0c50bb835a2cdaf09e2bb3fc626e7472344007750426fee1e0d265
  reproduce.py (same bytes as the harness executed)
    9b2ddfe656cfc98bead68e8029ff07dff6e39f0cc6f386c61a2e6db5006ee041
  expected-50-ids.json
    31b2264caaf5ec6d55fc4603a7df77888b565f4481ceb8f18a2ba59a2774facc

The first operator-16 attempt before adding the completed-todo visibility
precondition is retained in ../preliminary-incomplete/. It contains a 30-second
locator.fill timeout and no complete browser receipt. It is explicitly
incomplete and is not part of the valid operator-16 result above.

sha256sums.txt covers the preserved raw outputs, receipts, inventory, and
harness files in this directory. It excludes itself.

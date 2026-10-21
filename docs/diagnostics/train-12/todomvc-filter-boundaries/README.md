# TodoMVC filter-boundary witness, 2026-10-21

These files record two strict native campaigns run from worktree commit
`aabed65c19599c548a4530a357bd0361e15bc0c2`, with only
`spikes/own-renderer/e2e/todomvc.spec.mjs` modified. The test source SHA-256
was `0397aca273bdf2a49e95a90a063428def7af403d08b5ff1e25795ba6b695ea93`.
The mutation operator definitions and receipt validators were unchanged.

`focused-operators-10-11.txt` records baseline, operators 10 and 11, and
restoration. Its harness is `run-focused.py`; both mutants were killed by
assertions, with no refusal. Baseline and restored native browser receipts each
contain all 50 tests passing; host receipts each contain all four tests passing.

`sweep-operators-12-20.txt` records baseline, the exact original operator
indices 12 through 20, and restoration. Its selection inventory is
`inventory-12-20.json`, and its harness is `run-sweep.py`. All nine mutants
were killed by assertions, with no refusal. Baseline and restored native
browser receipts each contain all 50 tests passing; host receipts each contain
all four tests passing. The `receipts/` directory preserves the 15 browser
JSON receipts from both runs. The summaries preserve the host inventories and
native assertion diagnostics.

The wrappers reused the warm Cargo target and `node_modules` from
`/home/haki/code/pw-wt/train-12` through temporary symlinks removed at process
exit. Node 22.21.1 was selected explicitly. Each campaign ran the strict
existing producers and validators, with one sequential mutation at a time.
The focused campaign process returned zero; the sweep process returned zero.
The raw summary's trailing `*_exit=` fields are blank because the wrapper's
shell-variable interpolation did not serialize the numeric code; the outer
process results returned exit code zero. Do not infer any other outcome from
those blank fields.

`setup-*.txt` retains initial harness/setup failures. The Node 12 parse error,
missing `cargo` PATH, and missing worktree `target` path occurred before a
complete baseline and are not evidence. The two intermediate harness files
also record selection/inventory construction errors before producers ran.
They are kept separate from the successful strict campaign summaries.

This is a bounded witness for operators 10–20, not a run of the complete 43
operator set or the full verification recipe. No time-budget stop was counted
as a kill.

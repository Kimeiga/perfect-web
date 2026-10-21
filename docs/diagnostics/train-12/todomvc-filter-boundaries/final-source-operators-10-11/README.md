# Final-source TodoMVC filter-boundary proof, 2026-10-21

This strict run follows restoration of the original empty Completed-filter
transition in the filtered-update test. The source test SHA-256 was
`260d8f30037942c17d8d936087ced87603044a11da38e18eac7291f1b83b95c2`.

Baseline and restored native browser runs each passed 50/50 tests; baseline and
restored host runs each passed 4/4. Original unchanged operators 10 and 11
were each killed by native assertions; neither was refused or survived. The
existing strict receipt producers and validators were used.

`run-focused-final.py` is the harness; `inventory.json` records the exact two
original names and indices, and `receipts/` preserves all four browser JSON
receipts. `raw-final.txt` contains the full summary. The outer process returned
exit code zero. Its trailing `final_focused_exit=` marker is blank because the
wrapper did not serialize the shell variable; no outcome is inferred from that
marker.

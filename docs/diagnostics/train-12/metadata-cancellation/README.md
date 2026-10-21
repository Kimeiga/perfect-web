# Playwright witness metadata and cancellation repair proof

Captured 2026-10-21 from source commit `9fe78da7` with Node `v22.21.1`.
The temporary `spikes/own-renderer/node_modules` symlink pointed to the
existing `/home/haki/code/pw-wt/train-12/spikes/own-renderer/node_modules`
cache and was removed after the run; the shared cache was not modified.

- `refusal.txt` and `refusal-receipts/` preserve the first bounded run. It
  exited nonzero at the 11-test baseline because that worktree could not
  resolve `@playwright/test`.
- `success.txt` and `success-receipts/` preserve the rerun with the existing
  dependency cache. It exited zero: 11/11 native baseline tests passed and
  all 9 intended mutants were killed by their exact witnesses. No browser app
  or server campaign is claimed.
- `scripts/tests/test_mutation_cancel_restores_source.py` exercises both
  runners' actual `main()` loops with stubbed native producers, delivers
  SIGTERM only after a real source mutation, and asserts source bytes and the
  prior signal handler are restored before exit.

The first refusal remains historical evidence. It is retained alongside the
successful run rather than overwritten.

## TodoMVC compatibility controls

`todomvc-compat-native/raw-run.txt` retains the 11-test native baseline and
all eight compatibility-guard mutations. `receipts/` preserves the native
inventory, frozen expected IDs, process statuses, stdout/stderr, structured
result reports, and validation receipts for the baseline and each control.
This is the native compatibility-guard campaign; it does not claim a Cypress
application/browser run. `catalog-validation.txt` checks the exact eight
verdicts with the unchanged `evidence_counts.VERDICT` parser and `names`
matcher, pairing each mutation description with its qualified witness from
the parallel `WITNESSES` list. All eight rows match uniquely; no parser rule
was changed.

The raw TodoMVC run is SHA256
`ed412aefff032760b815104252e625197aab7524dc05b57ec82267b9477d058f`.

## Isolated cancellation probe

Reproduce both bounded child probes with:

```sh
python3 -m unittest -v scripts.tests.test_mutation_cancel_restores_source
```

That test copies each source into a temporary directory, invokes the actual
runner `main()` in a child process, stubs native producer work, and delivers
SIGTERM after observing the changed copy. The parent checks exit 143 and exact
source restoration; the child receipt checks prior-handler restoration. The
parent reaps each child before its temporary directory is removed. The captured
child stdout and process-status records are in `todomvc-compat-native/cancellation-probe/`.
These probes test mutation cleanup and process ownership for the isolated
child only; they do not exercise descendant process groups or real native
producer shutdown.

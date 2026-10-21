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

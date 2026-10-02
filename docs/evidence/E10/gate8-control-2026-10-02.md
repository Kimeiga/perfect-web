# E7 gate 8 on this machine, 2026-10-02: the same at HEAD and at 6545029

Found while closing E10. `just e10-bench` stops at E7's gate 8 ("standard
interactions produce no long animation frame"), so it could not re-record E10
gate item 4 at HEAD. The question is whether the store page regressed since
`6545029`, the commit `docs/evidence/E10/bench.txt` was recorded at, or
whether the instrument is unstable on this machine.

## What the failing frame is

Each failing run sees one long animation frame of 52-63 ms (once 110 ms),
just over the API's 50 ms threshold. The test was temporarily edited to print
the entries, then restored byte for byte:

```text
{"duration":54.1,"blocking":3.865,"scripts":[]}
{"duration":56.4,"blocking":6.089,"scripts":[]}
```

No script is attributed, and blocking time is 4-6 ms. The frame is long, and
no page script ran long in it. `WindowServer`, the macOS compositor, was at
45% CPU during these runs.

## The control: 6545029, the recorded tree, run today

The browser side is unchanged between the two commits:
`spikes/own-renderer/public/`, `runtime/pw-resume*` and the gate's spec have
no diff. The handler module grew from 281 to 692 bytes (ADR-0058's exact-`Int`
guard). The whole tree at `6545029` was built in a temporary worktree sharing
the main target directory, then gate 8 was run 8 times:

```sh
git worktree add <tmp>/wt6545 6545029
# node_modules and target/ symlinked to the main checkout's
BUILD_ONLY=1 bash spikes/own-renderer/run.sh && cargo build -p pw-dev-server
cd spikes/own-renderer
PW_PERFORMANCE=1 PORT=3141 pnpm exec playwright test e2e/performance.spec.mjs \
  --project=chromium --workers=1 --reporter=list -g "gate 8"      # x8
```

| tree | runs with a long frame | worst frames (ms) |
|---|---|---|
| `6545029` (recorded as passing) | 4 of 8 | 57.5, 62.3, 62.9, 60.9 |
| HEAD, before the close commits | 6 of 8 | 55.5, 56.0, 53.2, 59.5, 52.0, 59.4 |
| `bff437c`, `just e10-close-bench 8` | 3 of 8 | 59.3, 64.1, 61.9 |

The recorded tree fails today at a similar rate. **This is not a regression in
E10.** The recorded zero in `docs/evidence/E10/bench.txt`, and E7's own record,
were each one passing sample from an instrument that is unstable on this
machine. This control was run by hand, so it is recorded here with its
commands; `just e10-close-bench` re-runs the HEAD half.

## What this does not decide

Whether gate 8's detector should count a frame no page script made long. That
is a change to a closed gate's instrument, and needs a ruling (EVIDENCE_LEDGER,
E7-G8). Until then, "standard interactions produce no long animation frame"
may not be stated as holding on this machine.

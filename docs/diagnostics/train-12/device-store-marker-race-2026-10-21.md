# Device-store marker test synchronization (2026-10-21)

This records the hosted Chromium failure for `device-store.spec.mjs` test “no request carries what the device keeps” and the narrow test-only correction. The hosted trace shows that pressing Enter returned before the device update was applied: the immediately following Count click began about 0.5 ms later, and reload began immediately after the click. In the pre-reload snapshot the marker remained in `#draft`, with no `#notes li` row; after reload the notes list was empty. The runtime applies the device update and localStorage write when its handler runs; the browser action returning alone did not establish that commit point.

Hosted source and artifacts:

- Failure log: `/mnt/c/Users/haki/code/perfect-web-handoff/tracks/evidence/root-followups/verify-39555014640/job-119072724520-failure.log`
- Chromium trace and error context: `/mnt/c/Users/haki/code/perfect-web-handoff/tracks/evidence/root-followups/verify-39555014640/browser-chromium-artifact/test-results/device-store-no-request-carries-what-the-device-keeps-chromium/`
- The trace's Enter return was at approximately `06:29:11.944 UTC`; Count click began `06:29:11.945`, completed `06:29:11.985`, and reload began at approximately `06:29:11.985`.

Five local pre-fix probes were observed against the built demo page. Each iteration had the same meaningful sequence: after Enter, the marker remained in the draft, rows were empty, `demo-notes@2` was null, and device notes were default; after Count, the draft was empty, the row contained the marker, storage held the JSON note, and the device state was read; after reload, the row and stored note remained. The requests observed were GETs and did not contain the marker. The probe script was `/tmp/device-store-marker-investigation/probe.mjs`; its five result lines were printed during the investigation but were not retained in a raw file. A later attempt to replay those five probes through a temporary Playwright spec aborted before browser launch because the host planner rejected the unregistered temporary spec with “missing or oversized collection receipt”; it produced no probe results and is not counted as a run.

The test now waits for the exact marker row and checks the persisted shape under `demo-notes@2` (`[{id: "0-<marker>", text: "<marker>", pinned: false}]`) before pressing Count. It then waits for `#presses` to read `1` before reload. The post-reload row check and request privacy assertions remain in place. No production runtime, retry, timeout, or test inventory changes were made.

Validation on source commit `e877236b3579d2b7abeab9cf1ac35679b2606519`, with this test-only source change:

- `pnpm exec playwright test e2e/device-store.spec.mjs --project=chromium --reporter=line`: Playwright reported **15 passed**. One shell wrapper ended with a quoting error after Playwright had completed; the Playwright run itself reported all 15 passed.
- `pnpm exec playwright test e2e/device-store.spec.mjs --grep 'no request carries what the device keeps' --reporter=line`: **3 passed** (Chromium, Firefox, WebKit).
- No focused original mutation was run locally; hosted whole-inventory verification remains due.

An earlier setup mistake omitted `BUILD_ONLY=1` from `run.sh`, which started the ordinary broad Playwright suite. It was stopped before any test result and is not evidence. A separate temporary-spec setup attempt also stopped before browser launch as described above. Neither is counted as validation.

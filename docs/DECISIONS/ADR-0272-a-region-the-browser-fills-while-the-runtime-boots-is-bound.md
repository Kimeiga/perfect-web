# ADR-0272: a region the browser fills while the runtime boots is bound

Status: accepted under the owner's delegation of 2026-10-02; found by the
notifications track on CI. Date: 2026-10-08. Milestone: E14.

## Context

- **A button in a streamed region did nothing.** Verify run 37748154579
  failed `stream.spec.mjs`'s "Chrome 150 and later fills a region itself"
  in Chromium: the region Chrome filled showed its recommendations, a press
  on one was made, and `#picked` still read "nothing" five seconds later.
  Here it failed once in 30 runs, and a copy that kept the runtime's
  record failed once in 60: the runtime had indexed 4 addresses, said "no
  element 1 for part 6" binding the region's buttons, recorded no region
  settling, and `#picked` read "nothing" fifteen seconds after the press.
- **The runtime boots across the network.** `attach` indexes the page,
  then waits for its boot decision (`/pw-resume.wasm`, `/pw-handlers`),
  then binds the handlers and watches the regions still pending
  (ADR-0148). A browser with the platform's out-of-order streaming, Chrome
  150 and later, applies a region's `<template for>` itself as it parses,
  and can do so during that wait. The region was indexed pending, bound
  from that index, and found settled when the runtime looked: it was never
  read again, and its buttons were bound to nothing.
- **A press that does nothing is the worst failure a page has**: "A silent
  failure here is the worst outcome available", the runtime says where a
  press fails; this one did not even fail. The page looked ready, and said
  nothing.

## Decision

1. **The runtime notes the stream regions pending when it first indexes
   the page.**
2. **One that settled while it booted is read again and bound** when it
   watches the regions, as one settling later is, and reported settled by
   the browser, which alone fills a region then.

## Acceptance

- **`e2e/stream.spec.mjs`, "a region Chrome fills while the runtime boots
  is bound"**, in the host's Chrome 150 or later: the recommendations
  slowed, so the runtime indexes the page first, and the runtime's boot
  held at the network until Chrome has filled the region, every time; the
  region reported settled by the browser, and a press on it answered.
  Without the re-reading it failed 4 runs of 4, "nothing".
- **The suite's Chrome test it came from**, 10 runs of 10, and the browser
  suite in three engines.
- **`scripts/stream_boot_mutations.py`, 3 mutants**: the regions pending at
  the first index not noted; one settled while booting not read again and
  bound; and not reported. Recorded by `just e14-stream-boot`.

## Not claimed

- **A press made before the runtime has booted** does nothing, as on any
  page: the runtime binds handlers when it is ready (`data-pw-ready`).
- **Other engines** apply a region's template when the runtime does, after
  it has booted, and meet no such race.

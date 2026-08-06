#!/usr/bin/env bash
# spike: layout-phase-scheduler — run and record evidence.
# Charter §14 Milestone 0 task 13.
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
SPIKE="$REPO_ROOT/spikes/layout-phase-scheduler"
EVIDENCE="$REPO_ROOT/docs/evidence/E0"
mkdir -p "$EVIDENCE"
cd "$SPIKE"

CHROME="${CHROME_PATH:-/Applications/Google Chrome.app/Contents/MacOS/Google Chrome}"
if [ ! -x "$CHROME" ]; then
    echo "spike-layout: Chrome not found at:" >&2
    echo "  $CHROME" >&2
    echo "Set CHROME_PATH to a Chromium-based browser binary." >&2
    exit 1
fi

OUT="$EVIDENCE/spike-layout-phase-scheduler.txt"

{
    echo "spike: layout-phase-scheduler"
    echo "generated: $(date -u +%Y-%m-%dT%H:%M:%SZ)"
    echo "node: $(node --version)"
    echo "chrome: $("$CHROME" --version 2>/dev/null || echo unknown)"
    echo "host: $(uname -sm) / $(sysctl -n machdep.cpu.brand_string 2>/dev/null)"
    echo "command: node spikes/layout-phase-scheduler/measure.mjs"
    echo
    echo "pages under test:"
    echo "  public/thrash.html     interleaves geometry reads with layout-invalidating writes"
    echo "  public/phased.html     batches all reads, plans, then commits all writes"
    echo "  public/observers.html  ResizeObserver + feedback loop + containment"
    echo
    echo "NOTE: headless, single machine, no CPU throttling, no sample distribution."
    echo "This is a BASELINE for detecting forced synchronous layout (charter §14 M0"
    echo "task 13), not a benchmark result (charter §18.5)."
    echo
    node measure.mjs
} | tee "$OUT"

rm -rf "$SPIKE/.chrome-profile"

echo
echo "spike-layout-phase-scheduler: evidence written to $OUT"

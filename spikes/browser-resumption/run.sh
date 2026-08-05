#!/usr/bin/env bash
# spike: browser-resumption — run and record evidence.
#
# Risk-retirement experiment RQ-1. Per the architect ruling:
#   "Engineering milestones describe what we have implemented. The risk-retirement
#    queue describes experiments that may use temporary dependencies to test
#    assumptions early. Passing an early experiment can retire a risk but cannot
#    close the corresponding implementation milestone."
#
# This tests whether MARKO's resumption is real. It does not close E7.
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
SPIKE="$REPO_ROOT/spikes/browser-resumption"
MARKO="$REPO_ROOT/spikes/marko-stream-resume"
EVIDENCE="$REPO_ROOT/docs/evidence/M0"
mkdir -p "$EVIDENCE"
cd "$SPIKE"

CHROME="${CHROME_PATH:-/Applications/Google Chrome.app/Contents/MacOS/Google Chrome}"
[ -x "$CHROME" ] || { echo "spike-browser-resumption: Chrome not found at $CHROME" >&2; exit 1; }
[ -f "$MARKO/dist/index.mjs" ] || { echo "building the marko app first..."; (cd "$MARKO" && pnpm build >/dev/null 2>&1); }

OUT="$EVIDENCE/spike-browser-resumption.txt"

{
    echo "spike: browser-resumption  (risk-retirement experiment RQ-1)"
    echo "generated: $(date -u +%Y-%m-%dT%H:%M:%SZ)"
    echo "node: $(node --version)"
    echo "chrome: $("$CHROME" --version 2>/dev/null || echo unknown)"
    echo "safari: $(defaults read /Applications/Safari.app/Contents/Info.plist CFBundleShortVersionString 2>/dev/null || echo unknown)"
    echo "under test: marko 6.3.32 / @marko/run 0.11.8 (spikes/marko-stream-resume)"
    echo "host: $(uname -sm) / $(sysctl -n machdep.cpu.brand_string 2>/dev/null)"
    echo "command: node spikes/browser-resumption/drive.mjs"
    echo
    echo "The checks below were PRE-REGISTERED before the experiment ran, together"
    echo "with the decision rule applied at the end. See README.md."
    echo
    node drive.mjs
} | tee "$OUT"

rm -rf "$SPIKE/.chrome-profile"

echo
echo "spike-browser-resumption: evidence written to $OUT"

#!/usr/bin/env bash
# E7 gate items 7-10, measured on the real store page through the own renderer.
#
# Alone and on Chromium only:
#
#   - alone, because a long-animation-frame measurement taken while three
#     engine families hammer the machine measures the machine;
#   - Chromium only, because the Long Animation Frame API is not implemented
#     elsewhere and a detector that silently reports zero where it is
#     unsupported is `docs/RISK_QUEUE.md` 22.
#
# Every figure here has a negative control in the same file, and each control
# runs through the same instrument in the same session — see the header of
# `e2e/performance.spec.mjs`.
set -euo pipefail
REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
SPIKE="$REPO_ROOT/spikes/own-renderer"
EVIDENCE="$REPO_ROOT/docs/evidence/E7"
PORT="${PORT:-3141}"

if [ ! -d "$SPIKE/dist" ]; then
  echo "dist/ is missing - run 'just spike-own-renderer' first" >&2
  exit 1
fi

cd "$SPIKE"
out="$(PW_PERFORMANCE=1 PORT="$PORT" pnpm exec playwright test e2e/performance.spec.mjs \
  --project=chromium --workers=1 --reporter=list 2>&1 | sed 's/\x1b\[[0-9;]*m//g')"
echo "$out"

mkdir -p "$EVIDENCE"
{
  echo "E7 gate items 7-10 — the own renderer's cost, measured"
  echo
  echo "produced by: just e7-performance"
  echo "engine:      chromium (Long Animation Frame API is Chromium-only)"
  echo "workers:     1 (a loaded machine measures the load)"
  echo
  echo "Every number below has a negative control in the same session:"
  echo "  the forced-layout detector is shown going red on a deliberate thrash;"
  echo "  the layout-read counter is shown counting a deliberate read;"
  echo "  the uncontained large menu is shown costing measurable time."
  echo
  echo "$out" | grep -E "^ *EVIDENCE" | sed 's/^ *EVIDENCE */  /'
  echo
  echo "$out" | tail -3
} > "$EVIDENCE/performance.txt"
echo
echo "evidence written to $EVIDENCE/performance.txt"

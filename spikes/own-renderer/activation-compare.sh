#!/usr/bin/env bash
# Activation cost of the browser runtime at a base commit and now: the same
# browser, the same machine, N samples each, one worker (charter §18.5: no
# one-run anecdotes).
#
# Why this exists: `just e10-bench` first compared one sample against E7's
# 2026-08-07 record (22.70 ms) and got 4.20 ms. That was a low first sample
# on a newer Chromium, and E10 had not touched the activation path. Eleven
# samples of each runtime overlap (median 15.9 against 16.4 ms). So the
# baseline is the old runtime measured now, never a number from another
# browser build.
#
#   usage: activation-compare.sh [BASE-COMMIT] [SAMPLES]
set -euo pipefail
REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
SPIKE="$REPO_ROOT/spikes/own-renderer"
BASE="${1:-83af93c}"
N="${2:-11}"

cd "$SPIKE"
tmp="$(mktemp -d)"
cp dist/pw-runtime.mjs "$tmp/now.mjs"
# The document's runtime is restored whatever happens: a swapped runtime left
# in dist/ would make the next browser run measure the wrong code.
trap 'cp "$tmp/now.mjs" dist/pw-runtime.mjs; rm -rf "$tmp"' EXIT
git -C "$REPO_ROOT" show "$BASE:spikes/own-renderer/public/pw-runtime.mjs" > "$tmp/before.mjs"

for which in before now; do
  cp "$tmp/$which.mjs" dist/pw-runtime.mjs
  samples="$(PW_PERFORMANCE=1 pnpm exec playwright test e2e/performance.spec.mjs \
    -g 'gate 7c' --project=chromium --workers=1 --repeat-each="$N" --reporter=list 2>&1 \
    | grep -oE 'activation-ms=[0-9.]+' | sed 's/activation-ms=//' | sort -n)"
  count="$(printf '%s\n' "$samples" | grep -c .)"
  if [ "$count" -ne "$N" ]; then
    echo "expected $N samples for $which, got $count" >&2
    exit 1
  fi
  median="$(printf '%s\n' "$samples" | awk '{a[NR]=$1} END {print a[int((NR+1)/2)]}')"
  label="$which ($([ "$which" = before ] && echo "$BASE" || git -C "$REPO_ROOT" rev-parse --short HEAD))"
  printf '  %-22s median %6s ms   samples: %s\n' "$label" "$median" "$(printf '%s ' $samples)"
done

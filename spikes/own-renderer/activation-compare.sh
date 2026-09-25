#!/usr/bin/env bash
# Activation cost of the browser runtime at a base commit and now: the same
# browser, the same machine, one worker (charter §18.5: no one-run anecdotes).
#
# INTERLEAVED: before, now, before, now, in ROUNDS rounds of PER samples each.
# Measured the plain way (all of one, then all of the other), three runs gave
# medians of 15.9/16.4, 15.7/15.5 and 10.3/15.7 ms. The machine's state drifts
# between blocks of samples by more than anything E10 changed. Interleaving
# spreads that drift over both sides instead of giving it to one.
#
# It also exists because `just e10-bench` first compared one sample against
# E7's 2026-08-07 record (22.70 ms) and printed 4.20 ms: a low first sample, on
# a newer Chromium, for a change that had not touched activation.
#
#   usage: activation-compare.sh [BASE-COMMIT] [ROUNDS] [PER]
set -euo pipefail
REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
SPIKE="$REPO_ROOT/spikes/own-renderer"
BASE="${1:-83af93c}"
ROUNDS="${2:-7}"
PER="${3:-3}"

cd "$SPIKE"
tmp="$(mktemp -d)"
cp dist/pw-runtime.mjs "$tmp/now.mjs"
# The document's runtime is restored whatever happens: a swapped runtime left
# in dist/ would make the next browser run measure the wrong code.
trap 'cp "$tmp/now.mjs" dist/pw-runtime.mjs; rm -rf "$tmp"' EXIT
git -C "$REPO_ROOT" show "$BASE:spikes/own-renderer/public/pw-runtime.mjs" > "$tmp/before.mjs"

sample() {
  cp "$tmp/$1.mjs" dist/pw-runtime.mjs
  PW_PERFORMANCE=1 pnpm exec playwright test e2e/performance.spec.mjs \
    -g 'gate 7c' --project=chromium --workers=1 --repeat-each="$PER" --reporter=list 2>&1 \
    | grep -oE 'activation-ms=[0-9.]+' | sed 's/activation-ms=//' >> "$tmp/$1.samples"
}
: > "$tmp/before.samples"
: > "$tmp/now.samples"
for _ in $(seq "$ROUNDS"); do
  sample before
  sample now
done

for which in before now; do
  count="$(grep -c . "$tmp/$which.samples")"
  if [ "$count" -ne $((ROUNDS * PER)) ]; then
    echo "expected $((ROUNDS * PER)) samples for $which, got $count" >&2
    exit 1
  fi
  samples="$(sort -n "$tmp/$which.samples")"
  median="$(printf '%s\n' "$samples" | awk '{a[NR]=$1} END {print a[int((NR+1)/2)]}')"
  q1="$(printf '%s\n' "$samples" | awk '{a[NR]=$1} END {print a[int((NR+3)/4)]}')"
  q3="$(printf '%s\n' "$samples" | awk '{a[NR]=$1} END {print a[int((3*NR+1)/4)]}')"
  label="$which ($([ "$which" = before ] && echo "$BASE" || git -C "$REPO_ROOT" rev-parse --short HEAD))"
  printf '  %-22s median %6s ms   interquartile %s-%s   n=%s   samples: %s\n' \
    "$label" "$median" "$q1" "$q3" "$count" "$(printf '%s ' $samples)"
done

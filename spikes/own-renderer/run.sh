#!/usr/bin/env bash
# E7 task 2 — the own renderer, in a browser.
#
# Charter §14 M7 gate 1: a purely static `.pw` page renders entirely through
# this implementation. No Marko, no application JavaScript, no runtime.
#
#   .pw → pw check → pw emit-template → pw-render → HTML → a browser
#
# The chain is printed rather than described, so "Marko was not involved" is
# something a reader verifies from the commands rather than from a sentence.
set -euo pipefail
REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
SPIKE="$REPO_ROOT/spikes/own-renderer"
OUT="$SPIKE/dist"
EVIDENCE="$REPO_ROOT/docs/evidence/E7"
PORT="${PORT:-3141}"

mkdir -p "$EVIDENCE"
rm -rf "$OUT"

PAGES=(
  "$REPO_ROOT/examples/hello-static/app.pw"
  "$REPO_ROOT/examples/render/tricky.pw"
)

echo "== 1. the pages are checked before they are rendered =="
cargo run --quiet -p pw-cli --manifest-path "$REPO_ROOT/Cargo.toml" -- \
  check --plain "${PAGES[@]}"

echo
echo "== 2. checked template IR =="
cargo run --quiet -p pw-cli --manifest-path "$REPO_ROOT/Cargo.toml" -- \
  emit-template "${PAGES[@]}" > "$SPIKE/template-ir.json"
echo "   $(wc -c < "$SPIKE/template-ir.json" | tr -d ' ') bytes"

echo
echo "== 3. HTML, by pw-render =="
cargo run --quiet -p pw-render --manifest-path "$REPO_ROOT/Cargo.toml" --bin pw-render -- \
  --out "$OUT" < "$SPIKE/template-ir.json"

echo
echo "== 4. nothing in the output is a script =="
if grep -rql "<script" "$OUT"; then
  echo "FAIL: a rendered page contains a script"
  exit 1
fi
echo "   0 script tags across $(ls "$OUT" | wc -l | tr -d ' ') page(s)"

echo
echo "== 5. determinism: the same IR renders to the same bytes =="
cargo run --quiet -p pw-render --manifest-path "$REPO_ROOT/Cargo.toml" --bin pw-render -- \
  --out "$OUT.again" < "$SPIKE/template-ir.json"
if ! diff -r "$OUT" "$OUT.again" > /dev/null; then
  echo "FAIL: two renders of one IR differ"
  exit 1
fi
rm -rf "$OUT.again"
echo "   byte-identical"

echo
echo "== 6. the browser's parsed tree =="
cd "$SPIKE"
if [ ! -d node_modules ]; then
  pnpm install --silent
fi
# `pnpm exec`, not `npx`: `npx` resolves playwright from its own cache and then
# cannot find the browsers this workspace installed, so every engine but the
# one already present reports "browser not installed" — a harness failure that
# reads exactly like a renderer failure.
PORT="$PORT" pnpm exec playwright test --reporter=list 2>&1 | sed 's/\x1b\[[0-9;]*m//g' | tail -40

{
  echo "E7 task 2 — the own renderer"
  echo
  echo "chain:  .pw → pw check → pw emit-template → pw-render → HTML → browser"
  echo "        Marko is not in it."
  echo
  echo "pages:  $(ls "$OUT" | tr '\n' ' ')"
  echo "script tags in the output: 0"
  echo "two renders of one IR: byte-identical"
} > "$EVIDENCE/own-renderer.txt"
echo
echo "evidence written to $EVIDENCE/own-renderer.txt"

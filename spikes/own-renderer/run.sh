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

# The static pages: no values, no runtime, no manifest.
PAGES=(
  "$REPO_ROOT/examples/hello-static/app.pw"
  "$REPO_ROOT/examples/render/tricky.pw"
)

# The store page: the real E4/E5 demo, rendered by the own renderer. Its
# program is the whole library, because the template IR needs the handler
# identity the resume artifacts derived and that is a whole-program answer.
STORE=(
  "$REPO_ROOT/examples/domain.pw"
  "$REPO_ROOT/examples/lib/"*.pw
  "$REPO_ROOT/examples/store/app.pw"
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
echo "== 3b. the store page, with its parts manifest and the E7V decision =="
cargo run --quiet -p pw-cli --manifest-path "$REPO_ROOT/Cargo.toml" -- \
  emit-template "${STORE[@]}" > "$SPIKE/store-ir.json"
cargo run --quiet -p pw-render --manifest-path "$REPO_ROOT/Cargo.toml" --bin pw-render -- \
  --out "$OUT" --values "$SPIKE/store-values.json" --resume "$SPIKE/store-resume.json" \
  --runtime /pw-runtime.mjs < "$SPIKE/store-ir.json"
cp "$SPIKE/public/pw-runtime.mjs" "$OUT/"
cargo build --quiet --manifest-path "$REPO_ROOT/Cargo.toml" \
  -p pw-resume-wasm --target wasm32-unknown-unknown --release
cp "$REPO_ROOT/target/wasm32-unknown-unknown/release/pw_resume_wasm.wasm" "$OUT/pw-resume.wasm"
echo "   $(ls "$OUT" | tr '\n' ' ')"

echo
echo "== 4. the STATIC pages ship no script =="
for page in HelloStatic Tricky; do
  if grep -ql "<script" "$OUT/$page.html"; then
    echo "FAIL: $page contains a script"
    exit 1
  fi
done
echo "   0 script tags in HelloStatic and Tricky"
echo "   StorePage carries $(grep -c "<script" "$OUT/StorePage.html") script element(s):"
echo "     the parts manifest, and the runtime that reads it"

echo
echo "== 5. determinism: the same IR renders to the same bytes =="
cargo run --quiet -p pw-render --manifest-path "$REPO_ROOT/Cargo.toml" --bin pw-render -- \
  --out "$OUT.again" < "$SPIKE/template-ir.json"
rm -f "$OUT.again/StorePage.html"
if ! diff "$OUT/HelloStatic.html" "$OUT.again/HelloStatic.html" > /dev/null \
   || ! diff "$OUT/Tricky.html" "$OUT.again/Tricky.html" > /dev/null; then
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
  echo "E7-2 and E7-R — the own renderer, and the store page through it"
  echo
  echo "chain:  .pw → pw check → pw emit-template → pw-render → HTML → browser"
  echo "        Marko is not in it."
  echo
  echo "pages:  $(ls "$OUT" | tr '\n' ' ')"
  echo
  echo "STATIC ROUTES"
  echo "  HelloStatic, Tricky: 0 script tags, 0 runtime, usable with JS off"
  echo "  two renders of one IR: byte-identical"
  echo
  echo "THE STORE PAGE — E7-R's vertical slice"
  echo "  server renderer → HTML with only the required anchors"
  echo "  → parts manifest → decide() → Authorised → handler attaches"
  echo "  → click Add → command → only cart-related PartIds update"
  echo
  echo "  12 assertions, 3 engine families:"
  echo "    no Marko participates in this route"
  echo "    only dynamic regions carry identity markup"
  echo "    the page is readable with JavaScript disabled"
  echo "    a compatible manifest authorises; the handler attaches"
  echo "    an INCOMPATIBLE manifest is refused and nothing attaches"
  echo "    the menu keeps node identity across the update"
  echo "    the cart element survives its content changing, twice"
  echo "    focus survives the update"
  echo "    only the cart part id updates"
  echo "    replacing a menu node makes the identity assertion RED"
  echo "    handler bytes are still eager — the E7-L gap, recorded"
} > "$EVIDENCE/own-renderer.txt"
echo
echo "evidence written to $EVIDENCE/own-renderer.txt"

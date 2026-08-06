#!/usr/bin/env bash
# spike: pw-to-marko — charter §14 M3, the Marko rendering adapter.
#
# Generates Marko routes from `.pw` with `pw emit-marko`, builds them with the
# pinned toolchain, and measures the bytes each route ships. ADR-0017 states
# what this is and is not evidence for; the short version is that streaming and
# resumption remain MARKO's behaviours, measured through `pw`.
#
# Nothing under src/routes is authored: it is deleted and regenerated here, and
# it is gitignored so it cannot drift into being the source of truth.
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
SPIKE="$REPO_ROOT/spikes/pw-to-marko"
EVIDENCE="$REPO_ROOT/docs/evidence/E3"
mkdir -p "$EVIDENCE"

OUT="$EVIDENCE/spike-pw-to-marko.txt"

{
    echo "spike: pw-to-marko"
    echo "generated: $(date -u +%Y-%m-%dT%H:%M:%SZ)"
    echo "host: $(uname -sm)"
    echo "node: $(node --version)"
    echo "commands:"
    echo "  pw emit-marko examples/{hello-static,counter}/app.pw --out spikes/pw-to-marko/src/routes/<route>"
    echo "  pnpm --filter spike-pw-to-marko build"
    echo "  node spikes/pw-to-marko/measure.mjs"
    echo

    echo "===================== 1. GENERATE FROM .pw ========================="
    # Deleted first: a stale template from a previous run would be measured as
    # if it were current output.
    rm -rf "$SPIKE/src/routes"
    mkdir -p "$SPIKE/src/routes/static" "$SPIKE/src/routes/counter"
    cd "$REPO_ROOT"
    cargo run --quiet -p pw-cli -- emit-marko examples/hello-static/app.pw \
        --out "$SPIKE/src/routes/static"
    cargo run --quiet -p pw-cli -- emit-marko examples/counter/app.pw \
        --out "$SPIKE/src/routes/counter"

    # @marko/run routes are `+page.marko`; the adapter emits one file per view.
    mv "$SPIKE/src/routes/static/HelloStatic.marko" "$SPIKE/src/routes/static/+page.marko"
    mv "$SPIKE/src/routes/counter/Counter.marko" "$SPIKE/src/routes/counter/+page.marko"
    echo
    echo "--- generated: static ---"
    cat "$SPIKE/src/routes/static/+page.marko"
    echo "--- generated: counter ---"
    cat "$SPIKE/src/routes/counter/+page.marko"
    echo

    echo "===================== 2. BUILD ====================================="
    cd "$SPIKE"
    pnpm build 2>&1 | sed 's/\x1b\[[0-9;]*m//g' | grep -vE '^\s*$'
    echo

    echo "===================== 3. MEASURE ==================================="
    node measure.mjs
} 2>&1 | tee "$OUT"

echo
echo "evidence written to ${OUT#"$REPO_ROOT"/}"

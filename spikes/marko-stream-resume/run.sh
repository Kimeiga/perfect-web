#!/usr/bin/env bash
# spike: marko-stream-resume — build, measure, and record evidence.
# Charter §14 Milestone 0 task 9.
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
SPIKE="$REPO_ROOT/spikes/marko-stream-resume"
EVIDENCE="$REPO_ROOT/docs/evidence/E0"
mkdir -p "$EVIDENCE"
cd "$SPIKE"

command -v node >/dev/null || { echo "node not found" >&2; exit 1; }
command -v pnpm >/dev/null || { echo "pnpm not found — run: corepack enable pnpm" >&2; exit 1; }

if [ ! -d "$REPO_ROOT/node_modules" ]; then
    echo "node_modules missing — run: just bootstrap" >&2
    exit 1
fi

OUT="$EVIDENCE/spike-marko-stream-resume.txt"

{
    echo "spike: marko-stream-resume"
    echo "generated: $(date -u +%Y-%m-%dT%H:%M:%SZ)"
    echo "node: $(node --version)   pnpm: $(pnpm --version)"
    echo "marko: 6.3.32   @marko/run: 0.11.8   @marko/run-adapter-node: 2.0.6   vite: 8.2.0"
    echo "host: $(uname -sm)"
    echo "commands:"
    echo "  pnpm --filter spike-marko-stream-resume build"
    echo "  node spikes/marko-stream-resume/measure.mjs"
    echo
    echo "routes:"
    echo "  /static         no state, no handlers, no await   -> tests the 0-JS budget"
    echo "  /stream         two delayed subtrees (400/1200ms) -> tests streaming"
    echo "  /counter        one button in a small document    -> tests resumption"
    echo "  /counter-large  same button, 200 extra inert rows -> tests payload scaling"
    echo

    echo "============================ BUILD ================================="
    pnpm build 2>&1 | sed 's/\x1b\[[0-9;]*m//g' | grep -vE '^\s*$'
    echo

    echo "========================= MEASUREMENTS ============================="
    node measure.mjs 2>&1
} | tee "$OUT"

echo
echo "spike-marko-stream-resume: evidence written to $OUT"

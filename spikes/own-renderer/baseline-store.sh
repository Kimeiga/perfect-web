#!/usr/bin/env bash
# The benchmark's store (ADR-0156), built into `dist-baseline`: what E14-A's
# contract runs against beside the Next.js and SvelteKit baselines, and what
# every task's patches start from. The canonical store, `examples`, grows
# toward charter §15; this one is frozen, as the other two stacks' are.
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
SPIKE="$REPO_ROOT/spikes/own-renderer"

WORK="$(mktemp -d)"
trap 'rm -rf "$WORK"' EXIT

# Its own IR files, so the canonical store's, which the server's tests read,
# stay as they are.
PW_SOURCES="$REPO_ROOT/benchmarks/baselines/pleris" PW_OUT="$SPIKE/dist-baseline" \
  PW_STORE_IR="$WORK/store-ir.json" PW_TEMPLATE_IR="$WORK/template-ir.json" \
  BUILD_ONLY=1 bash "$SPIKE/run.sh" > /dev/null
echo "built the benchmark's store into $SPIKE/dist-baseline"

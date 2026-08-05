#!/usr/bin/env bash
# spike: compiler-diagnostic — run + record evidence.
# Charter §14 Milestone 0 task 11.
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
EVIDENCE="$REPO_ROOT/docs/evidence/M0"
mkdir -p "$EVIDENCE"
cd "$REPO_ROOT"

OUT="$EVIDENCE/spike-compiler-diagnostic.txt"

{
    echo "spike: compiler-diagnostic"
    echo "generated: $(date -u +%Y-%m-%dT%H:%M:%SZ)"
    echo "rustc: $(rustc --version)"
    echo "host: $(uname -sm) / $(sw_vers -productVersion 2>/dev/null || true)"
    echo "commands:"
    echo "  cargo test -p spike-compiler-diagnostic"
    echo "  cargo run -p spike-compiler-diagnostic -- --demo --plain"
    echo "  cargo run -p spike-compiler-diagnostic -- --demo --plain --explain"
    echo
    echo "=============================== TESTS ==============================="
    cargo test --quiet -p spike-compiler-diagnostic 2>&1
    echo
    echo "========================= DEMO DIAGNOSTICS =========================="
    cargo run --quiet -p spike-compiler-diagnostic -- --demo --plain 2>&1
    echo
    echo "======================= EXPLAIN (pw explain) ========================"
    cargo run --quiet -p spike-compiler-diagnostic -- --demo --plain --explain 2>&1
} | tee "$OUT"

echo
echo "spike-compiler-diagnostic: evidence written to $OUT"

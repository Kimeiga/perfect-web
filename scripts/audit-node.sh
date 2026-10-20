#!/usr/bin/env bash
# Charter section 3.6; ADR-0371: incomplete or failed audits never pass.
# Stdin mode validates a supplied report; normal mode also holds the producer
# to its exit status. Documented advisories may pass, unavailable scans cannot.
set -euo pipefail
REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
ALLOW="$REPO_ROOT/tools/node-audit-allow.txt"

if [ "${1:-}" = "-" ]; then
    python3 "$REPO_ROOT/scripts/audit_node.py" --allow "$ALLOW"
else
    cd "$REPO_ROOT"
    status=0
    report="$(pnpm audit --json)" || status=$?
    printf '%s' "$report" | python3 "$REPO_ROOT/scripts/audit_node.py" --allow "$ALLOW" --producer-exit "$status"
fi

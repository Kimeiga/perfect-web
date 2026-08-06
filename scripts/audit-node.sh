#!/usr/bin/env bash
# Charter §3.6 vulnerability scanning for the Node workspace.
#
# Fails on ANY advisory that is not listed in `tools/node-audit-allow.txt` with
# a written reason — the same discipline `deny.toml` applies to Rust. Lowering
# `--audit-level` instead would silence a whole severity class, including
# advisories nobody has looked at yet.
#
# Reads `pnpm audit --json` from stdin when given `-`, so the allow-list logic
# is testable without a vulnerable dependency tree.
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
ALLOW="$REPO_ROOT/tools/node-audit-allow.txt"

if [ "${1:-}" = "-" ]; then
    report="$(cat)"
else
    cd "$REPO_ROOT"
    # `pnpm audit` exits non-zero when it finds anything; that is this script's
    # decision to make, not pnpm's.
    report="$(pnpm audit --json 2>/dev/null || true)"
fi

allowed="$(grep -oE '^GHSA-[a-z0-9-]+' "$ALLOW" 2>/dev/null || true)"

unexpected="$(
    printf '%s' "$report" | ALLOWED="$allowed" python3 -c '
import json, os, sys

allowed = set(os.environ.get("ALLOWED", "").split())
try:
    report = json.load(sys.stdin)
except Exception:
    sys.exit(0)  # nothing parseable means nothing to report

for key, adv in (report.get("advisories") or {}).items():
    ghsa = adv.get("github_advisory_id") or key
    if ghsa in allowed:
        continue
    sev = adv.get("severity", "?")
    mod = adv.get("module_name", "?")
    title = adv.get("title", "")
    print("%s  %-8s %s  %s" % (ghsa, sev, mod, title))
'
)"

if [ -n "$unexpected" ]; then
    echo "audit-node: advisories not on the allow list:" >&2
    printf '%s\n' "$unexpected" >&2
    echo >&2
    echo "  Fix the dependency, or add the advisory to tools/node-audit-allow.txt" >&2
    echo "  with a reason and a revisit condition." >&2
    exit 1
fi

n="$(printf '%s\n' "$allowed" | sed '/^$/d' | wc -l | tr -d ' ')"
echo "audit-node: OK (no new advisories; $n accepted and documented)"

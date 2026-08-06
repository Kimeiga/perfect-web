#!/usr/bin/env bash
# Fails when two tracked paths differ only by case.
#
# Charter §13.5. macOS is case-insensitive by default, so a repository developed
# only on a Mac can contain `Foo.pw` and `foo.pw` as one file locally and two on
# Linux — where the checkout then fails or silently keeps one. The Mac cannot
# even create the collision to demonstrate it, which is precisely why the check
# reads the *index* rather than the filesystem.
#
# Reads paths from stdin when given `-`, so the detection logic is testable
# without needing a case-sensitive filesystem to construct a collision on.
set -euo pipefail

if [ "${1:-}" = "-" ]; then
    paths="$(cat)"
else
    cd "$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
    paths="$(git ls-files)"
fi

# Group by lowercased path; any group with more than one distinct member is a
# collision. `sort -u` first so the same path listed twice is not a false alarm.
collisions="$(
    printf '%s\n' "$paths" |
        sed '/^$/d' |
        sort -u |
        awk '{ lower = tolower($0); seen[lower] = seen[lower] "\n    " $0; count[lower]++ }
             END { for (k in count) if (count[k] > 1) printf "  %s%s\n", k, seen[k] }'
)"

if [ -n "$collisions" ]; then
    echo "case-check: paths differing only by case:" >&2
    printf '%s\n' "$collisions" >&2
    echo >&2
    echo "  These are one file on macOS and two on Linux. Rename one." >&2
    exit 1
fi

echo "case-check: OK ($(printf '%s\n' "$paths" | sed '/^$/d' | wc -l | tr -d ' ') paths)"

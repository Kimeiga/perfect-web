#!/usr/bin/env bash
# TodoMVC's official Cypress suite, unchanged, against TodoMVC in Pleris
# (track `todomvc`, W7): the acceptance of its first milestone.
#
# It serves `spikes/own-renderer/dist-todomvc` (built by `todomvc.sh`) on a
# host of its own, PORT+143 (the Playwright suite's are PORT+140 to PORT+142),
# and runs `cypress/e2e/spec.cy.js` as the repository ships it
# (`UPSTREAM.md` holds each file to its upstream blob):
#
#     cypress run --env framework=page/todomvc.app.Todos --config baseUrl=http://127.0.0.1:<port>
#
# The suite visits `baseUrl + '/' + framework`: the name is the page's own
# path on the host. (The root, `.`, was tried: Cypress asks for `/.`, which no
# host here serves as `/`.) The name is in none of the suite's tables.
#
# Cypress's binary is not in the repository. Where it is not installed, this
# says so and runs nothing (exit 0): `pnpm exec cypress install` fetches it,
# into Cypress's own cache. Cypress needs a display: it starts Xvfb itself
# where none is set, so the one WSLg or a desktop gives is left out of its
# environment, and the run is the same on a machine with none.
#
# **The diagnostic** (`DIAGNOSTIC=1`, not the acceptance): the same unchanged
# files, run as an app the suite's own tables treat as keeping nothing in
# `localStorage` (`noLocalStorageCheck`: `puremvc`, which has no known issues
# either), reached through the address's fragment, which the page ignores:
#
#     cypress run --env framework=puremvc --config baseUrl=http://127.0.0.1:<port>/page/todomvc.app.Todos#
#
# The acceptance's own name is in none of the tables, so each test that asks
# `localStorage` for the todos fails, and the first test after a failed one
# fails in its hook (the failed tests' polling of a storage that is gone
# throws), which skips every test after it. The diagnostic shows what the rest
# of the suite says of this app.
#
# The suite's output goes to $OUT (default /tmp/pw-todomvc-cypress.txt, or
# /tmp/pw-todomvc-cypress-diagnostic.txt), and its summary is printed.
set -euo pipefail

HERE="$(cd "$(dirname "$0")" && pwd)"
REPO_ROOT="$(cd "$HERE/../.." && pwd)"
SPIKE="$REPO_ROOT/spikes/own-renderer"
DIST="$SPIKE/dist-todomvc"
PORT="$(( ${PORT:-3141} + 143 ))"
if [ "${DIAGNOSTIC:-}" = 1 ]; then
  FRAMEWORK="puremvc"
  BASE="http://127.0.0.1:$PORT/page/todomvc.app.Todos#"
  OUT="${OUT:-/tmp/pw-todomvc-cypress-diagnostic.txt}"
  echo "cypress: the DIAGNOSTIC run (framework $FRAMEWORK, which the suite takes to keep nothing in localStorage): not the acceptance"
else
  FRAMEWORK="page/todomvc.app.Todos"
  BASE="http://127.0.0.1:$PORT"
  OUT="${OUT:-/tmp/pw-todomvc-cypress.txt}"
fi

cd "$HERE"

# Each vendored file is the upstream's, byte for byte (UPSTREAM.md).
python3 "$REPO_ROOT/scripts/todomvc_cypress_vendored.py"

if ! pnpm exec cypress verify > /dev/null 2>&1; then
  echo "cypress: the binary is not installed on this machine, so the official suite was not run"
  echo "         (pnpm exec cypress install, in spikes/todomvc-cypress; UPSTREAM.md says what it fetches)"
  exit 0
fi
echo "cypress: $(pnpm exec cypress --version | tr '\n' ' ')"

if [ ! -d "$DIST/build" ]; then
  echo "run.sh: $DIST is not built; run spikes/own-renderer/todomvc.sh" >&2
  exit 1
fi

# The host: TodoMVC's build, in memory, on a port of its own.
(cd "$SPIKE" && PORT="$PORT" PW_FEED_DATABASE_URL= PW_STORE_DATABASE_URL= \
  "$REPO_ROOT/target/debug/pw-dev-server" dist-todomvc > /tmp/pw-todomvc-cypress-host.log 2>&1) &
HOST=$!
trap 'kill "$HOST" 2>/dev/null || true' EXIT
for _ in $(seq 1 100); do
  if curl -fsS -o /dev/null "http://127.0.0.1:$PORT/"; then break; fi
  sleep 0.2
done
curl -fsS -o /dev/null "http://127.0.0.1:$PORT/" || { echo "run.sh: the host did not start" >&2; exit 1; }

set +e
env -u DISPLAY -u WAYLAND_DISPLAY pnpm exec cypress run \
  --env framework="$FRAMEWORK" \
  --config "baseUrl=$BASE" \
  --reporter spec > "$OUT" 2>&1
CODE=$?
set -e

# Each test's line, why each failed (the first line of its error), and
# Cypress's own table of the counts.
sed 's/\x1b\[[0-9;]*[A-Za-z]//g' "$OUT" | grep -E "^ +(✓|✔|[0-9]+\)|-) |^ +(AssertionError|CypressError|Error|SecurityError)|│ (Tests|Passing|Failing|Pending|Skipped):" || true
echo "cypress exit status: $CODE"
exit 0

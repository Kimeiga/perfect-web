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
# **It does not skip.** Where Cypress cannot run (no Xvfb, no binary, a build
# that is missing, a host that does not start) this exits 1 and says why, and
# so does a run that printed no results: the evidence recipe (`just
# e14-todomvc`) must never record a pass for a suite that did not run. The
# verdict is `scripts/todomvc_cypress_result.py`'s, read from what Cypress
# printed: this exits with it. (`pnpm exec cypress install`, in this
# directory, fetches Cypress's binary into its own cache; `UPSTREAM.md` says
# what.) Cypress starts Xvfb itself where no display is set, and the one WSLg
# or a desktop gives is left out of its environment, so the run is the same on
# a machine with none.
#
# **The diagnostic** (`DIAGNOSTIC=1`, not the acceptance): the same unchanged
# files, run as an app the suite's own tables treat as keeping nothing in
# `localStorage` (`noLocalStorageCheck`: `puremvc`, which has no known issues
# either), reached through the address's fragment, which the page ignores:
#
#     cypress run --env framework=puremvc --config baseUrl=http://127.0.0.1:<port>/page/todomvc.app.Todos#
#
# The acceptance's own name is in none of the tables, so each test that asks
# `localStorage` for the todos fails by design, and a failed test's polling
# loop makes a later hook fail, which skips the rest of the suite. The
# diagnostic shows what the rest of each test says of this app.
#
# Cypress's output goes to $OUT (default /tmp/pw-todomvc-cypress.txt, or
# /tmp/pw-todomvc-cypress-diagnostic.txt), with the browser's own console
# (ELECTRON_ENABLE_LOGGING), which is where the failed tests' polling loops
# record why a hook fails; the report goes to the standard output.
#
# Environment: PORT (the base, default 3141), DIAGNOSTIC, OUT, PW_DEV_SERVER
# (the host's binary, default target/debug/pw-dev-server), TODOMVC_DIST (the
# built app: a name in spikes/own-renderer or a path, default dist-todomvc),
# XVFB_BIN (default Xvfb). The last three are for the tests of this script
# (scripts/tests/test_todomvc_cypress.py), which run it with a Cypress that
# prints what the test says.
set -euo pipefail

HERE="$(cd "$(dirname "$0")" && pwd)"
REPO_ROOT="$(cd "$HERE/../.." && pwd)"
SPIKE="$REPO_ROOT/spikes/own-renderer"
DIST_NAME="${TODOMVC_DIST:-dist-todomvc}"
case "$DIST_NAME" in
  /*) DIST="$DIST_NAME" ;;
  *) DIST="$SPIKE/$DIST_NAME" ;;
esac
SERVER="${PW_DEV_SERVER:-$REPO_ROOT/target/debug/pw-dev-server}"
XVFB="${XVFB_BIN:-Xvfb}"
PORT="$(( ${PORT:-3141} + 143 ))"
if [ "${DIAGNOSTIC:-}" = 1 ]; then
  MODE=diagnostic
  FRAMEWORK="puremvc"
  BASE="http://127.0.0.1:$PORT/page/todomvc.app.Todos#"
  OUT="${OUT:-/tmp/pw-todomvc-cypress-diagnostic.txt}"
  echo "cypress: the DIAGNOSTIC run (framework $FRAMEWORK, which the suite takes to keep nothing in localStorage): not the acceptance"
else
  MODE=acceptance
  FRAMEWORK="page/todomvc.app.Todos"
  BASE="http://127.0.0.1:$PORT"
  OUT="${OUT:-/tmp/pw-todomvc-cypress.txt}"
fi

# A suite that cannot run is a failure, never a skip.
cannot() {
  echo "run.sh: Cypress cannot run: $*"
  exit 1
}

cd "$HERE"

# Each vendored file is the upstream's, byte for byte (UPSTREAM.md).
python3 "$REPO_ROOT/scripts/todomvc_cypress_vendored.py"

command -v "$XVFB" > /dev/null 2>&1 \
  || cannot "Xvfb is not installed (Cypress needs a display and starts Xvfb itself; apt: xvfb)"
pnpm exec cypress verify > /dev/null 2>&1 \
  || cannot "its binary is not installed, or does not start (pnpm exec cypress install, in spikes/todomvc-cypress; UPSTREAM.md says what it fetches)"
echo "cypress: $(pnpm exec cypress --version | tr '\n' ' ')"
[ -d "$DIST/build" ] || cannot "$DIST is not built (bash spikes/own-renderer/todomvc.sh)"
[ -x "$SERVER" ] || cannot "the host $SERVER is not built (cargo build -p pw-dev-server)"

# The host: TodoMVC's build, in memory, on a port of its own.
(cd "$SPIKE" && PORT="$PORT" PW_FEED_DATABASE_URL= PW_STORE_DATABASE_URL= \
  "$SERVER" "$DIST" > /tmp/pw-todomvc-cypress-host.log 2>&1) &
HOST=$!
trap 'kill "$HOST" 2>/dev/null || true' EXIT
for _ in $(seq 1 100); do
  if curl -fs -o /dev/null "http://127.0.0.1:$PORT/"; then break; fi
  sleep 0.2
done
curl -fs -o /dev/null "http://127.0.0.1:$PORT/" \
  || cannot "the host did not start (/tmp/pw-todomvc-cypress-host.log)"

set +e
env -u DISPLAY -u WAYLAND_DISPLAY ELECTRON_ENABLE_LOGGING=1 \
  pnpm exec cypress run \
  --env framework="$FRAMEWORK" \
  --config "baseUrl=$BASE" \
  --reporter spec > "$OUT" 2>&1
CODE=$?
set -e

# The verdict: this exits with it. A run with no results table, or with an
# outcome the app's design does not give, fails here.
python3 "$REPO_ROOT/scripts/todomvc_cypress_result.py" "$MODE" "$OUT" "$CODE"

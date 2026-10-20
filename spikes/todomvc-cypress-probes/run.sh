#!/usr/bin/env bash
# **Two facts of the Cypress TodoMVC's own suite is run with** (track
# `device-store`, ADR-XXXX), each shown with no app at all, by the Cypress
# `spikes/todomvc-cypress` pins (15.21.1, the version upstream's `^15.14.2`
# resolves to):
#
# - `spy/`: a spy has `resetHistory` and no `reset`, so the spec's
#   `checkItemSaved` (`cy.wrap(spy).invoke('reset')`) errs for every app it
#   checks: the run's tests 4 and 9, and 10 and 11, which a failed hook skips;
# - `alias/`: an alias a custom command ending in `.then()` made is not read
#   again after `cy.reload()`, so the spec's persistence test (24) errs for
#   every app it checks; an alias `cy.get` made is, the control.
#
# Each probe's tests pass where what they name holds; this exits 0 only when
# every test of both passed, and 1 otherwise, saying which. It does not skip:
# where Cypress cannot run (no Xvfb, no binary) it exits 1 and says why.
#
# Environment: PORT (the base, default 3141; the static page is served on
# PORT+158), OUT (Cypress's output, default /tmp/pw-todomvc-cypress-probes).
set -euo pipefail

HERE="$(cd "$(dirname "$0")" && pwd)"
REPO_ROOT="$(cd "$HERE/../.." && pwd)"
CYPRESS_DIR="$REPO_ROOT/spikes/todomvc-cypress"
OUT="${OUT:-/tmp/pw-todomvc-cypress-probes}"
PROBE_PORT="$(( ${PORT:-3141} + 158 ))"
export PROBE_PORT
mkdir -p "$OUT"

cannot() {
  echo "probes: Cypress cannot run: $*"
  exit 1
}

cd "$CYPRESS_DIR"
command -v Xvfb > /dev/null 2>&1 \
  || cannot "Xvfb is not installed (Cypress needs a display and starts Xvfb itself; apt: xvfb)"
pnpm exec cypress verify > /dev/null 2>&1 \
  || cannot "its binary is not installed, or does not start (pnpm exec cypress install, in spikes/todomvc-cypress)"
echo "probes: $(pnpm exec cypress --version | tr '\n' ' ')"

# The static page, for the alias probe.
python3 -m http.server "$PROBE_PORT" --bind 127.0.0.1 --directory "$HERE/alias/site" \
  > "$OUT/site.log" 2>&1 &
SITE=$!
trap 'kill "$SITE" 2>/dev/null || true' EXIT
for _ in $(seq 1 50); do
  if curl -fs -o /dev/null "http://127.0.0.1:$PROBE_PORT/"; then break; fi
  sleep 0.2
done
curl -fs -o /dev/null "http://127.0.0.1:$PROBE_PORT/" \
  || cannot "the static page is not served ($OUT/site.log)"

failed=0
for probe in spy alias; do
  set +e
  env -u DISPLAY -u WAYLAND_DISPLAY ELECTRON_ENABLE_LOGGING=1 \
    pnpm exec cypress run --project "$HERE/$probe" --reporter spec \
    > "$OUT/$probe.txt" 2>&1
  code=$?
  set -e
  plain="$(sed 's/\x1b\[[0-9;]*[A-Za-z]//g' "$OUT/$probe.txt")"
  echo
  echo "== $probe (exit $code)"
  # Each test's line, as the spec reporter prints it, and the spy's own words.
  grep -E "^ +(✓|[0-9]+\)) " <<< "$plain" | sed 's/ *([0-9]*ms)$//' || true
  grep -o "PROBE cypress [^\"]*" <<< "$plain" | head -1 || true
  passing="$(grep -oE "^ +[0-9]+ passing" <<< "$plain" | grep -oE "[0-9]+" || echo 0)"
  failing="$(grep -oE "^ +[0-9]+ failing" <<< "$plain" | grep -oE "[0-9]+" || echo 0)"
  echo "$probe: $passing passing, $failing failing"
  if [ "$code" -ne 0 ] || [ "$failing" != 0 ] || [ "$passing" != 2 ]; then
    echo "$probe: FAILED: each of its 2 tests must pass, and Cypress exit 0 ($OUT/$probe.txt)"
    failed=1
  fi
done
echo
if [ "$failed" -ne 0 ]; then
  echo "probes: FAILED"
  exit 1
fi
echo "probes: OK: both facts hold in this Cypress, with no app"

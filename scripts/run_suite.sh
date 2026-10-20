#!/usr/bin/env bash
# **A browser suite, run for a recipe's evidence** (track evidence-integrity-2,
# the integrator's ruling 6 of 2026-10-15).
#
#   bash scripts/run_suite.sh [-C DIR] [--runs N] -- COMMAND [ARGUMENT]...
#
# Runs COMMAND, a Playwright run (`pnpm exec playwright test e2e/feed.spec.mjs
# --reporter=line`), in DIR (`spikes/own-renderer` unless `-C` names another,
# from the repository's root), N times (once unless `--runs` says more), and
# prints what a reader of the evidence needs of each run, its colours and
# line erasures taken out: each failure (`  1) [webkit] › ...`, `Error:`,
# `Expected`, `Received`), each test's line where the list reporter prints
# them (`✓`, `✘`, `-`), a spec's `EVIDENCE` lines, and the counts (`30
# passed`, `1 failed`, `1 flaky`, `2 skipped`, `1 interrupted`, `5 did not
# run`). Each of more runs than one is headed `== run i of N`.
#
# Exits with the status of the first run that failed, once every run has
# run, or 0: a failed test fails the recipe that ran it, whose evidence still
# records what the suite said. A run that exits 0 and passed no test fails
# too: every test skipped, or none selected, is no evidence of passing
# behaviour. Until 2026-10-15, 83 of the 85 recipes that ran a browser suite
# ended its pipeline `|| true`, and their evidence could say "1 failed" in a
# green run (e14-navigate's did, 2026-10-10). A known failure is a job's
# `--known` at the fetch (ADR-0281), never a recipe that swallows its suite's
# status; `scripts/suite_status.py` holds every recipe to this helper.
#
# **A run that fails keeps its output** (track ci-diagnostics, 2026-10-19).
# Playwright clears `test-results/` at the start of each run, so a failure
# followed by a passing run (`e10-browser`'s three, a mutation control's after
# it in the same CI shard) lost its trace. Each failing run's `test-results/`
# is copied, as it ends, to `kept-traces/<recipe>/<time>/run-<i>/` in DIR
# (`scripts/keep_traces.py`: bounded, 32 MiB in all, `PW_KEPT_TRACES_KIB`),
# the recipe being `$PW_RECIPE` (`ci_recipes.py` sets it; `local` where
# unset). `verify.yml` uploads every `kept-traces/`. The status is the
# suite's, whether or not its output could be kept.
set -uo pipefail

dir="spikes/own-renderer"
runs=1
while [ $# -gt 0 ]; do
  case "$1" in
    -C) dir="$2"; shift 2 ;;
    --runs) runs="$2"; shift 2 ;;
    --) shift; break ;;
    *) echo "run_suite: unknown option '$1' (usage: run_suite.sh [-C DIR] [--runs N] -- COMMAND...)" >&2; exit 2 ;;
  esac
done
if [ $# -eq 0 ]; then
  echo "run_suite: no command to run" >&2
  exit 2
fi
case "$runs" in
  '' | *[!0-9]* | 0) echo "run_suite: --runs takes a count of 1 or more, not '$runs'" >&2; exit 2 ;;
esac
if ! [ "$runs" -gt 0 ] 2>/dev/null; then
  echo "run_suite: --runs takes a count of 1 or more, not '$runs'" >&2
  exit 2
fi

# What the evidence keeps of a run, once its escape sequences are gone.
KEEP='^ +[0-9]+\) |Error:|^ +(Expected|Received)|^ +(✓|✘|-) |EVIDENCE|^ +[0-9]+ (passed|failed|flaky|skipped|interrupted|did not run)'

first=0
out="$(mktemp)"
trap 'rm -f "$out"' EXIT

# Where a failing run's output is kept, apart from the next run's (see above).
here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
tag="$(date -u +%Y%m%dT%H%M%SZ)-$$"
keep_output() {
  [ -d "$dir/test-results" ] || return 0
  local said
  said="$(python3 "$here/keep_traces.py" "$dir/test-results" --root "$dir/kept-traces" \
    --label "${PW_RECIPE:-}" --tag "$tag" --run "run-$1" 2>&1)" || true
  echo "  run_suite: $said"
}

for i in $(seq "$runs"); do
  if [ "$runs" -gt 1 ]; then
    echo "== run $i of $runs"
  fi
  (cd "$dir" && "$@") > "$out" 2>&1
  status=$?
  sed -E 's/\x1b\[[0-9;?]*[A-Za-z]//g' "$out" > "$out.clean"
  mv "$out.clean" "$out"
  # The filter matching nothing is not the suite's status: that is `status`.
  kept="$(grep -aE "$KEEP" "$out" || true)"
  if [ -n "$kept" ]; then
    printf '%s\n' "$kept"
  fi
  if [ "$status" -eq 0 ] && ! grep -aqE '^ +[1-9][0-9]* passed' "$out"; then
    echo "  run_suite: the suite passed no test"
    status=1
  fi
  if [ "$status" -ne 0 ]; then
    # A run that failed before any test said why, in lines the filter does
    # not keep: a crash, a server that did not start, a directory missing.
    if [ -z "$kept" ]; then
      echo "  run_suite: the suite's last lines:"
      tail -n 5 "$out" | sed 's/^/    /'
    fi
    # Before the next run clears the directory it is in.
    keep_output "$i"
    echo "  run_suite: exit $status"
    if [ "$first" -eq 0 ]; then
      first=$status
    fi
  fi
done
exit "$first"

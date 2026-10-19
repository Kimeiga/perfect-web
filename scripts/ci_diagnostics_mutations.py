#!/usr/bin/env python3
"""Mutation controls for track ci-diagnostics: two CI-reliability defects
found on 2026-10-19, so that CI's failures can be read and are the code's.

Each mutant undoes one piece:
1. a failing browser run's trace is kept where the next run does not clear
   it (`run_suite.sh`, `keep_traces.py`, `mutation_baseline.py`,
   `ci_recipes.py`, `verify.yml`): a failed run keeps nothing, or only the
   last run's, or a passing run's is kept too, or it is kept inside
   `test-results/`, where the next run clears it; the recipe's name, the
   run's number or the call's own tag is not in its path, so that one
   overwrites another; the budget is not held, or counts only the run in
   hand, or the environment's is ignored, or what was left out is not named;
   a name leaves the directory; a failure to keep changes the suite's status;
   a red baseline keeps nothing, or a passing run's output, or an earlier
   recipe's, or what was last run is no browser run (a mutant's, kept after
   all), or the harness's suite is not looked at; a recipe runs without its
   name; an upload step drops `kept-traces/`, or git does not ignore it;
2. (the checker's latency, in the second commit.)

Each runs the suite that holds it: `scripts/tests/`'s, by `unittest`. At
least one test must fail.

Run from the repository root; `just e14-ci-diagnostics` records the output.
The sources are restored after every mutant, whatever happens.
"""

import os
import pathlib
import re
import signal
import subprocess
import sys

ROOT = pathlib.Path(__file__).resolve().parent.parent
HELPER = ROOT / "scripts/run_suite.sh"
KEEP = ROOT / "scripts/keep_traces.py"
BASELINE = ROOT / "scripts/mutation_baseline.py"
RECIPES = ROOT / "scripts/ci_recipes.py"
VERIFY = ROOT / ".github/workflows/verify.yml"
IGNORE = ROOT / ".gitignore"
RENDERER_IGNORE = ROOT / "spikes/own-renderer/.gitignore"

# (what, suite, file, anchor, replacement)
MUTANTS = [
    # 1. The helper keeps a failed run's output.
    (
        "a failed run keeps nothing",
        "helper",
        HELPER,
        '    keep_output "$i"\n',
        "    :\n",
    ),
    (
        "only the last run's output is kept",
        "helper",
        HELPER,
        '    keep_output "$i"\n',
        '    [ "$i" -eq "$runs" ] && keep_output "$i"\n',
    ),
    (
        "a run that passed is kept too",
        "helper",
        HELPER,
        '  kept="$(grep -aE "$KEEP" "$out" || true)"\n',
        '  kept="$(grep -aE "$KEEP" "$out" || true)"\n  keep_output "$i"\n',
    ),
    (
        "the output is kept inside test-results, where the next run clears it",
        "helper",
        HELPER,
        '--root "$dir/kept-traces"',
        '--root "$dir/test-results/kept-traces"',
    ),
    (
        "the recipe's name is not in the path",
        "helper",
        HELPER,
        '    --label "${PW_RECIPE:-}" --tag',
        '    --label "" --tag',
    ),
    (
        "the run's number is not in the path",
        "helper",
        HELPER,
        '--run "run-$1"',
        '--run "run"',
    ),
    (
        "the call's own tag is the same for every call",
        "helper",
        HELPER,
        'tag="$(date -u +%Y%m%dT%H%M%SZ)-$$"\n',
        'tag="call"\n',
    ),
    (
        "a failure to keep fails the helper with a status of its own",
        "helper",
        HELPER,
        '--run "run-$1" 2>&1)" || true\n',
        '--run "run-$1" 2>&1)" || exit 3\n',
    ),
    # The keeping itself.
    (
        "the budget is not held",
        "keep",
        KEEP,
        "        if used + need > budget:\n",
        "        if False:\n",
    ),
    (
        "the budget counts only the run in hand",
        "keep",
        KEEP,
        "    used = tree_bytes(root)\n",
        "    used = 0\n",
    ),
    (
        "what was left out is not named",
        "keep",
        KEEP,
        "    if skipped:\n        listing = ",
        "    if False:\n        listing = ",
    ),
    (
        "a name leaves the directory",
        "keep",
        KEEP,
        '    cleaned = re.sub(r"[^A-Za-z0-9._-]", "_", text or "").strip(".")\n',
        '    cleaned = text or ""\n',
    ),
    (
        "the environment's budget is ignored",
        "keep",
        KEEP,
        "    return int(said) if said.isdigit() else BUDGET_KIB\n",
        "    return BUDGET_KIB\n",
    ),
    (
        "the budget is a thousand times the traces",
        "keep",
        KEEP,
        "BUDGET_KIB = 32 * 1024\n",
        "BUDGET_KIB = 32 * 1024 * 1024\n",
    ),
    # A red baseline keeps its browser run, and a mutant's is not kept.
    (
        "a red baseline keeps nothing",
        "baseline",
        BASELINE,
        "    keep_browser_output(out)\n    out.flush()",
        "    out.flush()",
    ),
    (
        "a browser run that passed is kept",
        "baseline",
        BASELINE,
        "    if last is None or last[1] == 0 or not runs_a_suite(last[0]):\n",
        "    if last is None or not runs_a_suite(last[0]):\n",
    ),
    (
        "what was last run need be no browser run, so a mutant's is kept",
        "baseline",
        BASELINE,
        "    if last is None or last[1] == 0 or not runs_a_suite(last[0]):\n",
        "    if last is None or last[1] == 0:\n",
    ),
    (
        "an earlier recipe's output is kept",
        "baseline",
        BASELINE,
        "            if not source.is_dir() or source.stat().st_mtime < STARTED:\n",
        "            if not source.is_dir():\n",
    ),
    (
        "the benchmark harness's suite is not looked at",
        "baseline",
        BASELINE,
        'SUITES = ("spikes/own-renderer", "benchmarks/harness")\n',
        'SUITES = ("spikes/own-renderer",)\n',
    ),
    (
        "a failure to keep a baseline's output hides what failed",
        "baseline",
        BASELINE,
        "    except Exception as error:  # noqa: BLE001 - a failed baseline is told anyway\n",
        "    except KeyError as error:  # noqa: BLE001 - a failed baseline is told anyway\n",
    ),
    # CI names the recipe, uploads the keeping, and git ignores it.
    (
        "a recipe runs without its name",
        "keep",
        RECIPES,
        '                env={**os.environ, "PW_RECIPE": recipe},\n',
        "                env=os.environ,\n",
    ),
    (
        "the recipes' upload drops kept-traces",
        "keep",
        VERIFY,
        "          path: |\n"
        "            spikes/own-renderer/kept-traces/\n"
        "            benchmarks/harness/kept-traces/\n"
        "            spikes/own-renderer/test-results/\n",
        "          path: spikes/own-renderer/test-results/\n",
    ),
    (
        "the browser jobs' upload drops kept-traces",
        "keep",
        VERIFY,
        "          path: |\n"
        "            spikes/own-renderer/test-results/\n"
        "            spikes/own-renderer/kept-traces/\n",
        "          path: spikes/own-renderer/test-results/\n",
    ),
    (
        "git does not ignore kept-traces at the root",
        "keep",
        IGNORE,
        "kept-traces/\n",
        "",
    ),
    (
        "git does not ignore kept-traces beside the suite",
        "keep",
        RENDERER_IGNORE,
        "kept-traces/\n",
        "",
    ),
]

UNITTEST = [sys.executable, "-m", "unittest"]
SUITES = {
    "helper": UNITTEST + ["scripts/tests/test_run_suite.py"],
    "keep": UNITTEST + ["scripts/tests/test_keep_traces.py"],
    "baseline": UNITTEST + ["scripts/tests/test_mutation_baseline.py"],
}

# How long one command may run. Past the bound it is killed with everything
# it started, and the run counts as failing.
BOUND = 1800


def counted(out):
    """(built, passed, failed) as a suite's output says: `unittest`'s `Ran`
    and `OK (...)` or `FAILED (...)` lines (a skipped test neither passed nor
    failed)."""
    ran = [int(n) for n in re.findall(r"^Ran (\d+) tests? in", out, re.M)]
    if not ran:
        return False, 0, 0
    ended = re.findall(r"^(?:OK|FAILED) \((.*)\)$", out, re.M)
    failed = sum(int(n) for line in ended for n in re.findall(r"(?:failures|errors)=(\d+)", line))
    skipped = sum(int(n) for line in ended for n in re.findall(r"skipped=(\d+)", line))
    return True, sum(ran) - failed - skipped, failed


def run_tests(suite):
    """(built, passed, failed) over the suite's tests."""
    p = subprocess.Popen(SUITES[suite], cwd=ROOT, start_new_session=True,
                         stdout=subprocess.PIPE, stderr=subprocess.STDOUT, text=True)
    try:
        out, _ = p.communicate(timeout=BOUND)
    except subprocess.TimeoutExpired:
        os.killpg(p.pid, signal.SIGKILL)
        p.communicate()
        return True, 0, 1
    return counted(out)


def main():
    import mutation_baseline  # keeps what each command says, for a red baseline
    # Stopped from outside, the sources are still restored: SIGTERM raises
    # here, and the `finally` below runs.
    signal.signal(signal.SIGTERM, lambda *_: sys.exit(1))
    for suite in SUITES:
        built, passed, failed = run_tests(suite)
        print(f"baseline ({suite}): {passed} passed, {failed} failed", flush=True)
        if not built or failed or not passed:
            print("FAIL: the unmutated baseline is not green; no mutant can mean anything")
            mutation_baseline.explain()
            return 1

    survivors = 0
    for what, suite, path, anchor, replacement in MUTANTS:
        original = path.read_text()
        if original.count(anchor) != 1:
            print(f"{what}: ANCHOR NOT FOUND EXACTLY ONCE in {path.name}")
            survivors += 1
            continue
        try:
            path.write_text(original.replace(anchor, replacement, 1))
            built, passed, failed = run_tests(suite)
        finally:
            path.write_text(original)
        if not built:
            verdict = "KILLED (does not build)"
        elif failed:
            verdict = f"KILLED ({failed} of {passed + failed} tests fail)"
        else:
            verdict = "SURVIVED"
            survivors += 1
        print(f"{what} [{suite}]: {verdict}", flush=True)

    print(f"{len(MUTANTS) - survivors} of {len(MUTANTS)} mutants killed")
    return 1 if survivors else 0


if __name__ == "__main__":
    sys.exit(main())

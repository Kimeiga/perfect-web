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
2. the checker's latency ceiling bounds the fastest of several checks
   (`check_latency.rs`): the ceiling is raised, or counted in the wrong
   unit; it bounds the median, or the fastest of the wall clock, and not the
   fastest CPU time; a quiet machine is waited on, or none is, or the waiting
   does not end with its window or with its most checks; fewer than seven
   checks are taken; the ratio of the slow path to the clean is of the
   medians; the control (a check that happened) reads the median, or nothing.
   (The checker made pathological, and the clock that reads nothing, are
   `evidence_integrity_mutations.py`'s, which run the measurement itself.)
3. a mutation script's mutants are built incrementally where the run asks
   (`mutant_incremental.py`, `mutation_baseline.py`, `ci_recipes.py`,
   `verify.yml`): the run's word is not heard, or is taken whether given or
   not, or does not win over the recipe's own `CARGO_INCREMENTAL=0`; the
   script never starts it, or the caches are never looked at after a command,
   or are removed whatever they hold, or never, or a target triple's are
   missed, or a command that is no cargo's is looked at, or incremental
   compilation off is no reason to leave them, or the limit is not the
   environment's, or a hundred times the default; the recipes' prune leaves a
   triple's; the `Recipes` step does not ask, or every build is incremental.

Each runs the suite that holds it: `scripts/tests/`'s, by `unittest`, or
`pw-core`'s statistic tests, by `cargo test`. At least one test must fail.

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
LATENCY = ROOT / "compiler/pw-core/tests/check_latency.rs"
INCREMENTAL = ROOT / "scripts/mutant_incremental.py"

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
    # 2. The latency ceiling bounds the fastest of several checks.
    (
        "the ceiling is raised to 1.6 s a thousand lines",
        "latency",
        LATENCY,
        "const CEILING_PER_THOUSAND_LINES: Duration = Duration::from_millis(800);\n",
        "const CEILING_PER_THOUSAND_LINES: Duration = Duration::from_millis(1600);\n",
    ),
    (
        "a thousand lines is counted the wrong way round",
        "latency",
        LATENCY,
        "    cpu.mul_f64(1000.0 / lines as f64)\n",
        "    cpu.mul_f64(lines as f64 / 1000.0)\n",
    ),
    (
        "the ceiling bounds the median again",
        "latency",
        LATENCY,
        "    fits(cost.fastest, lines)\n",
        "    fits(cost.cpu, lines)\n",
    ),
    (
        "the fastest is the median",
        "latency",
        LATENCY,
        '        fastest: runs.iter().map(|r| r.cpu).min().expect("a run"),\n',
        "        fastest: median(runs.iter().map(|r| r.cpu).collect()),\n",
    ),
    (
        "the fastest is of the wall clock",
        "latency",
        LATENCY,
        '        fastest: runs.iter().map(|r| r.cpu).min().expect("a run"),\n',
        '        fastest: runs.iter().map(|r| r.wall).min().expect("a run"),\n',
    ),
    (
        "a quiet machine is waited on too",
        "latency",
        LATENCY,
        "        let waiting = enough.is_some_and(|enough| !enough(fastest));\n",
        "        let waiting = enough.is_some();\n",
    ),
    (
        "no machine is waited for",
        "latency",
        LATENCY,
        "        if !waiting || runs.len() >= MAX_RUNS || spent >= WINDOW {\n",
        "        if true {\n",
    ),
    (
        "the waiting does not end with its window",
        "latency",
        LATENCY,
        "        if !waiting || runs.len() >= MAX_RUNS || spent >= WINDOW {\n",
        "        if !waiting || runs.len() >= MAX_RUNS {\n",
    ),
    (
        "the waiting does not end with its most checks",
        "latency",
        LATENCY,
        "        if !waiting || runs.len() >= MAX_RUNS || spent >= WINDOW {\n",
        "        if !waiting || spent >= WINDOW {\n",
    ),
    (
        "fewer than seven checks are taken",
        "latency",
        LATENCY,
        "const MIN_RUNS: usize = 7;\n",
        "const MIN_RUNS: usize = 3;\n",
    ),
    (
        "the slow path is judged against the clean at their medians",
        "latency",
        LATENCY,
        "    rejected.fastest < clean.fastest * 5\n",
        "    rejected.cpu < clean.cpu * 5\n",
    ),
    (
        "the control reads the median of the checks",
        "latency",
        LATENCY,
        "    cost.fastest > Duration::from_micros(200)\n",
        "    cost.cpu > Duration::from_micros(200)\n",
    ),
    (
        "the control is lifted",
        "latency",
        LATENCY,
        "    cost.fastest > Duration::from_micros(200)\n",
        "    true\n",
    ),
    # 3. A mutant's builds are incremental where the run asks, and bounded.
    (
        "a run that asks for incremental builds is not heard",
        "incremental",
        INCREMENTAL,
        '    return (os.environ if env is None else env).get("PW_MUTANT_INCREMENTAL") == "1"\n',
        "    return False\n",
    ),
    (
        "every run is taken to ask",
        "incremental",
        INCREMENTAL,
        '    return (os.environ if env is None else env).get("PW_MUTANT_INCREMENTAL") == "1"\n',
        "    return True\n",
    ),
    (
        "a recipe's own zero wins over the run's word",
        "incremental",
        INCREMENTAL,
        '        os.environ["CARGO_INCREMENTAL"] = "1"\n',
        '        os.environ.setdefault("CARGO_INCREMENTAL", "1")\n',
    ),
    (
        "the script never starts it",
        "incremental",
        BASELINE,
        "\nmutant_incremental.start()\n",
        "\npass\n",
    ),
    (
        "the caches are never looked at after a command",
        "incremental",
        BASELINE,
        "            mutant_incremental.bounded(self.args)\n",
        "            pass\n",
    ),
    (
        "the caches are never removed",
        "incremental",
        INCREMENTAL,
        "    if held <= limit:\n",
        "    if True:\n",
    ),
    (
        "the caches are removed whatever they hold",
        "incremental",
        INCREMENTAL,
        "    if held <= limit:\n",
        "    if False:\n",
    ),
    (
        "a target triple's caches are missed",
        "incremental",
        INCREMENTAL,
        '    for pattern in ("*/incremental", "*/*/incremental"):\n',
        '    for pattern in ("*/incremental",):\n',
    ),
    (
        "a command that is no cargo's is looked at",
        "incremental",
        INCREMENTAL,
        '    if os.environ.get("CARGO_INCREMENTAL") != "1" or not runs_cargo(args):\n',
        '    if os.environ.get("CARGO_INCREMENTAL") != "1":\n',
    ),
    (
        "the caches are removed where incremental builds are off",
        "incremental",
        INCREMENTAL,
        '    if os.environ.get("CARGO_INCREMENTAL") != "1" or not runs_cargo(args):\n',
        "    if not runs_cargo(args):\n",
    ),
    (
        "the limit is not the environment's",
        "incremental",
        INCREMENTAL,
        "    return (int(said) if said.isdigit() else MAX_MIB) * 1024 * 1024\n",
        "    return MAX_MIB * 1024 * 1024\n",
    ),
    (
        "the limit is a hundred times as much",
        "incremental",
        INCREMENTAL,
        "MAX_MIB = 6 * 1024\n",
        "MAX_MIB = 600 * 1024\n",
    ),
    (
        "the recipes' prune leaves a target triple's caches",
        "incremental",
        RECIPES,
        '    for pattern in ("*/incremental", "*/*/incremental"):\n',
        '    for pattern in ("*/incremental",):\n',
    ),
    (
        "the Recipes step does not ask for incremental builds",
        "incremental",
        VERIFY,
        '          PW_MUTANT_INCREMENTAL: "1"\n',
        "",
    ),
    (
        "every build of the workflow is incremental",
        "incremental",
        VERIFY,
        '  CARGO_INCREMENTAL: "0"\n',
        '  CARGO_INCREMENTAL: "1"\n',
    ),
]

UNITTEST = [sys.executable, "-m", "unittest"]
SUITES = {
    "helper": UNITTEST + ["scripts/tests/test_run_suite.py"],
    "keep": UNITTEST + ["scripts/tests/test_keep_traces.py"],
    "baseline": UNITTEST + ["scripts/tests/test_mutation_baseline.py"],
    "incremental": UNITTEST + ["scripts/tests/test_mutant_incremental.py"],
    # The statistic's tests, which run no check: no timing in them.
    "latency": [
        "cargo", "test", "--quiet", "--locked", "-p", "pw-core", "--test", "check_latency", "--",
        "the_ceiling_is_800", "the_ceiling_bounds", "the_fastest_is_of", "a_quiet_machine",
        "a_loaded_machine", "the_waiting", "a_checker_made_slow", "a_clock_that", "producing_diagnostics",
    ],
}

# How long one command may run. Past the bound it is killed with everything
# it started, and the run counts as failing.
BOUND = 1800


def counted(out):
    """(built, passed, failed) as a suite's output says: `unittest`'s `Ran`
    and `OK (...)` or `FAILED (...)` lines (a skipped test neither passed nor
    failed), or cargo's `test result:` lines."""
    ran = [int(n) for n in re.findall(r"^Ran (\d+) tests? in", out, re.M)]
    if ran:
        ended = re.findall(r"^(?:OK|FAILED) \((.*)\)$", out, re.M)
        failed = sum(int(n) for line in ended for n in re.findall(r"(?:failures|errors)=(\d+)", line))
        skipped = sum(int(n) for line in ended for n in re.findall(r"skipped=(\d+)", line))
        return True, sum(ran) - failed - skipped, failed
    found = re.findall(r"test result: \w+\. (\d+) passed; (\d+) failed", out)
    if not found:
        return False, 0, 0
    return True, sum(int(p) for p, _ in found), sum(int(f) for _, f in found)


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

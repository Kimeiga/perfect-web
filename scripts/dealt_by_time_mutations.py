#!/usr/bin/env python3
"""Mutation controls for ADR-0290: a verification run is dealt by the
seconds its recipes last took.

Each mutant undoes one piece: the plan's costs read from the seconds kept,
a shard's setup counted where a recipe is dealt, the unmeasured estimated
at the upper quartile of its kind (the amendment of 2026-10-09: not the
median, not every recipe's, its scripts read for its kind, the host's
tests a kind of their own), the database recipes' shards chosen by when
the run ends (and the fewest of those that end it as soon), and the fetch
keeping only a recipe that ran to its end, over the others kept. At least
one test of `scripts/tests/test_ci_scripts.py` must then fail.

Run from the repository root; `just e14-dealt-by-time` records the output.
The sources are restored after every mutant, whatever happens.
"""

import pathlib
import re
import signal
import subprocess
import sys

ROOT = pathlib.Path(__file__).resolve().parent.parent
PLAN = ROOT / "scripts/ci_plan.py"
FETCH = ROOT / "scripts/evidence_fetch.py"

# (what is undone, file, anchor, replacement)
MUTANTS = [
    (
        "a recipe costs its mutants again",
        PLAN,
        "    costs = estimated(names, body, measured())\n",
        '    costs = {n: cost(body.get(n, "")) for n in names}\n',
    ),
    (
        "a shard's setup is not counted",
        PLAN,
        "            return ends[k] + costs.get(name, 1) + SETUP[joined(kinds[k], wants)] - SETUP[kinds[k]]\n",
        "            return ends[k] + costs.get(name, 1)\n",
    ),
    (
        "a recipe not yet measured is estimated at the rate where none is",
        PLAN,
        "        return rs[(3 * len(rs)) // 4] if rs else RATE\n",
        "        return RATE\n",
    ),
    # Its amendment of 2026-10-09: the upper quartile of its kind.
    (
        "a recipe not yet measured is estimated at the median",
        PLAN,
        "        return rs[(3 * len(rs)) // 4] if rs else RATE\n",
        "        return rs[len(rs) // 2] if rs else RATE\n",
    ),
    (
        "a recipe not yet measured is estimated at every recipe's rate",
        PLAN,
        "        rs = sorted(rates.get(k) or every)\n",
        "        rs = sorted(every)\n",
    ),
    (
        "a script a recipe runs is not read for its kind",
        PLAN,
        "            texts.append((ROOT / \"scripts\" / script).read_text())\n",
        "            pass\n",
    ),
    (
        "the host's tests are taken for a type check's",
        PLAN,
        '    if "pw-dev-server" in text:\n        return "host"\n',
        "",
    ),
    (
        "the recipes run against a database take a shard each",
        PLAN,
        "    for beside in range(1, most + 1) if on_database else [0]:\n",
        "    for beside in [most] if on_database else [0]:\n",
    ),
    (
        "the recipes run against a database share one shard",
        PLAN,
        "    for beside in range(1, most + 1) if on_database else [0]:\n",
        "    for beside in [1] if on_database else [0]:\n",
    ),
    (
        "more shards are taken where fewer end the run as soon",
        PLAN,
        "        if best is None or ends < best[0]:\n",
        "        if best is None or ends <= best[0]:\n",
    ),
    (
        "a plan of nothing takes the longest of no shards",
        PLAN,
        "    if not names:\n        return []\n",
        "",
    ),
    (
        "a recipe that failed has its seconds kept",
        FETCH,
        '    return {r["recipe"]: round(r["seconds"]) for r in results if r.get("status") == 0}\n',
        '    return {r["recipe"]: round(r["seconds"]) for r in results}\n',
    ),
    (
        "the seconds kept replace every other recipe's",
        FETCH,
        "    kept.update(seconds)\n",
        "    kept = dict(seconds)\n",
    ),
]

TESTS = [[sys.executable, "-m", "unittest", "scripts/tests/test_ci_scripts.py"]]


def run_tests():
    """(built, passed, failed) over every test command."""
    built, passed, failed = True, 0, 0
    for cmd in TESTS:
        r = subprocess.run(cmd, cwd=ROOT, capture_output=True, text=True)
        out = r.stdout + r.stderr
        ran = re.search(r"^Ran (\d+) tests?", out, re.M)
        if ran is None:
            built = False
            continue
        bad = re.search(r"^FAILED \((.*)\)", out, re.M)
        n_bad = sum(int(n) for n in re.findall(r"=(\d+)", bad.group(1))) if bad else 0
        passed += int(ran.group(1)) - n_bad
        failed += n_bad
    return built, passed, failed


def main():
    import mutation_baseline  # keeps what each command says, for a red baseline
    # Stopped from outside, the sources are still restored: SIGTERM raises
    # here, and the `finally` below runs.
    signal.signal(signal.SIGTERM, lambda *_: sys.exit(1))
    built, passed, failed = run_tests()
    print(f"baseline: {passed} passed, {failed} failed", flush=True)
    if not built or failed or not passed:
        print("FAIL: the unmutated baseline is not green; no mutant can mean anything")
        mutation_baseline.explain()
        return 1

    survivors = 0
    for what, path, anchor, replacement in MUTANTS:
        original = path.read_text()
        if original.count(anchor) != 1:
            print(f"{what}: ANCHOR NOT FOUND EXACTLY ONCE in {path.name}")
            survivors += 1
            continue
        try:
            path.write_text(original.replace(anchor, replacement, 1))
            built, passed, failed = run_tests()
        finally:
            path.write_text(original)
        if not built:
            verdict = "KILLED (does not run)"
        elif failed:
            verdict = f"KILLED ({failed} of {passed + failed} tests fail)"
        else:
            verdict = "SURVIVED"
            survivors += 1
        print(f"{what}: {verdict}", flush=True)

    print(f"{len(MUTANTS) - survivors} of {len(MUTANTS)} mutants killed")
    return 1 if survivors else 0


if __name__ == "__main__":
    sys.exit(main())

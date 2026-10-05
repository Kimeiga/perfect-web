#!/usr/bin/env python3
"""Mutation controls for ADR-0215: a query's `retry` reaches its runtime as
declared, and `fixed` is no strategy (the owner's ruling 0089-b).

Run from the repository root; `just e14-query-retry` records the output. The
source is restored after every mutant, whatever happens.
"""

import pathlib
import re
import subprocess
import sys

ROOT = pathlib.Path(__file__).resolve().parent.parent
PLAN = ROOT / "compiler/pw-core/src/page_values.rs"
CACHE = ROOT / "runtime/pw-resource/src/lib.rs"
SERVER = ROOT / "spikes/own-renderer/server/src/main.rs"
POLICY = ROOT / "compiler/pw-core/src/policy.rs"

# (what is undone, file, anchor, replacement)
MUTANTS = [
    (
        "the plan says every query jitters",
        PLAN,
        "            Retry::Bounded { jitter, .. } => jitter,\n",
        "            Retry::Bounded { .. } => true,\n",
    ),
    (
        "the cache jitters whatever it is told",
        CACHE,
        "        if spread == 0 || !self.jitter {\n",
        "        if spread == 0 {\n",
    ),
    (
        "the server does not read it",
        SERVER,
        "        .jitter(policy[\"jitter\"] == true);\n",
        "        .jitter(true);\n",
    ),
    (
        "`fixed` is a strategy again",
        POLICY,
        "    // `fixed` was a third until ADR-0215 (ruling 0089-b): a query's runtime\n",
        "    Op {\n        id: \"policy.retry.fixed\",\n        name: \"fixed\",\n        args: RETRY_ARGS,\n    },\n"
        "    // `fixed` was a third until ADR-0215 (ruling 0089-b): a query's runtime\n",
    ),
]

TESTS = [
    ["cargo", "test", "--quiet", "--locked", "-p", "pw-core", "--test", "query_retry", "--test", "policy_values"],
    ["cargo", "test", "--quiet", "--locked", "-p", "pw-resource", "--test", "gate"],
    ["cargo", "test", "--quiet", "--locked", "-p", "pw-dev-server", "--", "a_querys_retry_reaches_its_cache_as_declared"],
]


def run_tests():
    """(built, passed, failed) over every test command."""
    built, passed, failed = True, 0, 0
    for cmd in TESTS:
        r = subprocess.run(cmd, cwd=ROOT, capture_output=True, text=True)
        out = r.stdout + r.stderr
        results = re.findall(r"test result: \w+\. (\d+) passed; (\d+) failed", out)
        if not results:
            built = False
            continue
        for p, f in results:
            passed += int(p)
            failed += int(f)
    return built, passed, failed


def main():
    built, passed, failed = run_tests()
    print(f"baseline: {passed} passed, {failed} failed")
    if not built or failed or not passed:
        print("FAIL: the unmutated baseline is not green; no mutant can mean anything")
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
            verdict = "KILLED (does not build)"
        elif failed:
            verdict = f"KILLED ({failed} of {passed + failed} tests fail)"
        else:
            verdict = "SURVIVED"
            survivors += 1
        print(f"{what}: {verdict}")

    print(f"{len(MUTANTS) - survivors} of {len(MUTANTS)} mutants killed")
    return 1 if survivors else 0


if __name__ == "__main__":
    sys.exit(main())

#!/usr/bin/env python3
"""Mutation controls for ADR-0209: a command computes the entries it
invalidates, and the server drops them by the key it computed.

Each mutant undoes one piece: lowering an `invalidates` key into the
command, the contract's import for it, the WIT's function, and the server
dropping the entry, by its key and no other. The conformance tests and the
dev server's must then fail.

Run from the repository root; `just e14-command-invalidations` records the
output. The source is restored after every mutant, whatever happens.
"""

import pathlib
import re
import subprocess
import sys

ROOT = pathlib.Path(__file__).resolve().parent.parent
LOWER = ROOT / "compiler/pw-core/src/backend/lower.rs"
CONTRACT = ROOT / "compiler/pw-core/src/contract.rs"
SERVER = ROOT / "spikes/own-renderer/server/src/main.rs"

# (what is undone, file, anchor, replacement)
MUTANTS = [
    (
        "an `invalidates` key is not the command's",
        LOWER,
        '        .filter(|p| p.name == "emits" || p.name == "invalidates")\n',
        '        .filter(|p| p.name == "emits")\n',
    ),
    (
        "the contract imports no invalidation",
        CONTRACT,
        "    for key in invalidated.iter().flat_map(|p| &p.keys) {\n",
        "    for key in invalidated.iter().flat_map(|p| &p.keys).take(0) {\n",
    ),
    (
        "an invalidated entry is not dropped",
        SERVER,
        "            reached.extend(drop_key(query, Some(values.clone())));\n",
        "            let _ = (query, values);\n",
    ),
    (
        "an invalidated entry drops every entry of its query",
        SERVER,
        "            reached.extend(drop_key(query, Some(values.clone())));\n",
        "            reached.extend(drop_key(query, None));\n",
    ),
]

TESTS = [
    ["cargo", "test", "--quiet", "--locked", "-p", "pw-conformance", "--test", "invalidations"],
    ["cargo", "test", "--quiet", "--locked", "-p", "pw-dev-server"],
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

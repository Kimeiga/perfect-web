#!/usr/bin/env python3
"""Mutation controls for ADR-0132: the browser's resume decision knows what
the build compiled.

Each mutant undoes one piece: the table being what the decision reads, the
table being replaced rather than added to, and a handler's capture schema
being part of what is known. The test in `pw-resume-wasm` must then fail.

Run from the repository root; `just e14-resume-gate` records the output.
The source is restored after every mutant, whatever happens.
"""

import pathlib
import re
import subprocess
import sys

ROOT = pathlib.Path(__file__).resolve().parent.parent
GATE = ROOT / "runtime/pw-resume-wasm/src/lib.rs"

# (what is undone, file, anchor, replacement)
MUTANTS = [
    (
        "the decision knows the store's two handlers by name",
        GATE,
        "            .map(|(identity, capture)| known(identity, capture))\n",
        "            .map(|_| known(\"add_to_cart\", \"cart\"))\n",
    ),
    (
        "the table is added to, not replaced",
        GATE,
        "    known.clear();\n",
        "",
    ),
    (
        "a handler's capture schema is not what is known",
        GATE,
        "            .map(|(identity, capture)| known(identity, capture))\n",
        "            .map(|(identity, _)| known(identity, \"\"))\n",
    ),
]

TESTS = [
    ["cargo", "test", "--quiet", "--locked", "-p", "pw-resume-wasm"],
]


def run_tests():
    """(built, passed, failed) over every test command."""
    built, passed, failed = True, 0, 0
    for cmd in TESTS:
        r = subprocess.run(cmd, cwd=ROOT, capture_output=True, text=True)
        out = r.stdout + r.stderr
        m = re.search(r"test result: \w+\. (\d+) passed; (\d+) failed", out)
        if m is None:
            built = False
            continue
        passed += int(m.group(1))
        failed += int(m.group(2))
    return built, passed, failed


def main():
    import mutation_baseline  # keeps what each command says, for a red baseline
    built, passed, failed = run_tests()
    print(f"baseline: {passed} passed, {failed} failed")
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

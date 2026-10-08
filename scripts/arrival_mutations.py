#!/usr/bin/env python3
"""Mutation controls for ADR-0259 (ruling 0057-c): a map or set from outside
is sorted on arrival, in the component and in the browser's module.

Each mutant undoes one piece: the component's sort of what arrives, its
order, and the browser's module's sort. A key twice stopping the invocation
is `map_mutations.py`'s, as it was. The tests of each must then fail.

Run from the repository root; `just e14-arrival` records the output. The
source is restored after every mutant, whatever happens.
"""

import pathlib
import re
import subprocess
import sys

ROOT = pathlib.Path(__file__).resolve().parent.parent
WASM = ROOT / "compiler/pw-core/src/backend/wasm.rs"
JS = ROOT / "compiler/pw-core/src/backend/js_pure.rs"

# (what is undone, file, anchor, replacement)
MUTANTS = [
    (
        "the component reads what arrives as it came",
        WASM,
        "                let ptr = match self.merge_sort(given, len, entry, &mut |this, a, b, take| {\n",
        "                let ptr = given;\n                let _unread = match self.merge_sort(given, len, entry, &mut |this, a, b, take| {\n",
    ),
    (
        "the component sorts what arrives in reverse",
        WASM,
        "                        .extend([I::LocalGet(o), I::I32Const(0), I::I32LeS, I::LocalSet(take)]);",
        "                        .extend([I::LocalGet(o), I::I32Const(0), I::I32GeS, I::LocalSet(take)]);",
    ),
    (
        "the browser's module reads what arrives as it came",
        JS,
        "function checked(xs, of) {\\n  const s = [...xs].sort((a, b) => key_order(of(a), of(b)));",
        "function checked(xs, of) {\\n  const s = [...xs];",
    ),
]

TESTS = [
    ["cargo", "test", "--quiet", "--locked", "-p", "pw-conformance", "--test", "maps"],
    ["cargo", "test", "--quiet", "--locked", "-p", "pw-conformance", "--test", "javascript"],
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

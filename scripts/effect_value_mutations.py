#!/usr/bin/env python3
"""Mutation controls for ADR-0078: an effect is performed where its function
is named.

Each mutant undoes one piece of how a declaration named as a value
contributes its effects: that it is counted at all, that a binding in scope
is its own value, that a helper declaring no row carries everything it
performs, and that the diagnostic says the function was named as a value.
The effect tests and the generality witnesses must then fail.

Run from the repository root; `just e10-effects-through-values` records the
output. The source is restored after every mutant, whatever happens.
"""

import pathlib
import re
import subprocess
import sys

ROOT = pathlib.Path(__file__).resolve().parent.parent
EFFECTS = ROOT / "compiler/pw-core/src/effects.rs"

# (what is undone, file, anchor, replacement)
MUTANTS = [
    (
        "a function named as a value is not counted",
        EFFECTS,
        "        self.value_effects(unit, body, types, &mut out);\n",
        "",
    ),
    (
        "a binding in scope is read as the declaration it shadows",
        EFFECTS,
        "                Expr::Name(_) if types.lexical().binder(id).is_some() => continue,\n",
        "",
    ),
    (
        "a helper declaring no row carries only what it calls",
        EFFECTS,
        "                let found = self.infer_in_at(*unit, body, types);",
        "                let found = {\n"
        "                    let _ = types;\n"
        "                    self.infer_at(*unit, body)\n"
        "                };",
    ),
    (
        "a function named as a value is said to be called",
        EFFECTS,
        "                    via: Via::Value {",
        "                    via: Via::Direct {",
    ),
]

TESTS = [
    ["cargo", "test", "--quiet", "--locked", "-p", "pw-core", "--test", "effects_through_values"],
    ["cargo", "test", "--quiet", "--locked", "-p", "pw-core", "--test", "generality"],
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

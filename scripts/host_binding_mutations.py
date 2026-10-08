#!/usr/bin/env python3
"""Mutation controls for ADR-0262: a host binding is an operation a host
provides, `"namespace:package/interface#name"`, each part a WIT
identifier, checked where it is written (PW0335).

Each mutant undoes one piece: the domain itself, the interface the uploads
track's binding left out, the namespace, and three of a WIT identifier's
rules (one case a word, a letter first, nothing but letters and digits).
The tests of `compiler/pw-core/tests/host_bindings.rs` must then fail.

Run from the repository root; `just e14-host-bindings` records the output.
The source is restored after every mutant, whatever happens.
"""

import pathlib
import re
import subprocess
import sys

ROOT = pathlib.Path(__file__).resolve().parent.parent
POLICY = ROOT / "compiler/pw-core/src/policy.rs"

# (what is undone, file, anchor, replacement)
MUTANTS = [
    (
        "a binding is any string again",
        POLICY,
        '        "host" => Domain::HostOp,\n',
        '        "host" => Domain::Str,\n',
    ),
    (
        "a binding needs no interface",
        POLICY,
        "        .ok_or(\"it names no interface after its package, `/`\")?;\n",
        "        .unwrap_or((interface, \"any\"));\n",
    ),
    (
        "a binding needs no namespace",
        POLICY,
        "        .ok_or(\"its package has no namespace, `namespace:package`\")?;\n",
        "        .unwrap_or((\"pw\", package));\n",
    ),
    (
        "a word of an identifier may mix cases",
        POLICY,
        "        if lower && upper {\n",
        "        if lower && upper && id.is_empty() {\n",
    ),
    (
        "an identifier may begin with what is not a letter",
        POLICY,
        "        if i == 0 && !first.is_ascii_alphabetic() {\n",
        "        if i == 0 && first == '-' {\n",
    ),
    (
        "an identifier may hold an underscore",
        POLICY,
        "        if let Some(c) = word.chars().find(|c| !c.is_ascii_alphanumeric()) {\n",
        "        if let Some(c) = word.chars().find(|c| !c.is_ascii_alphanumeric() && *c != '_') {\n",
    ),
]

TESTS = [
    ["cargo", "test", "--quiet", "--locked", "-p", "pw-core", "--test", "host_bindings"],
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

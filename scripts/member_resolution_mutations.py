#!/usr/bin/env python3
"""Mutation controls for ADR-0254: a member names the declaration its module
sees.

Each mutant undoes one piece: every declaration of a member kept, rather
than the last registered; the calling module asked, and what it imports and
what is declared beside the receiver's type seen; a module that sees
several or none refused, PW0628, naming every declaration where it sees
none; the inference and the value relations asking as the body's module;
and the affine check reading an ambiguous member as no function value. The
tests of each must then fail.

Run from the repository root; `just e14-member-resolution` records the
output. The source is restored after every mutant, whatever happens.
"""

import pathlib
import re
import subprocess
import sys

ROOT = pathlib.Path(__file__).resolve().parent.parent
SIGNATURES = ROOT / "compiler/pw-core/src/signatures.rs"
INFER = ROOT / "compiler/pw-core/src/infer.rs"
VALUES = ROOT / "compiler/pw-core/src/values.rs"
AFFINE = ROOT / "compiler/pw-core/src/affine.rs"

# (what is undone, file, anchor, replacement)
MUTANTS = [
    (
        "a second declaration replaces the first",
        SIGNATURES,
        "                        out.by_member\n                            .entry((key, decl.name.clone()))\n                            .or_default()\n                            .push(sig.clone());\n",
        "                        *out.by_member\n                            .entry((key, decl.name.clone()))\n                            .or_default() = vec![sig.clone()];\n",
    ),
    (
        "the calling module is not asked",
        SIGNATURES,
        "                    .filter(|s| self.sees(module, receiver, name, s));\n",
        "                    .filter(|s| self.sees(None, receiver, name, s));\n",
    ),
    (
        "an import is not seen",
        SIGNATURES,
        "            || here.imports.iter().any(|i| {\n",
        "            || false && here.imports.iter().any(|i| {\n",
    ),
    (
        "a term beside its type is not seen",
        SIGNATURES,
        "            && t.unit == s.definition.unit\n",
        "            && t.unit == s.definition.unit\n            && false\n",
    ),
    (
        "no declaration is named where none is seen",
        SIGNATURES,
        "            0 => all.iter().map(|s| s.path.clone()).collect(),\n",
        "            0 => Vec::new(),\n",
    ),
    (
        "an ambiguous member is not refused",
        VALUES,
        "            Some(r) if !self.sigs.member_choices(self.module(), r, name).is_empty() => {\n",
        "            Some(r) if false && !self.sigs.member_choices(self.module(), r, name).is_empty() => {\n",
    ),
    (
        "the value relations ask as no module",
        VALUES,
        "        self.ws.module_of(self.at).map(|m| m.name.as_str())\n",
        "        self.ws.module_of(self.at).map(|m| m.name.as_str()).filter(|_| false)\n",
    ),
    (
        "the inference asks as no module",
        INFER,
        "        self.sigs.member_in(self.module.as_deref(), ty, name)\n",
        "        self.sigs.member_in(None, ty, name)\n",
    ),
    (
        "an ambiguous member is a function value",
        AFFINE,
        "            !types.ambiguous_member(body, *base, name) && function_value(body, types, *base)\n",
        "            function_value(body, types, *base)\n",
    ),
]

TESTS = [
    ["cargo", "test", "--quiet", "--locked", "-p", "pw-core", "--test", "member_resolution"],
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

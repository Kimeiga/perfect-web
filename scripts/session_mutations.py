#!/usr/bin/env python3
"""Mutation controls for ADR-0264: a session's handle never reaches the
browser.

Each mutant undoes one piece: an answer checked (a query's, a command's),
what markup prints (a hole, an attribute's value, a string's holes), and a
handle found where it sits (the type itself, an argument, a record's
field). The tests of `compiler/pw-core/tests/session_to_browser.rs` must
then fail.

Run from the repository root; `just e14-session-to-browser` records the
output. The source is restored after every mutant, whatever happens.
"""

import pathlib
import re
import subprocess
import sys

ROOT = pathlib.Path(__file__).resolve().parent.parent
CHECK = ROOT / "compiler/pw-core/src/check.rs"

# (what is undone, file, anchor, replacement)
MUTANTS = [
    (
        "a query's answer may hold a session's handle",
        CHECK,
        "            DeclKind::Query | DeclKind::Subscription | DeclKind::Command\n",
        "            DeclKind::Subscription | DeclKind::Command\n",
    ),
    (
        "a command's answer may hold a session's handle",
        CHECK,
        "            DeclKind::Query | DeclKind::Subscription | DeclKind::Command\n",
        "            DeclKind::Query | DeclKind::Subscription\n",
    ),
    (
        "a printed hole may hold a session's handle",
        CHECK,
        "                Node::Interpolation(e) => vec![*e],\n",
        "                Node::Interpolation(_) => Vec::new(),\n",
    ),
    (
        "an attribute's value may hold a session's handle",
        CHECK,
        "                    .filter(|a| !matches!(a.namespace(), Some((\"on\", _))))\n",
        "                    .filter(|a| a.name.is_empty())\n",
    ),
    (
        "a string's holes may hold a session's handle",
        CHECK,
        "                    Expr::Interpolated { parts, .. } => parts.clone(),\n",
        "                    Expr::Interpolated { .. } => vec![e],\n",
    ),
    (
        "a session's handle in an answer's argument is not found",
        CHECK,
        "    if ty.args().iter().any(|a| session_in(sigs, a, seen)) {\n",
        "    if ty.args().iter().take(0).any(|a| session_in(sigs, a, seen)) {\n",
    ),
    (
        "a session's handle in a record's field is not found",
        CHECK,
        "    decl.record\n        .iter()\n        .flatten()\n        .map(|(_, t)| t)\n"
        "        .chain(decl.representation.iter())\n"
        "        .chain(decl.variants.iter().flatten().flat_map(|(_, ts)| ts.iter()))\n"
        "        .filter_map(crate::resolved::TypeResolution::resolved)\n        .collect()\n",
        "    decl.representation\n        .iter()\n"
        "        .filter_map(crate::resolved::TypeResolution::resolved)\n        .collect()\n",
    ),
    (
        "a printed value that is a session's handle is not one",
        CHECK,
        "                Some(crate::signatures::PrivacyQualifier::Session)\n            ) || args.iter().any(|a| session_in_ty(sigs, a, seen))\n",
        "                Some(crate::signatures::PrivacyQualifier::Secret)\n            ) || args.iter().any(|a| session_in_ty(sigs, a, seen))\n",
    ),
]

TESTS = [
    ["cargo", "test", "--quiet", "--locked", "-p", "pw-core", "--test", "session_to_browser"],
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

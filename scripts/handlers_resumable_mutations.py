#!/usr/bin/env python3
"""Mutation controls for ADR-0134: every handler is resumable, and what it
captures is what it reads.

Each mutant undoes one piece: an `on:` lambda being a handler, an unlisted
handler's captures being inferred, a signal and the handler's own bindings
being left out of them, the template carrying what the manifest derived,
`() =>` taking no parameter, and an event part with no code being refused
at build. The tests in `every_handler_is_resumable.rs` must then fail.

Run from the repository root; `just e14-handlers-resumable` records the
output. The source is restored after every mutant, whatever happens.
"""

import pathlib
import re
import subprocess
import sys

ROOT = pathlib.Path(__file__).resolve().parent.parent
RESUME = ROOT / "compiler/pw-core/src/resume.rs"
BUILD = ROOT / "compiler/pw-core/src/build.rs"
LOWER = ROOT / "compiler/pw-core/src/lower.rs"

# (what is undone, file, anchor, replacement)
MUTANTS = [
    (
        "an `on:` lambda is not a handler",
        # Re-anchored by ADR-0199: a declaration's name is a handler too.
        RESUME,
        "            Expr::Lambda { .. } | Expr::Name(_) | Expr::Field { .. } => on.contains(e),\n",
        "            Expr::Lambda { .. } | Expr::Name(_) | Expr::Field { .. } => false,\n",
    ),
    (
        "an unlisted handler captures nothing",
        RESUME,
        "        out.push((n.clone(), body.expr_span(e), e));\n",
        "        let _ = (n, e);\n",
    ),
    (
        "a signal is captured",
        RESUME,
        "        if own.contains(n) || signal(b) || out.iter().any(|(m, ..)| m == n) {\n",
        "        if own.contains(n) || out.iter().any(|(m, ..)| m == n) {\n",
    ),
    (
        "what the handler binds itself is captured",
        RESUME,
        "        if own.contains(n) || signal(b) || out.iter().any(|(m, ..)| m == n) {\n",
        "        if signal(b) || out.iter().any(|(m, ..)| m == n) {\n",
    ),
    (
        "the template does not carry what the manifest derived",
        BUILD,
        "            identities.insert((unit, decl, lambda), (m.handler, m.capture_paths));\n",
        "            identities.insert((unit, decl, lambda), (m.handler, Vec::new()));\n",
    ),
    (
        "`() =>` takes a parameter",
        LOWER,
        "                        && p.children().next().is_none()\n",
        "                        && p.children().next().is_none() && false\n",
    ),
    (
        "an event part with no code is built",
        BUILD,
        "                .filter(|p| p.kind == \"event\" && p.value.is_empty())\n",
        "                .filter(|p| p.kind == \"event\" && p.value.is_empty() && false)\n",
    ),
]

TESTS = [
    [
        "cargo", "test", "--quiet", "--locked", "-p", "pw-core",
        "--test", "every_handler_is_resumable",
    ],
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

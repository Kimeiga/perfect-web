#!/usr/bin/env python3
"""Mutation controls for ADR-0217: what a handler at the top of the page
captures is set again when it changes (ADR-0210's urgent defect 2).

Each mutant undoes one piece: the template's read of a handler's captures,
the plan's entry for it, the server's state and its patch, and the
speculation's region. The tests of each must then fail.

Run from the repository root; `just e14-top-captures` records the output.
The source is restored after every mutant, whatever happens.
"""

import pathlib
import re
import subprocess
import sys

ROOT = pathlib.Path(__file__).resolve().parent.parent
TEMPLATE = ROOT / "compiler/pw-core/src/template_ir.rs"
PLAN = ROOT / "compiler/pw-core/src/page_values.rs"
SPECULATION = ROOT / "compiler/pw-core/src/backend/speculation.rs"
SERVER = ROOT / "spikes/own-renderer/server/src/main.rs"

# (what is undone, file, anchor, replacement)
MUTANTS = [
    (
        "a handler's captures are no read",
        TEMPLATE,
        "                        ReadKind::Captures,\n                        ReadAt::Expr(*e),\n",
        "                        ReadKind::Meta,\n                        ReadAt::Expr(*e),\n",
    ),
    (
        "the plan lists no handler",
        PLAN,
        "            captures.push(read.part.0);\n",
        "            let _ = read;\n",
    ),
    (
        "the server keeps no captures",
        SERVER,
        "            shown.captures.insert(id, written);\n",
        "            let _ = written;\n",
    ),
    (
        "the server patches no captures",
        SERVER,
        "            if was.captures.get(id) == Some(written) {\n",
        "            if true || was.captures.get(id) == Some(written) {\n",
    ),
    (
        "a speculation renders the captures as an attribute",
        SPECULATION,
        "                    format!(\"{{ kind: \\\"captures\\\", part: {} }}\", r.part)\n",
        "                    format!(\"{{ kind: \\\"attribute\\\", part: {} }}\", r.part)\n",
    ),
]

TESTS = [
    ["cargo", "test", "--quiet", "--locked", "-p", "pw-core", "--test", "top_level_captures"],
    ["cargo", "test", "--quiet", "--locked", "-p", "pw-dev-server", "--", "a_handlers_captures_at_the_top_of_the_page_are_set_again"],
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

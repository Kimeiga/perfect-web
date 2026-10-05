#!/usr/bin/env python3
"""Mutation controls for ADR-0197: a pattern tells a case from a binding by
its capital (ADR-0195, ruling 2).

Each mutant undoes one piece:
- the parser reads a pattern's capital, and `pattern_kind` tells the two;
- a case is declared with a capital (PW0625);
- a template arm's fields are bindings;
- the backend lowers `true` and `false`, `Bool`'s cases.

Every mutant must fail the tests of the rule, or the compiled programs'.

Run from the repository root; `just e14-case-names` records the output. The
source is restored after every mutant, whatever happens.
"""

import os
import pathlib
import re
import signal
import subprocess
import sys

ROOT = pathlib.Path(__file__).resolve().parent.parent
GRAMMAR = ROOT / "compiler/pw-syntax/src/grammar.rs"
RULES = ROOT / "compiler/pw-core/src/rules.rs"
LOWER = ROOT / "compiler/pw-core/src/lower.rs"
BACKEND = ROOT / "compiler/pw-core/src/backend/lower.rs"

# (what, file, anchor, replacement)
MUTANTS = [
    (
        "the parser reads no capital",
        GRAMMAR,
        "                    || pattern_kind(self.cur_text()) == PatternKind::Case\n",
        "",
    ),
    (
        "every pattern name is a case",
        GRAMMAR,
        "        Some(b'A'..=b'Z') => PatternKind::Case,\n        _ => PatternKind::Binding,\n",
        "        _ => PatternKind::Case,\n",
    ),
    (
        "a case named in lowercase is accepted",
        RULES,
        "        check_case_names(decl, &mut out);\n",
        "",
    ),
    (
        "a template arm's field may be a case",
        LOWER,
        "                .all(|n| ident(n) && pw_syntax::pattern_kind(n) == pw_syntax::PatternKind::Binding)\n",
        "                .all(|n| ident(n))\n",
    ),
    (
        "the backend does not lower `true` and `false`",
        BACKEND,
        "                if *ty == Type::Bool\n                    && args.is_empty()\n"
        "                    && matches!(path.as_str(), \"true\" | \"false\") =>\n",
        "                if false =>\n",
    ),
]

TESTS = [
    ["cargo", "test", "--quiet", "--locked", "-p", "pw-core", "--test", "case_names"],
    ["cargo", "test", "--quiet", "--locked", "-p", "pw-conformance", "--test", "patterns"],
]

# How long one command may run. Past the bound it is killed with everything
# it started, and the run counts as failing.
BOUND = 1200


def run_tests():
    """(built, passed, failed) over the tests. A test process that aborts,
    as a stack overflow does, fails."""
    passed = failed = 0
    for command in TESTS:
        p = subprocess.Popen(command, cwd=ROOT, start_new_session=True,
                             stdout=subprocess.PIPE, stderr=subprocess.STDOUT, text=True)
        try:
            out, _ = p.communicate(timeout=BOUND)
        except subprocess.TimeoutExpired:
            os.killpg(p.pid, signal.SIGKILL)
            p.communicate()
            return True, passed, failed + 1
        found = re.findall(r"test result: \w+\. (\d+) passed; (\d+) failed", out)
        if not found:
            if "could not compile" in out or "error[E" in out:
                return False, passed, failed
            # It built, and the process ended before it reported.
            failed += 1
            continue
        passed += sum(int(n) for n, _ in found)
        failed += sum(int(f) for _, f in found)
    return True, passed, failed


def main():
    # Stopped from outside, the source is still restored: SIGTERM raises
    # here, and the `finally` below runs.
    signal.signal(signal.SIGTERM, lambda *_: sys.exit(1))
    built, passed, failed = run_tests()
    print(f"baseline: {passed} passed, {failed} failed", flush=True)
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
        print(f"{what}: {verdict}", flush=True)

    print(f"{len(MUTANTS) - survivors} of {len(MUTANTS)} mutants killed")
    return 1 if survivors else 0


if __name__ == "__main__":
    sys.exit(main())

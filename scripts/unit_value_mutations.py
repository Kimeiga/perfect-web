#!/usr/bin/env python3
"""Mutation controls for ADR-0200: `()` is the unit value, and no expression
the compiler cannot read checks.

Each mutant undoes one piece: `()` lowered, typed and built; a file's syntax
errors reported by `check_sources`, and the file kept out of the program; a
block left open ended by its element's close tag; and the refusal of an
expression or a pattern the compiler does not read.

Every mutant must fail the tests of the rule.

Run from the repository root; `just e14-unit-value` records the output. The
source is restored after every mutant, whatever happens.
"""

import os
import pathlib
import re
import signal
import subprocess
import sys

ROOT = pathlib.Path(__file__).resolve().parent.parent
LOWER = ROOT / "compiler/pw-core/src/lower.rs"
VALUES = ROOT / "compiler/pw-core/src/values.rs"
BACKEND = ROOT / "compiler/pw-core/src/backend/lower.rs"
CHECK = ROOT / "compiler/pw-core/src/check.rs"
GRAMMAR = ROOT / "compiler/pw-syntax/src/grammar.rs"

# (what, file, anchor, replacement)
MUTANTS = [
    (
        "`()` is an expression that did not parse",
        LOWER,
        "                None => b.expr(Expr::Literal(Literal::Unit), span),\n",
        "                None => b.expr(Expr::Error, span),\n",
    ),
    (
        "`()` is typed as an `Int`",
        VALUES,
        "                Literal::Unit => Primitive::Unit,\n",
        "                Literal::Unit => Primitive::Int,\n",
    ),
    (
        "the backend does not build `()`",
        BACKEND,
        "                    Literal::Unit => (Const::Unit, Type::Unit),\n",
        "                    Literal::Unit => return Lowering::Blocked { why: String::new(), span },\n",
    ),
    (
        "a file's syntax errors are not reported",
        CHECK,
        "                p.errors.into_iter().map(syntax_error).collect(),\n",
        "                Vec::new(),\n",
    ),
    (
        "a file that does not parse enters the program",
        CHECK,
        "        .filter(|(_, p)| p.ok())\n",
        "        .filter(|(_, p)| p.ok() || true)\n",
    ),
    (
        "an expression the compiler cannot read checks",
        CHECK,
        "    out.extend(unread(&unit.hir, &unit.src));\n",
        "    out.extend(unread(&unit.hir, &unit.src));\n    out.retain(|d| d.code != crate::codes::UNREAD.id);\n",
    ),
    (
        "a block left open takes its element's close tag",
        GRAMMAR,
        "                    if blocks < open.len() {\n",
        "                    if false {\n",
    ),
    (
        "a pattern the compiler cannot read checks",
        CHECK,
        "            .filter(|(_, p, _)| matches!(p, HPat::Error))\n",
        "            .filter(|_| false)\n",
    ),
]

TESTS = [
    ["cargo", "test", "--quiet", "--locked", "-p", "pw-core", "--test", "unit_value"],
    ["cargo", "test", "--quiet", "--locked", "-p", "pw-core", "--test", "every_expression_is_read"],
    ["cargo", "test", "--quiet", "--locked", "-p", "pw-syntax", "--lib"],
    ["cargo", "test", "--quiet", "--locked", "-p", "pw-core", "--test", "template_blocks"],
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
    import mutation_baseline  # keeps what each command says, for a red baseline
    # Stopped from outside, the source is still restored: SIGTERM raises
    # here, and the `finally` below runs.
    signal.signal(signal.SIGTERM, lambda *_: sys.exit(1))
    built, passed, failed = run_tests()
    print(f"baseline: {passed} passed, {failed} failed", flush=True)
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
        print(f"{what}: {verdict}", flush=True)

    print(f"{len(MUTANTS) - survivors} of {len(MUTANTS)} mutants killed")
    return 1 if survivors else 0


if __name__ == "__main__":
    sys.exit(main())

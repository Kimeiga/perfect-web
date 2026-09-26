#!/usr/bin/env python3
"""Mutation controls for ADR-0068: what each construct takes, checked.

Each mutant undoes one piece of how a call through a function value, a `for`
loop's list, `?`'s operand, and the branches of an `if` or a `match` whose
value is used are related, or of how an `elif` chain is lowered. The
calls-and-branches tests and the if-chain conformance tests must then fail.

Run from the repository root; `just e10-calls` records the output.
The source is restored after every mutant, whatever happens.
"""

import pathlib
import re
import subprocess
import sys

ROOT = pathlib.Path(__file__).resolve().parent.parent
VALUES = ROOT / "compiler/pw-core/src/values.rs"
LOWER = ROOT / "compiler/pw-core/src/lower.rs"

# (what is undone, file, anchor, replacement)
MUTANTS = [
    (
        "a call through a binding is resolved as a declaration",
        VALUES,
        "            && self.lexical.binder(*callee).is_some()\n",
        "            && self.lexical.binder(*callee).is_some()\n            && false\n",
    ),
    (
        "a function value's arguments are not related",
        VALUES,
        "                    match unify(&mut Subst::default(), expected, &actual) {\n"
        "                        Verdict::Agree => Outcome::Agree,\n"
        "                        Verdict::Undecided => Outcome::Undecided(Undecided::Unknown),\n"
        "                        Verdict::Disagree => Outcome::Disagree {\n"
        "                            expected: self.display(expected),\n"
        "                            actual: self.display(&actual),\n"
        "                        },\n"
        "                    }\n"
        "                };\n"
        "                relations.push(relation(",
        "                    let _ = (expected, actual);\n"
        "                    Outcome::Agree\n"
        "                };\n"
        "                relations.push(relation(",
    ),
    (
        "a function value's arity is not related",
        VALUES,
        "            match supplied.len() == params.len() {",
        "            match true || supplied.len() == params.len() {",
    ),
    (
        "a function value's call answers nothing known",
        VALUES,
        "        Solved { result, relations }",
        "        let _ = result;\n        Solved {\n            result: Ty::Unknown,\n            relations,\n        }",
    ),
    (
        "a value that is not a function may be called",
        VALUES,
        "                        Outcome::Disagree {\n"
        '                            expected: "a function".to_string(),\n'
        "                            actual: self.display(&other),\n"
        "                        },",
        "                        {\n"
        "                            let _ = &other;\n"
        "                            Outcome::Agree\n"
        "                        },",
    ),
    (
        "a `for` loop's list is not related",
        VALUES,
        "                    Ty::Builtin(Builtin::List, _) | Ty::Any => Outcome::Agree,",
        "                    _ if true => Outcome::Agree,",
    ),
    (
        "`?`'s operand is not related",
        VALUES,
        "                    Ty::Builtin(Builtin::Option | Builtin::Result, _) | Ty::Any => Outcome::Agree,",
        "                    _ if true => Outcome::Agree,",
    ),
    (
        "branches are not related",
        VALUES,
        "        if !self.used.contains(&id) {",
        "        if true || !self.used.contains(&id) {",
    ),
    (
        "a statement's branches are related as a used value's",
        VALUES,
        "            if used && els.is_some() {",
        "            if els.is_some() {",
    ),
    (
        "an `elif` chain's next condition is its `else`",
        LOWER,
        "                let els = self.if_chain(b, kids.get(2..).unwrap_or_default(), &span);",
        "                let els = kids.get(2).map(|c| self.expr(b, c));",
    ),
]

TESTS = [
    ["cargo", "test", "--quiet", "--locked", "-p", "pw-core", "--test", "calls_and_branches"],
    ["cargo", "test", "--quiet", "--locked", "-p", "pw-conformance", "--test", "if_chains"],
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

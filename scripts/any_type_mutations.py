#!/usr/bin/env python3
"""Mutation controls for ADR-0065: a value that holds at every type.

Each mutant undoes one piece of how the value relations type a value no
declaration fixes a part of: `None`, `[]`, `Ok` and `Err`, `todo`, an early
return, a construction's parameter no field mentions, a variable only such a
value meets, and a `let mut`, which holds one type. The any-type tests must
then fail.

Run from the repository root; `just e10-any` records the output.
The source is restored after every mutant, whatever happens.
"""

import pathlib
import re
import subprocess
import sys

ROOT = pathlib.Path(__file__).resolve().parent.parent
VALUES = ROOT / "compiler/pw-core/src/values.rs"

# (what is undone, file, anchor, replacement)
MUTANTS = [
    (
        "a value of any type is undecided",
        VALUES,
        "        (Ty::Any, _) | (_, Ty::Any) => Verdict::Agree,",
        "        (Ty::Any, _) | (_, Ty::Any) => Verdict::Undecided,",
    ),
    (
        "a variable a value of any type meets is bound to it",
        VALUES,
        "        (Ty::Var(v), Ty::Any) | (Ty::Any, Ty::Var(v)) => {\n            s.any.insert(*v);",
        "        (Ty::Var(v), Ty::Any) | (Ty::Any, Ty::Var(v)) => {\n            s.bound.insert(*v, Ty::Any);",
    ),
    (
        "a variable only a value of any type meets is unknown",
        VALUES,
        "            Ty::Var(v) if self.any.contains(&v) => Ty::Any,\n",
        "",
    ),
    (
        "`None` is an option of something unstated",
        VALUES,
        '            "None" if own => Ty::Builtin(Builtin::Option, vec![Ty::Any]),',
        '            "None" if own => Ty::Builtin(Builtin::Option, vec![Ty::Unknown]),',
    ),
    (
        "`todo` is unknown",
        VALUES,
        '            "todo" if own => Ty::Any,',
        '            "todo" if own => Ty::Unknown,',
    ),
    (
        "`[]` is a list of something unstated",
        VALUES,
        "                        .unwrap_or(Ty::Any),",
        "                        .unwrap_or(Ty::Unknown),",
    ),
    (
        "`Err`'s success is unstated",
        VALUES,
        '            "Err" => Ty::Builtin(Builtin::Result, vec![Ty::Any, v]),',
        '            "Err" => Ty::Builtin(Builtin::Result, vec![Ty::Unknown, v]),',
    ),
    (
        "an early return's success is unstated",
        VALUES,
        "                Ty::Builtin(Builtin::Result, vec![Ty::Any, args[1].clone()])",
        "                Ty::Builtin(Builtin::Result, vec![Ty::Unknown, args[1].clone()])",
    ),
    (
        "a call's construction leaves its unmentioned parameters unknown",
        VALUES,
        "            s.any.extend(unmentioned(self.ws, binder, &params));",
        "            let _ = &params;",
    ),
    (
        "a record built by field names leaves its unmentioned parameters unknown",
        VALUES,
        "        s.any.extend(unmentioned(\n"
        "            self.ws,\n"
        "            def,\n"
        "            &declared\n"
        "                .iter()\n"
        "                .map(|(_, r)| Some(r.clone()))\n"
        "                .collect::<Vec<_>>(),\n"
        "        ));\n",
        "",
    ),
    (
        "a case without a payload leaves its parameters unknown",
        VALUES,
        "            any: unmentioned(\n"
        "                self.ws,\n"
        "                def,\n"
        "                &fields.iter().map(|f| Some(f.clone())).collect::<Vec<_>>(),\n"
        "            ),",
        "            any: BTreeSet::new(),",
    ),
    (
        "a `let mut` holds any type its first value holds at",
        VALUES,
        "                        Pattern::Bind { mutable: true, .. } => one_type(self.of(*init)),",
        "                        Pattern::Bind { mutable: true, .. } => self.of(*init),",
    ),
    (
        "an assignment does not complete a `let mut`'s type",
        VALUES,
        "                        added |= self.bind(b, one_type(self.of(*rhs)));",
        "                        let _ = (b, rhs);",
    ),
    (
        "a value of any type is no number",
        VALUES,
        "            Ty::Primitive(Primitive::Int | Primitive::Float) | Ty::Any => Outcome::Agree,",
        "            Ty::Primitive(Primitive::Int | Primitive::Float) => Outcome::Agree,",
    ),
]

TESTS = [
    ["cargo", "test", "--quiet", "--locked", "-p", "pw-core", "--test", "any_type"],
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

#!/usr/bin/env python3
"""Mutation controls for ADR-0089: a policy's value is one its domain has.

Each mutant undoes one piece: running the check, each domain's reading (a
word, a duration, a world, an operator and its arguments, a parameter or a
partition, a type), and the three readers that matched a value loosely
enough to be bypassed. The tests in `policy_values.rs` must then fail.

Run from the repository root; `just e10-policy-values` records the output.
The source is restored after every mutant, whatever happens.
"""

import pathlib
import re
import subprocess
import sys

ROOT = pathlib.Path(__file__).resolve().parent.parent
POLICY = ROOT / "compiler/pw-core/src/policy.rs"
CHECK = ROOT / "compiler/pw-core/src/check.rs"
RULES = ROOT / "compiler/pw-core/src/rules.rs"

# (what is undone, file, anchor, replacement)
MUTANTS = [
    (
        "a policy's value is not checked",
        CHECK,
        "        per_unit.extend(policy_values(&workspace, i, &u.hir));\n",
        "",
    ),
    (
        "a word outside the domain is a word of it",
        POLICY,
        "        Domain::Word(words) => (!words.contains(&value)).then_some(ValueFault::Word(words)),",
        "        Domain::Word(_) => None,",
    ),
    (
        "a duration is not read",
        POLICY,
        "        Domain::Duration => duration(value).is_none().then_some(ValueFault::Duration),",
        "        Domain::Duration => None,",
    ),
    (
        "any word is a world",
        POLICY,
        "            .find(|w| !crate::placement::ALL_WORLDS.iter().any(|x| x.name() == *w))",
        "            .find(|_| false)",
    ),
    (
        "an operator the head lacks is accepted",
        POLICY,
        "            let Some(op) = ops.iter().find(|o| o.name == name) else {\n                return Some(ValueFault::Operator(",
        "            let Some(op) = ops.iter().find(|o| o.name == name) else {\n                return None;\n                #[allow(unreachable_code)]\n                return Some(ValueFault::Operator(",
    ),
    (
        "an argument the operator lacks is accepted",
        POLICY,
        "                    return Some(ValueFault::UnknownArgument(op.name, name.to_string()));",
        "                    continue;",
    ),
    (
        "an argument of the wrong kind is accepted",
        POLICY,
        "                if !kind.fits(v) {",
        "                if false && !kind.fits(v) {",
    ),
    (
        "a required argument may be left out",
        POLICY,
        "        } if !given.contains(name) => Some(ValueFault::MissingArgument(op.name, name)),",
        "        } if false => Some(ValueFault::MissingArgument(op.name, name)),",
    ),
    (
        "a key may name anything",
        CHECK,
        "                        !decl.params.iter().any(|q| q.name == *n)\n                            && !(domain == Domain::ParamRef",
        "                        false && !decl.params.iter().any(|q| q.name == *n)\n                            && !(domain == Domain::ParamRef",
    ),
    (
        "a key may not name a partition",
        CHECK,
        "                            && !(domain == Domain::ParamRef\n                                && crate::privacy::Label::PARTITIONS.contains(n))",
        "                            && !(domain == Domain::ParamRef && false)",
    ),
    (
        "a type a policy names is not resolved",
        CHECK,
        "                    (!resolved).then(|| format!(\"`{value}` names no type visible here\"))",
        "                    (false && !resolved).then(|| format!(\"`{value}` names no type visible here\"))",
    ),
    (
        "a policy's type must be a declaration's",
        CHECK,
        "                    let resolved = crate::lower::type_fragment(value).is_some_and(|t| {",
        "                    let resolved = workspace.resolve_in(unit, crate::resolve::Namespace::Type, value)\n                        != Resolution::Unresolved\n                        && crate::lower::type_fragment(value).is_some_and(|t| {",
    ),
    (
        "a session read's staleness is read by its first character",
        RULES,
        "        && crate::policy::duration(&f.value).is_some_and(|ms| ms > 0)",
        "        && !f.value.starts_with('0')",
    ),
    (
        "a key's partition is found in its text",
        CHECK,
        "        .filter(|p| !items.contains(&p.as_str()))",
        "        .filter(|p| !key_text.contains(p.as_str()))",
    ),
    (
        "transport_only is found by its prefix",
        RULES,
        "        && !crate::policy::applied(\"retry\", &r.value)\n            .is_some_and(|o| o.id == \"policy.retry.transport_only\")",
        "        && !r.value.starts_with(\"transport_only\")",
    ),
]

TESTS = [
    ["cargo", "test", "--quiet", "--locked", "-p", "pw-core", "--test", "policy_values"],
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

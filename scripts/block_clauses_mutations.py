#!/usr/bin/env python3
"""Mutation controls for ADR-0247: a clause written in a block is judged by
its domain, as one heading a declaration is, and a length is a CSS length.

Each mutant undoes one piece: the names check keeping the clauses it reads,
each clause's whole value, without its layout, and where it is written; the
judging of a block's clauses and of a header's, and the note that tells
them apart; and the length judge: its count, whole or with a fraction, and
its unit, one CSS Values 4 defines. The tests of each must then fail.

Run from the repository root; `just e14-block-clauses` records the output.
The source is restored after every mutant, whatever happens.
"""

import pathlib
import re
import subprocess
import sys

ROOT = pathlib.Path(__file__).resolve().parent.parent
NAMES = ROOT / "compiler/pw-core/src/names.rs"
CHECK = ROOT / "compiler/pw-core/src/check.rs"
POLICY = ROOT / "compiler/pw-core/src/policy.rs"

# (what is undone, file, anchor, replacement)
MUTANTS = [
    (
        "a clause the walk reads is not kept",
        NAMES,
        "                    if crate::policy::domain_of(&head).is_some() {\n",
        "                    if crate::policy::domain_of(&head).is_some() && false {\n",
    ),
    (
        "a clause's value is its first statement",
        NAMES,
        "                        value = Some(value.map_or(at.clone(), |w| w.start..at.end));\n",
        "                        value = Some(value.map_or(at.clone(), |w| w));\n",
    ),
    (
        "a clause's value keeps its layout",
        NAMES,
        "                            .map(|v| pw_syntax::collapse_policy_whitespace(&self.src[v]))\n",
        "                            .map(|v| self.src[v].to_string())\n",
    ),
    (
        "a clause is placed at its head alone",
        NAMES,
        "                        self.clauses.push((head, written, start.start..end));\n",
        "                        self.clauses.push((head, written, start.clone()));\n",
    ),
    (
        "a clause written in a block is not judged",
        CHECK,
        "        per_unit.extend(clause_values(&workspace, i, &u.hir, &names.clauses));\n",
        "        per_unit.extend(clause_values(&workspace, i, &u.hir, &[]));\n",
    ),
    (
        "a policy heading a declaration is not judged",
        CHECK,
        "        for p in &decl.policies {\n"
        "            out.extend(policy_value(\n",
        "        for p in decl.policies.iter().filter(|_| false) {\n"
        "            out.extend(policy_value(\n",
    ),
    (
        "a clause is said to be a policy of its declaration",
        CHECK,
        '                "a clause in",\n',
        '                "a policy of",\n',
    ),
    (
        "a length is not judged",
        POLICY,
        "        Domain::Length => (!length(value)).then_some(ValueFault::Length),\n",
        "        Domain::Length => None,\n",
    ),
    (
        "a count with a fraction is not a count",
        POLICY,
        "        Some((whole, fraction)) => digits(whole) && digits(fraction),\n",
        "        Some((whole, fraction)) => false && digits(whole) && digits(fraction),\n",
    ),
    (
        "an empty count is a count",
        POLICY,
        "    let digits = |s: &str| !s.is_empty() && s.bytes().all(|b| b.is_ascii_digit());\n",
        "    let digits = |s: &str| s.bytes().all(|b| b.is_ascii_digit());\n",
    ),
    (
        "any word is a count",
        POLICY,
        "        None => digits(count),\n",
        "        None => !count.is_empty(),\n",
    ),
    (
        "any word is a unit",
        POLICY,
        "    count && LENGTH_UNITS.contains(&unit)\n",
        "    count && !unit.is_empty()\n",
    ),
    (
        "`px` is no unit",
        POLICY,
        '    "cm", "mm", "Q", "in", "pt", "pc", "px", // absolute\n',
        '    "cm", "mm", "Q", "in", "pt", "pc", // absolute\n',
    ),
]

TESTS = [
    [
        "cargo", "test", "--quiet", "--locked", "-p", "pw-core",
        "--test", "block_clauses", "--test", "policy_values", "--test", "every_name_resolves",
        "--test", "statements_separated", "--test", "checking_source",
    ],
    ["cargo", "test", "--quiet", "--locked", "-p", "pw-core", "--lib", "policy::"],
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

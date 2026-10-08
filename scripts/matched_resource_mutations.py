#!/usr/bin/env python3
"""Mutation controls for ADR-0269: a resource is held by what takes it apart.

Each mutant undoes one piece: a name an arm or a `?` binds counted as
holding what a binding carries; a match on the acquisition given to its
arms; an arm no carrying case reaches, and a `?`'s failure, owing nothing;
`Ok(_)`, a `_` that meets a carrying case, and a name that holds it whole,
each refused where it drops it; the arms after the carrying case taken
whole meeting none; `let h = r?` holding it; and `Some` named as a carrying
case. The tests of `compiler/pw-core/tests/matched_resources.rs` and
`let_discard.rs` must then fail.

Run from the repository root; `just e14-matched-resources` records the
output. The source is restored after every mutant, whatever happens.
"""

import pathlib
import re
import subprocess
import sys

ROOT = pathlib.Path(__file__).resolve().parent.parent
AFFINE = ROOT / "compiler/pw-core/src/affine.rs"

# (what is undone, file, anchor, replacement)
MUTANTS = [
    (
        "a name an arm or a `?` binds does not hold what it carries",
        AFFINE,
        "            .is_some_and(|b| b == a.binder || a.holders.contains(&b))\n",
        "            .is_some_and(|b| b == a.binder)\n",
    ),
    (
        "a match on the acquisition drops it, as before",
        AFFINE,
        "                    return held_by_arms(body, arms, p);\n",
        "                    return Holder::Nothing(Dropped::Matched(body.expr_span(p)));\n",
    ),
    (
        "an arm no carrying case reaches owes its release",
        AFFINE,
        "                        Some(ArmTakes::Nothing) => Flow::releasing(1).then(self.expr(a.body)),\n",
        "                        Some(ArmTakes::Nothing) => self.expr(a.body),\n",
    ),
    (
        "a `?`'s failure owes its release",
        AFFINE,
        "                    Flow::releasing(1).then(Flow::exit(span.clone()))\n",
        "                    Flow::exit(span.clone())\n",
    ),
    (
        "`Ok(_)` holds it",
        AFFINE,
        "                        Pattern::Wild => ArmTakes::Drops(body.pat_span(*q)),\n",
        "                        Pattern::Wild => ArmTakes::Nothing,\n",
    ),
    (
        "a `_` that meets a carrying case holds nothing",
        AFFINE,
        "            Pattern::Wild => {\n                taken = true;\n                ArmTakes::Drops(body.pat_span(arm.pat))\n",
        "            Pattern::Wild => {\n                taken = true;\n                ArmTakes::Nothing\n",
    ),
    (
        "an arm after the carrying case is taken whole still meets it",
        AFFINE,
        "            _ if taken => ArmTakes::Nothing,\n",
        "",
    ),
    (
        "a name that holds it whole holds nothing",
        AFFINE,
        "            Pattern::Bind { .. } | Pattern::Or(_) => {\n                taken = true;\n                ArmTakes::Whole(body.pat_span(arm.pat))\n",
        "            Pattern::Bind { .. } | Pattern::Or(_) => {\n                taken = true;\n                ArmTakes::Nothing\n",
    ),
    (
        "`let h = r?` does not hold it",
        AFFINE,
        "                holders.push(Binder::Pattern(*q));\n",
        "",
    ),
    (
        "a binding's arm that drops it is not refused",
        AFFINE,
        "                            fault.get_or_insert(Fault::Dropped(body.pat_span(arm.pat)));\n",
        "",
    ),
    (
        "`Some` is no carrying case",
        AFFINE,
        '    let carrying = |path: &str| matches!(path, "Ok" | "Result.Ok" | "Some" | "Option.Some");\n',
        '    let carrying = |path: &str| matches!(path, "Ok" | "Result.Ok");\n',
    ),
]

TESTS = [
    ["cargo", "test", "--quiet", "--locked", "-p", "pw-core", "--test", "matched_resources"],
    ["cargo", "test", "--quiet", "--locked", "-p", "pw-core", "--test", "let_discard"],
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

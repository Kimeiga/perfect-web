#!/usr/bin/env python3
"""Mutation controls for ADR-0129: a value's label follows it through calls,
bodies, branches and assignments.

Each mutant undoes one piece: an argument coming out of a call, a key left
out of it, a callee's body, the summary pass's fixed point, a branch's value
and a `match`'s, a sink told what decided it, a `?` and a lambda's driving
arguments as conditions, an assignment, the public log refusing a reader's
value, and PW5003 reading every restriction. The tests in
`labels_follow_values.rs` must then fail.

Run from the repository root; `just e14-value-labels` records the output.
The source is restored after every mutant, whatever happens.
"""

import pathlib
import re
import subprocess
import sys

ROOT = pathlib.Path(__file__).resolve().parent.parent
CHECK = ROOT / "compiler/pw-core/src/check.rs"
LABELS = ROOT / "compiler/pw-core/src/labels.rs"
PRIVACY = ROOT / "compiler/pw-core/src/privacy.rs"

# (what is undone, file, anchor, replacement)
MUTANTS = [
    (
        "an argument a labelled parameter is given stays out",
        PRIVACY,
        "                .filter(|r| !(matches!(r, Restriction::Secret(_)) && parameter.0.contains(r)))\n",
        "                .filter(|_| parameter.0.is_empty())\n",
    ),
    (
        "a key a parameter states comes out",
        PRIVACY,
        "                .filter(|r| !(matches!(r, Restriction::Secret(_)) && parameter.0.contains(r)))\n",
        "                .filter(|_| true)\n",
    ),
    (
        "a call is read by its signature alone",
        LABELS,
        "            .and_then(|s| s.get(&def))\n",
        "            .and_then(|_| None::<&Label>)\n",
    ),
    (
        "the summary pass stops after one step",
        CHECK,
        "        if next == out {\n            return out;\n        }\n",
        "        if next == out || true {\n            return next;\n        }\n",
    ),
    (
        "an `if`'s value does not carry its condition",
        LABELS,
        "                let l = self.label(body, *then).join(&self.label(body, *cond));\n",
        "                let l = self.label(body, *then);\n",
    ),
    (
        "a `match`'s value does not carry its subject",
        LABELS,
        "                arms.iter().fold(self.label(body, *scrutinee), |acc, a| {\n",
        "                arms.iter().fold(Label::public(), |acc, a| {\n",
    ),
    (
        "a sink is not told what decided it",
        CHECK,
        "        let Some((condition, cause)) = labels.condition(id) else {\n",
        "        let Some((condition, cause)) = labels.condition(id).filter(|_| false) else {\n",
    ),
    (
        "a `?` that may fail tells nothing",
        LABELS,
        "                    for t in tried(body, *s) {\n",
        "                    for t in tried(body, *s).into_iter().filter(|_| false) {\n",
    ),
    (
        "a lambda is not told what drives it",
        LABELS,
        "                    self.under(body, a.value, &inner, out);\n",
        "                    self.under(body, a.value, pc, out);\n",
    ),
    (
        "an assignment does not label its binding",
        LABELS,
        "                let Some(b) = me.types.lexical().binder(target) else {\n",
        "                let Some(b) = me.types.lexical().binder(target).filter(|_| false) else {\n",
    ),
    (
        "a public log takes a reader's value",
        CHECK,
        "            if label.is_public() {\n                continue;\n            }\n            refused = true;\n",
        "            if !label.holds_a_secret() {\n                continue;\n            }\n            refused = true;\n",
    ),
    (
        "PW5003 reads the first restriction",
        CHECK,
        "            let Some(cap) = value_label.secret_capabilities().next() else {\n",
        "            let Some(cap) = value_label.restrictions().next().and_then(|r| match r { Restriction::Secret(c) => Some(c.as_str()), _ => None }) else {\n",
    ),
]

TESTS = [
    ["cargo", "test", "--quiet", "--locked", "-p", "pw-core", "--test", "labels_follow_values"],
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

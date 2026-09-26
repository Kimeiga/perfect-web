#!/usr/bin/env python3
"""Mutation controls for ADR-0048: a read names a member its value's type has.

Each mutant undoes one piece of the member relation, the rule for an opaque
type's `.value`, the reading of an optional's contents, or the one-finding
deduplication, and the member tests must then fail.

Run from the repository root; `just e10-members` records the output. The
source is restored after every mutant, whatever happens.
"""

import pathlib
import re
import subprocess
import sys

ROOT = pathlib.Path(__file__).resolve().parent.parent
VALUES = ROOT / "compiler/pw-core/src/values.rs"
CHECK = ROOT / "compiler/pw-core/src/check.rs"

# (what is undone, file, anchor, replacement)
MUTANTS = [
    (
        "no member relation is recorded",
        VALUES,
        "Expr::Field { base, name } => out.extend(self.member(id, *base, name)),",
        "Expr::Field { .. } => {}",
    ),
    (
        "every known type has every member",
        VALUES,
        "            Some(r) if self.sigs.member_by(r, name).is_some() => Outcome::Agree,\n",
        "            Some(_) if true => Outcome::Agree,\n",
    ),
    (
        "a declaration taking the type is not a member",
        VALUES,
        "            Some(r) if self.sigs.member_by(r, name).is_some() => Outcome::Agree,\n",
        # Only a record's fields: a field's signature is defined by the type,
        # which has no callable signature of its own.
        "            Some(r) if self.sigs.member_by(r, name).is_some_and(|s| self.sigs.by_def(s.definition).is_none()) => Outcome::Agree,\n",
    ),
    (
        "a unit on a number is read as a member",
        VALUES,
        """            || matches!(
                self.body.expr(base),
                Expr::Literal(Literal::Int(_) | Literal::Float(_))
            )
        {
            return None;
        }
        let receiver = self.of(base);
        let outcome = match receiver.receiver() {""",
        """        {
            return None;
        }
        let receiver = self.of(base);
        let outcome = match receiver.receiver() {""",
    ),
    (
        "a module path is recorded as a member",
        VALUES,
        """        if self.resolves_as_path(id)
            || matches!(
                self.body.expr(base),""",
        """        if false
            || matches!(
                self.body.expr(base),""",
    ),
    (
        "an opaque type's value is readable from any module",
        VALUES,
        "                Some((def, _)) if def.unit == self.at => Outcome::Agree,\n",
        "                Some((_def, _)) => Outcome::Agree,\n",
    ),
    (
        "an opaque type's value is refused in its own module",
        VALUES,
        "                Some((def, _)) if def.unit == self.at => Outcome::Agree,\n",
        "",
    ),
    (
        "an opaque type's value has no type",
        VALUES,
        """            return match self.representation(&receiver, name) {
                Some((def, rep)) if def.unit == self.at => rep,
                _ => Ty::Unknown,
            };""",
        """            return Ty::Unknown;""",
    ),
    (
        "an optional's contents are an ordinary missing member",
        VALUES,
        "                None if self.contents_have(&receiver, name) => Outcome::Disagree {",
        "                None if false => Outcome::Disagree {",
    ),
    (
        "one defect is reported by every rule that reaches it",
        CHECK,
        """    let mut seen = BTreeSet::new();
    out.retain(|d| seen.insert((d.code, d.primary_span.start, d.primary_span.end)));
""",
        "",
    ),
]

TESTS = [
    ["cargo", "test", "--quiet", "--locked", "-p", "pw-core", "--test", "members"],
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

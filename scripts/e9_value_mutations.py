#!/usr/bin/env python3
"""E9-V mutation controls: each gate's mechanism, disabled, must fail its tests.

A test suite that passes against a checker which cannot decide anything is not
evidence. So each E9-V gate names the piece of `compiler/pw-core/src/values.rs`
(or the resolver) that decides it, this script disables that piece, and the
gate's tests in `tests/value_relations.rs` must go red. The source is restored
after every mutant, whatever happens.

Exit status is non-zero if any mutant SURVIVES (its tests still pass), if a
mutation no longer applies to the source (the anchor text moved: the control
would otherwise pass vacuously), or if the unmutated baseline is not green.
"""

import pathlib
import re
import subprocess
import sys

ROOT = pathlib.Path(__file__).resolve().parent.parent
VALUES = ROOT / "compiler/pw-core/src/values.rs"
RESOLVED = ROOT / "compiler/pw-core/src/resolved.rs"

# (gate, what is disabled, file, anchor, replacement)
MUTANTS = [
    (
        "V1",
        "primitive types always agree",
        VALUES,
        """        (Ty::Primitive(p), Ty::Primitive(q)) => match p == q {
            true => Verdict::Agree,
            false => Verdict::Disagree,
        },""",
        """        (Ty::Primitive(_), Ty::Primitive(_)) => Verdict::Agree,""",
    ),
    (
        "V1",
        "arity is never compared",
        VALUES,
        "let arity_ok = supplied.len() == params.len();",
        "let arity_ok = true || supplied.len() == params.len();",
    ),
    (
        "V2",
        "a generic callee is not instantiated",
        VALUES,
        "            Ty::Parameter { binder: b, index } if *b == binder => Ty::Var(*index),",
        "            Ty::Parameter { binder: b, index } if *b == binder && false => Ty::Var(*index),",
    ),
    (
        "V2",
        "a bound variable is not followed, so each argument rebinds it",
        VALUES,
        """                Some(b) => self.resolve(b),""",
        """                Some(_) => Ty::Var(*v),""",
    ),
    (
        "V3",
        "nominal identity ignores the declaration",
        VALUES,
        """            if d1 != d2 {
                return Verdict::Disagree;
            }""",
        """            if d1 != d2 && false {
                return Verdict::Disagree;
            }""",
    ),
    (
        "V4",
        "no result site is collected",
        VALUES,
        "        self.tail_sites(self.body.root, &mut out);",
        "        let _ = self.body.root;",
    ),
    (
        "V4",
        "a `?` propagates nothing",
        VALUES,
        """        let returned = match typer.of(*value) {""",
        """        let returned = match Ty::Unknown {""",
    ),
    (
        "V5",
        "an unresolved annotation is accepted",
        VALUES,
        """                TypeResolution::Unresolved { name, written } => Outcome::Disagree {
                    expected: why_unresolved(ws, at, name, written),
                    actual: written.clone(),
                },""",
        """                TypeResolution::Unresolved { .. } => Outcome::Agree,""",
    ),
    (
        "V5",
        "a declared constructor's arity is not checked",
        RESOLVED,
        """    if let Some(arity) = ws.type_arity(def)
        && arity != args.len()""",
        """    if let Some(arity) = ws.type_arity(def)
        && arity != args.len()
        && false""",
    ),
    (
        "V6",
        "an opaque type is compared by its representation",
        VALUES,
        """            TypeKey::Nominal(d, args) => Ty::Nominal(*d, args.iter().map(Ty::from_key).collect()),""",
        """            TypeKey::Nominal(d, args) => {
                let _ = (d, args);
                Ty::Primitive(Primitive::Str)
            }""",
    ),
]

TEST = ["cargo", "test", "--quiet", "--locked", "-p", "pw-core", "--test", "value_relations"]


def run_tests():
    r = subprocess.run(TEST, cwd=ROOT, capture_output=True, text=True)
    out = r.stdout + r.stderr
    m = re.search(r"test result: \w+\. (\d+) passed; (\d+) failed", out)
    compiled = m is not None
    passed, failed = (int(m.group(1)), int(m.group(2))) if m else (0, 0)
    return r.returncode, compiled, passed, failed


def main():
    code, compiled, passed, failed = run_tests()
    print(f"baseline: {passed} passed, {failed} failed (exit {code})")
    if code != 0 or failed or not passed:
        print("FAIL: the unmutated baseline is not green; no mutant can mean anything")
        return 1

    survivors = 0
    for gate, what, path, anchor, replacement in MUTANTS:
        original = path.read_text()
        if original.count(anchor) != 1:
            print(f"{gate}  {what}: ANCHOR NOT FOUND EXACTLY ONCE in {path.name}")
            survivors += 1
            continue
        try:
            path.write_text(original.replace(anchor, replacement, 1))
            code, compiled, passed, failed = run_tests()
        finally:
            path.write_text(original)
        if not compiled:
            verdict = "KILLED (does not build)"
        elif failed:
            verdict = f"KILLED ({failed} of {passed + failed} tests fail)"
        else:
            verdict = "SURVIVED"
            survivors += 1
        print(f"{gate}  {what}: {verdict}")

    print(f"{len(MUTANTS) - survivors} of {len(MUTANTS)} mutants killed")
    return 1 if survivors else 0


if __name__ == "__main__":
    sys.exit(main())

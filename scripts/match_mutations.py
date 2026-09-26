#!/usr/bin/env python3
"""Mutation controls for ADR-0038: every match is analysed, against its types.

A test that passes with the mechanism removed is not evidence for it. Each
mutant below undoes one piece of ADR-0038, and at least one of the tests must
then fail or not build. A mutant that survives is a fix nothing holds.

Run from the repository root; `just e10-match` records the output. The sources
are restored after every mutant, whatever happens.
"""

import pathlib
import re
import subprocess
import sys

ROOT = pathlib.Path(__file__).resolve().parent.parent
CHECK = ROOT / "compiler/pw-core/src/check.rs"
EXHAUST = ROOT / "compiler/pw-core/src/exhaust.rs"
VALUES = ROOT / "compiler/pw-core/src/values.rs"
GRAMMAR = ROOT / "compiler/pw-syntax/src/grammar.rs"

# (what is undone, file, anchor, replacement)
MUTANTS = [
    (
        "the scrutinee is not typed by the value relations",
        CHECK,
        "    match crate::values::type_of(sigs, ws, *at, *module, decl, body, scrutinee) {",
        "    let _ = (sigs, module, decl);\n    match (crate::values::Ty::Unknown, String::new()) {",
    ),
    (
        "`Option` and `Result` are not sum types to the analysis",
        CHECK,
        "            Ty::Builtin(Builtin::Option, a) if a.len() == 1 => {",
        "            Ty::Builtin(Builtin::Option, a) if a.len() == 1 && a.is_empty() => {",
    ),
    (
        "a match arm's names are not typed",
        VALUES,
        "        let mut out = Vec::new();\n        self.bind_pattern(scrutinee, pat, &mut out);\n        out\n",
        "        let _ = (scrutinee, pat);\n        Vec::new()\n",
    ),
    (
        "a shadowed parameter is read as the parameter",
        CHECK,
        "        && !rebinds(body, n)",
        "        && (!rebinds(body, n) || true)",
    ),
    (
        "a nested pattern is read against the scrutinee's type",
        CHECK,
        "                        *a,\n                        t,\n                        &program.type_name(t),",
        "                        *a,\n                        ty,\n                        ty_name,",
    ),
    (
        "a constructor its type lacks reads as a wildcard",
        CHECK,
        "                return Err(foreign(short, &cs));",
        "                return Ok(EPat::Wildcard);",
    ),
    (
        "another type's bare constructor reads as a binding",
        CHECK,
        '            if env.names_a_constructor(name) || matches!(name.as_str(), "true" | "false") {',
        '            if (env.names_a_constructor(name) || matches!(name.as_str(), "true" | "false")) && false {',
    ),
    (
        "a literal reads as a wildcard",
        CHECK,
        "        HPat::Literal(l) => {\n            let key = match (ty, l) {",
        "        HPat::Literal(l) => {\n            if literals.borrow().len() < usize::MAX {\n"
        "                return Ok(EPat::Wildcard);\n            }\n            let key = match (ty, l) {",
    ),
    (
        "a field count is checked only at the top of an arm",
        CHECK,
        "            if args.len() != fields.len() {",
        "            if args.len() != fields.len() && false {",
    ),
    (
        "one arm's fault hides the others'",
        CHECK,
        "    for fault in read.into_iter().filter_map(Result::err) {",
        "    for fault in read.into_iter().filter_map(Result::err).take(1) {",
    ),
    (
        "the analysis validates nested patterns against the scrutinee's constructors",
        EXHAUST,
        "                stack.extend(args.iter().zip(c.fields));",
        "                stack.extend(args.iter().map(|a| (a, ty.clone())));",
    ),
    (
        "an arm ends at `return`",
        GRAMMAR,
        "        if returns\n            && !self.newline_ahead()",
        "        if returns\n            && false\n            && !self.newline_ahead()",
    ),
    (
        "`return` is an operand",
        GRAMMAR,
        '        if self.at_kw("return") {\n            self.start(K::NameExpr);',
        '        if self.at_kw("return") && false {\n            self.start(K::NameExpr);',
    ),
    (
        "an arm without `=>` is accepted silently",
        GRAMMAR,
        "                if self.expect(Kind::FatArrow, \"after a match arm's pattern\") {",
        "                if self.eat(Kind::FatArrow) {",
    ),
]

TESTS = [
    ["cargo", "test", "--quiet", "--locked", "-p", "pw-core", "--test", "match_exhaustiveness"],
    ["cargo", "test", "--quiet", "--locked", "-p", "pw-core", "--lib", "exhaust::"],
    ["cargo", "test", "--quiet", "--locked", "-p", "pw-syntax", "--lib"],
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

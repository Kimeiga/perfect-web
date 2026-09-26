#!/usr/bin/env python3
"""Mutation controls for ADR-0074: what a template renders has a text form.

Each mutant undoes one piece of how a template's written values are related:
a text hole, an attribute and an interpolated attribute's holes to a type
with a text form; a boolean attribute to a truth; a value that may be absent,
in those places and in `{:else if}`; a loop's list and key to fields their
values have; and the attributes that are not text. The template tests must
then fail.

Run from the repository root; `just e10-template-text` records the output.
The source is restored after every mutant, whatever happens.
"""

import pathlib
import re
import subprocess
import sys

ROOT = pathlib.Path(__file__).resolve().parent.parent
VALUES = ROOT / "compiler/pw-core/src/values.rs"
NAMES = ROOT / "compiler/pw-core/src/names.rs"

# (what is undone, file, anchor, replacement)
MUTANTS = [
    (
        "a text hole is not related",
        VALUES,
        '                    out.extend(self.text(self.of(*e), "{..}".to_string(), self.body.expr_span(*e)));',
        "                    let _ = e;",
    ),
    (
        "an attribute's value is not related",
        VALUES,
        "                                _ => out.extend(self.text(\n"
        "                                    self.of(*e),\n"
        "                                    a.name.clone(),\n"
        "                                    self.body.expr_span(*e),\n"
        "                                )),",
        "                                _ => {}",
    ),
    (
        "an interpolated attribute's holes are not related",
        VALUES,
        "                                    for h in parts {\n"
        "                                        out.extend(self.text(\n"
        "                                            self.of(*h),\n"
        "                                            a.name.clone(),\n"
        "                                            self.body.expr_span(*h),\n"
        "                                        ));\n"
        "                                    }",
        "                                    let _ = parts;",
    ),
    (
        "a boolean attribute is written as text",
        VALUES,
        "                                _ if crate::template_ir::BOOLEAN_ATTRIBUTES",
        "                                _ if false && crate::template_ir::BOOLEAN_ATTRIBUTES",
    ),
    (
        "any value has a text form",
        VALUES,
        "                None => Outcome::Disagree {\n"
        '                    expected: "a `String`, an `Int` or a `Bool`".to_string(),\n'
        "                    actual: self.display(other),\n"
        "                },",
        "                None => Outcome::Agree,",
    ),
    (
        "an opaque type is not written as its representation",
        VALUES,
        "                Some((_, rep)) => return self.text(rep, target, span),",
        "                Some(_) => Outcome::Disagree {\n"
        "                    expected: String::new(),\n"
        "                    actual: self.display(other),\n"
        "                },",
    ),
    (
        "a value that may be absent is written as text",
        VALUES,
        "            Ty::Builtin(Builtin::Option | Builtin::Result, _) => {\n"
        "                return Some(self.absent(&t, target, span));\n"
        "            }",
        "            Ty::Builtin(Builtin::Option | Builtin::Result, _) => Outcome::Agree,",
    ),
    (
        "an `{:else if}` over a value that may be absent is related to nothing",
        VALUES,
        "            Ty::Builtin(Builtin::Option | Builtin::Result, _) => {\n"
        "                return Some(self.absent(&t, block.to_string(), self.body.expr_span(c)));\n"
        "            }",
        "            Ty::Builtin(Builtin::Option | Builtin::Result, _) => return None,",
    ),
    (
        "a loop's key is not written as text",
        VALUES,
        '                out.extend(self.text(t, format!("({key})"), span.clone()));',
        "                let _ = t;",
    ),
    (
        "a loop's list is not read field by field",
        VALUES,
        "                self.read_through(start, &rest, &span, out);",
        "                let _ = (start, rest);",
    ),
    (
        "a loop's key is its element whatever it names",
        VALUES,
        "            if let Some(t) = self.read_through(element, &segments[1..], &span, out) {",
        "            if let Some(t) = Some(element) {",
    ),
    (
        "a field a value lacks is read",
        VALUES,
        '                Some(format!("a member `{name}`"))',
        "                None",
    ),
    (
        "a computed list is read as a name",
        NAMES,
        "if written && !self.bound(head) && !self.resolves(head) {",
        "if !head.is_empty() && !self.bound(head) && !self.resolves(head) {",
    ),
    (
        "a stream's query is written as text",
        VALUES,
        '                            && !(tag == "stream" && a.name == "query")\n',
        "",
    ),
    (
        "a stream part's name is written as text",
        VALUES,
        '                            && !(matches!(tag.as_str(), "ready" | "failed") && a.name == "as");',
        ";",
    ),
    (
        "a mounted resource's arguments are written as text",
        VALUES,
        '                    let mounts = attrs.iter().any(|a| a.name == "resource");',
        "                    let mounts = false;",
    ),
]

TESTS = [
    ["cargo", "test", "--quiet", "--locked", "-p", "pw-core", "--test", "template_text"],
    ["cargo", "test", "--quiet", "--locked", "-p", "pw-core", "--test", "template_values"],
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

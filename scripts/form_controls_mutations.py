#!/usr/bin/env python3
"""Mutation controls for ADR-0221: a form control's value is written where
HTML reads it.

Each mutant undoes one piece: a `<textarea>`'s value written as its text, by
the template and by the renderer, a newline it starts with kept, a patch's
value; the browser setting it in place; and PW5036's refusals, in the check
and in the template behind it. The tests of each must then fail.

Run from the repository root; `just e14-form-controls` records the output.
The source is restored after every mutant, whatever happens.
"""

import pathlib
import re
import subprocess
import sys

ROOT = pathlib.Path(__file__).resolve().parent.parent
TEMPLATE = ROOT / "compiler/pw-core/src/template_ir.rs"
SIGNALS = ROOT / "compiler/pw-core/src/signals.rs"
PLAN = ROOT / "compiler/pw-core/src/page_values.rs"
RENDER = ROOT / "runtime/pw-render/src/lib.rs"
ESCAPE = ROOT / "runtime/pw-render/src/escape.rs"

# (what is undone, file, anchor, replacement)
MUTANTS = [
    (
        "a textarea's value is written as an attribute",
        TEMPLATE,
        '        if control == "textarea" && is_value(a) {\n',
        '        if false && control == "textarea" && is_value(a) {\n',
    ),
    (
        "the renderer writes a textarea's value as an attribute",
        RENDER,
        "            out.push_str(&escape::content(&s));\n",
        '            out.push_str(&format!("value=\\"{}\\"", escape::attribute(&s)));\n',
    ),
    (
        "a newline a value starts with is dropped",
        ESCAPE,
        "    let lead = if value.starts_with(['\\n', '\\r']) {\n",
        "    let lead = if false {\n",
    ),
    (
        "a patch carries a textarea's value as text",
        RENDER,
        "        Context::Content => escape::attribute(value),\n",
        "        Context::Content => escape::content(value),\n",
    ),
    (
        "the browser does not set a textarea's value in place",
        PLAN,
        "            matches!(context, Context::Attribute | Context::Content)\n",
        "            matches!(context, Context::Attribute)\n",
    ),
    (
        "a select's value is not refused",
        SIGNALS,
        '            ("select", Some(_)) => Some(format!(\n',
        '            ("select", Some(_)) if false => Some(format!(\n',
    ),
    (
        "a textarea with a value and text is not refused",
        SIGNALS,
        '            ("textarea", Some(_)) if !children.is_empty() => Some(\n',
        '            ("textarea", Some(_)) if false => Some(\n',
    ),
    (
        "a textarea's value read from no signal is not refused",
        SIGNALS,
        "                AttrValue::Expr(e) if matches!(body.expr(*e), Expr::Name(_)) && is_signal(*e) => {\n",
        "                AttrValue::Expr(_) => {\n",
    ),
    (
        "a value in a textarea's text is not refused",
        SIGNALS,
        '            ("textarea", None)\n                if children\n',
        '            ("textarea", None)\n                if false && children\n',
    ),
    (
        "the template writes a select's value",
        TEMPLATE,
        '        if control == "select" && is_value(a) {\n',
        '        if false && control == "select" && is_value(a) {\n',
    ),
    (
        "the template writes a textarea's value and its text",
        TEMPLATE,
        "            Some(_) if !children.is_empty() => out.push(Chunk::Dynamic(Part::Blocked {\n",
        "            Some(_) if false => out.push(Chunk::Dynamic(Part::Blocked {\n",
    ),
    (
        "the template writes a value in a textarea's text",
        TEMPLATE,
        "            None if !written => out.push(Chunk::Dynamic(Part::Blocked {\n",
        "            None if false => out.push(Chunk::Dynamic(Part::Blocked {\n",
    ),
]

TESTS = [
    ["cargo", "test", "--quiet", "--locked", "-p", "pw-core", "--test", "form_controls", "--test", "bind"],
    ["cargo", "test", "--quiet", "--locked", "-p", "pw-render", "--test", "security"],
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

#!/usr/bin/env python3
"""Mutation controls for ADR-0061: a declared sum type in a template's match.

Each mutant undoes one piece of how a `{#match}` arm over a declared sum
type is parsed, checked, written into the template IR, rendered, or given
its value by a server. The template-block tests, the renderer's tests, and
kiokun's server's conversion test must then fail.

Run from the repository root; `just e10-template-sum-types` records the
output. The source is restored after every mutant, whatever happens.
"""

import pathlib
import re
import subprocess
import sys

ROOT = pathlib.Path(__file__).resolve().parent.parent
CHECK = ROOT / "compiler/pw-core/src/check.rs"
HIR_LOWER = ROOT / "compiler/pw-core/src/lower.rs"
TEMPLATE_IR = ROOT / "compiler/pw-core/src/template_ir.rs"
RENDER = ROOT / "runtime/pw-render/src/lib.rs"
KIOKUN = ROOT / "spikes/kiokun/server/src/app.rs"

# (what is undone, file, anchor, replacement)
MUTANTS = [
    (
        "a template match over a declared type is refused again",
        CHECK,
        "                                let vs = sigs.type_decl(d)?.variants.as_ref()?;",
        "                                let vs = sigs.type_decl(d)?.variants.as_ref().filter(|_| false)?;",
    ),
    (
        "a case the type lacks is read as its first",
        CHECK,
        "                    let Some((_, fields)) = declared.iter().find(|(c, _)| *c == short) else {",
        "                    let Some((_, fields)) = declared\n"
        "                        .iter()\n"
        "                        .find(|(c, _)| *c == short)\n"
        "                        .or(declared.first())\n"
        "                    else {",
    ),
    (
        "an arm's field count is not checked",
        CHECK,
        "                    if !arm.bindings.is_empty() && arm.bindings.len() != *fields {",
        "                    if false && arm.bindings.len() != *fields {",
    ),
    (
        "an arm's qualifier is not checked",
        CHECK,
        "                        if !same {",
        "                        if false && !same {",
    ),
    (
        "a qualified arm does not parse",
        HIR_LOWER,
        "    if !case.starts_with(|c: char| c.is_uppercase()) || !name.split('.').all(ident) {",
        "    if !case.starts_with(|c: char| c.is_uppercase()) || !ident(name) {",
    ),
    (
        "an arm of several names does not parse",
        HIR_LOWER,
        "            let names: Vec<&str> = f.split(',').map(str::trim).collect();",
        "            let names: Vec<&str> = vec![f];",
    ),
    (
        "a declared case keeps its Pleris name in the IR",
        TEMPLATE_IR,
        "                c => crate::wit::ident(c),",
        "                c => c.to_string(),",
    ),
    (
        "a case of several fields binds only its first",
        TEMPLATE_IR,
        "                many => (None, many.to_vec()),",
        "                many => (many.first().cloned(), Vec::new()),",
    ),
    (
        "the renderer binds no field",
        RENDER,
        "                    scoped = scoped.with(name, part.clone());",
        "                    let _ = (name, part);",
    ),
    (
        "a server drops a declared case",
        KIOKUN,
        "        Val::Variant(name, payload) => case(name, payload.as_deref())?,\n",
        "",
    ),
]

TESTS = [
    ["cargo", "test", "--quiet", "--locked", "-p", "pw-core", "--test", "template_blocks"],
    ["cargo", "test", "--quiet", "--locked", "-p", "pw-render", "--test", "branches"],
    ["cargo", "test", "--quiet", "--locked", "-p", "kiokun-server", "a_declared_case"],
]


def run_tests():
    """(built, passed, failed) over every test command."""
    built, passed, failed = True, 0, 0
    for cmd in TESTS:
        r = subprocess.run(cmd, cwd=ROOT, capture_output=True, text=True)
        out = r.stdout + r.stderr
        found = re.findall(r"test result: \w+\. (\d+) passed; (\d+) failed", out)
        if not found:
            built = False
            continue
        for p, f in found:
            passed += int(p)
            failed += int(f)
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

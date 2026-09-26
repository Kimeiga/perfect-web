#!/usr/bin/env python3
"""Mutation controls for ADR-0042: `{:else}`, `{#match}` and interpolated
attributes in templates.

A test that passes with the mechanism removed is not evidence for it. Each
mutant undoes one piece of ADR-0042, and at least one test must then fail or
not build. The first is the defect ADR-0042 found: the lowering that dropped
`{:else}`, so both branches of an `if` rendered together.

`evidence_is_current` is left out of the tests run here: it would kill every
mutant that changes kiokun's committed build by noticing only that the bytes
differ, which says nothing about what they mean.

Run from the repository root; `just e10-templates` records the output. The
sources are restored after every mutant, whatever happens.
"""

import pathlib
import re
import subprocess
import sys

ROOT = pathlib.Path(__file__).resolve().parent.parent
LOWER = ROOT / "compiler/pw-core/src/lower.rs"
IR = ROOT / "compiler/pw-core/src/template_ir.rs"
CHECK = ROOT / "compiler/pw-core/src/check.rs"
INFER = ROOT / "compiler/pw-core/src/infer.rs"
RENDER = ROOT / "runtime/pw-render/src/lib.rs"

# (what is undone, file, anchor, replacement)
MUTANTS = [
    (
        "the lowering drops `{:else}` again",
        LOWER,
        "                    children.push(match is_block_marker(self.src, &c) {\n"
        "                        true => self.branch(b, &c),\n"
        "                        false => self.markup(b, &c),\n"
        "                    });",
        "                    if is_block_marker(self.src, &c) {\n"
        "                        continue;\n"
        "                    }\n"
        "                    children.push(self.markup(b, &c));",
    ),
    (
        "an attribute's holes are static text again",
        LOWER,
        "                    let parts = match raw.contains('{') {",
        "                    let parts = match false {",
    ),
    (
        "`{:else}` renders nothing",
        IR,
        "=>\n            {\n                lower_run(body, run, ctx, ix)\n            }",
        "=>\n            {\n                Vec::new()\n            }",
    ),
    (
        "`{:else if}` loses its branch",
        IR,
        "Some(v) => vec![conditional(body, nested, v, run, more, ctx, ix)],",
        "Some(v) => vec![conditional(body, nested, v, &[], more, ctx, ix)],",
    ),
    (
        "a URL may begin with a value",
        IR,
        "    if context == Context::Url && !matches!(segments.first(), Some(Segment::Static(_))) {",
        "    if false && !matches!(segments.first(), Some(Segment::Static(_))) {",
    ),
    (
        "a block may close with another's name",
        CHECK,
        "        } else if closer != name {",
        "        } else if false {",
    ),
    (
        "an unknown directive passes `pw check`",
        CHECK,
        '        if !matches!(name.as_str(), "if" | "each" | "match") {',
        "        if false {",
    ),
    (
        "an `{#if}` takes any marker",
        CHECK,
        '                    if !(*else_if || (marker == "{:else}" && last)) {',
        "                    if false {",
    ),
    (
        "`{:else}` in `{#each}` passes",
        CHECK,
        "                if let Some((b, marker, ..)) = branches.first() {",
        "                if let Some((b, marker, ..)) = branches.first().filter(|_| false) {",
    ),
    (
        "a template match need not be exhaustive",
        CHECK,
        "                if !missing.is_empty() {",
        "                if false {",
    ),
    (
        "`{#if}` over an Option passes",
        CHECK,
        "                    && matches!(ty.as_builtin(), Some(Builtin::Option | Builtin::Result))",
        "                    && matches!(ty.as_builtin(), Some(Builtin::Function))",
    ),
    (
        "an arm's binding has no type",
        INFER,
        "                types.bindings.insert(Binder::Arm(n, 0), p);",
        "                let _ = p;",
    ),
    (
        "a match renders its first arm whatever the case",
        RENDER,
        "            let arm = arms.iter().find(|a| a.case == *case).ok_or_else(|| {",
        "            let arm = arms.first().ok_or_else(|| {",
    ),
    (
        "an arm's payload is not bound",
        RENDER,
        "                (Some(name), Some(p)) => env.with(name, (**p).clone()),",
        "                (Some(_), Some(_)) => env.clone(),",
    ),
    (
        "a value in a URL is attribute-escaped, not one component",
        RENDER,
        "                            Context::Url => escape::url_component(&v),",
        "                            Context::Url => escape::attribute(&v),",
    ),
    (
        "an Option reads as a condition",
        RENDER,
        "            Value::Variant { .. } => Err(Blocked::UnrepresentedConstruct {",
        "            Value::Bool(_) if false => Err(Blocked::UnrepresentedConstruct {",
    ),
]

TESTS = [
    ["cargo", "test", "--quiet", "--locked", "-p", "pw-core", "--test", "template_blocks"],
    ["cargo", "test", "--quiet", "--locked", "-p", "pw-render", "--test", "branches"],
    ["cargo", "test", "--quiet", "--locked", "-p", "kiokun-server"],
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

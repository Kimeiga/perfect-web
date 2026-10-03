#!/usr/bin/env python3
"""Mutation controls for ADR-0142: an input bound to a signal.

Each mutant undoes one part:
- `bind:value` lowered to a value and a handler;
- the rule on what a binding binds (PW5304), and the other rules leaving
  what it wrote to it;
- the attributes a signal decides set in place, and a block rendered again
  only for what cannot be.

The tests in `bind.rs` and `signals_render_again.rs` must then fail. The
browser's half is `e2e/bind.spec.mjs`, in the own-renderer suite.

Run from the repository root; `just e14-bind` records the output. The
source is restored after every mutant, whatever happens.
"""

import pathlib
import re
import subprocess
import sys

ROOT = pathlib.Path(__file__).resolve().parent.parent
CORE = ROOT / "compiler/pw-core/src"
LOWER = CORE / "lower.rs"
SIGNALS = CORE / "signals.rs"
PLAN = CORE / "page_values.rs"
VALUES = CORE / "values.rs"
NAMES = CORE / "names.rs"
RESUME = CORE / "resume.rs"
CHECK = CORE / "check.rs"

# (what is undone, file, anchor, replacement)
MUTANTS = [
    (
        "bind:value is not lowered",
        LOWER,
        "        if attr.name != \"bind:value\" {\n",
        "        if attr.name != \"bind:value\" || true {\n",
    ),
    (
        "the handler a binding wrote is not marked",
        LOWER,
        "        b.bound.insert(handler);\n",
        "        let _ = &b.bound;\n",
    ),
    (
        "a binding of a value that is not a signal is accepted",
        SIGNALS,
        "            let Some((_, declared)) = signal else {\n                out.push(bound_value(\n",
        "            let Some((_, declared)) = signal else {\n                continue;\n                #[allow(unreachable_code)]\n                out.push(bound_value(\n",
    ),
    (
        "a signal of another type binds",
        SIGNALS,
        "            if written != \"String\" {\n",
        "            if written != \"String\" && false {\n",
    ),
    (
        "any element binds a value",
        SIGNALS,
        "            if !matches!(tag.as_str(), \"input\" | \"textarea\" | \"select\") {\n",
        "            if !matches!(tag.as_str(), \"input\" | \"textarea\" | \"select\") && false {\n",
    ),
    (
        "PW5302 reads a binding's handler too",
        SIGNALS,
        "                    && !body.bound.contains(&handler)\n",
        "",
    ),
    (
        "a binding's assignment is related as any other",
        VALUES,
        "            .any(|l| matches!(self.body.expr(*l), Expr::Lambda { body, .. } if *body == id))\n",
        "            .any(|l| matches!(self.body.expr(*l), Expr::Lambda { body, .. } if *body == id) && false)\n",
    ),
    (
        "PW0611 reads a binding's assignment too",
        NAMES,
        "            if body.bound.iter().any(|l| body.expr_span(*l) == span)\n                || body.provides",
        "            if false && body.bound.iter().any(|l| body.expr_span(*l) == span)\n                || body.provides",
    ),
    (
        "a binding's handler captures what it sets",
        RESUME,
        "    if body.bound.contains(&lambda) {\n        return Vec::new();\n    }\n",
        "",
    ),
    (
        "the inert-handler rule reads a binding's handler",
        CHECK,
        "                        && !matches!(a.value, AttrValue::Expr(e) if body.bound.contains(&e))\n",
        "",
    ),
    (
        "an attribute a signal decides is not set in place",
        PLAN,
        "    live_attributes(&template.chunks, &signals, false, &mut live);\n",
        "",
    ),
    (
        "a block is rendered again for every signal in it",
        PLAN,
        "            if framed || !set_in_place(p) {\n",
        "            if framed || !set_in_place(p) || true {\n",
    ),
    (
        "a block reads its own signal alone",
        PLAN,
        "            if framed || !set_in_place(p) {\n",
        "            if (framed || !set_in_place(p)) && false {\n",
    ),
    (
        "a URL attribute is set in place",
        PLAN,
        "        Part::Attribute { context, .. } => *context == Context::Attribute,\n",
        "        Part::Attribute { .. } => true,\n",
    ),
]

TESTS = [
    ["cargo", "test", "--quiet", "--locked", "-p", "pw-core", "--test", "bind"],
    ["cargo", "test", "--quiet", "--locked", "-p", "pw-core", "--test", "signals_render_again"],
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

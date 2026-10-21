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
import signal
import subprocess
import sys

import cargo_assertion_results
import mutation_source
import producer_process

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
        # Re-anchored by ADR-0221: a textarea's value is set in place too.
        "        Part::Attribute { context, .. } => {\n"
        "            matches!(context, Context::Attribute | Context::Content)\n"
        "        }\n",
        "        Part::Attribute { .. } => true,\n",
    ),
]

TESTS = [
    ["cargo", "test", "--quiet", "--locked", "-p", "pw-core", "--test", "bind"],
    ["cargo", "test", "--quiet", "--locked", "-p", "pw-core", "--test", "signals_render_again"],
]
INVENTORY = {}


def run_tests():
    """Complete frozen Cargo inventory; only native assertion sites earn kills."""
    passed = failed = 0
    for cmd in TESTS:
        try:
            child = producer_process.run([*cmd, "--", "--format=pretty"], cwd=ROOT, timeout=600)
        except subprocess.TimeoutExpired as expired:
            print(expired.output, flush=True)
            return False, 0, 0
        source = ROOT / "compiler/pw-core/tests" / (cmd[-1] + ".rs")
        key = tuple(cmd)
        result = cargo_assertion_results.validate(child.returncode, child.stdout,
            root=ROOT, sources=[source], expected=INVENTORY.get(key))
        print(child.stdout, flush=True)
        if not result.valid or result.passed + result.failed == 0:
            return False, 0, 0
        INVENTORY.setdefault(key, cargo_assertion_results.names(child.stdout))
        passed += result.passed
        failed += result.failed
    return True, passed, failed


def main():
    import mutation_baseline  # keeps what each command says, for a red baseline
    signal.signal(signal.SIGTERM, lambda *_: sys.exit(1))
    watch = mutation_baseline.mutation_bound.start()
    built, passed, failed = run_tests()
    print(f"baseline: {passed} passed, {failed} failed")
    if not built or failed or not passed or watch.stopped:
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
        with mutation_source.changed(path, original, original.replace(anchor, replacement, 1)):
            built, passed, failed = run_tests()
        if not built or watch.stopped:
            verdict = "REFUSED (invalid native assertion producer; no kill)"
            survivors += 1
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

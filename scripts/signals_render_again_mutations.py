#!/usr/bin/env python3
"""Mutation controls for ADR-0137: what the browser renders again, it can.

Each mutant undoes one part of the plan's walk: that it runs, where a block
a value other than a signal decides begins, what a block the browser renders
may read, the names such a block binds, the parts that are live outside any
block, and what a handler there captures. The tests in
`signals_render_again.rs` must then fail.

Run from the repository root; `just e14-signals-render-again` records the
output. The source is restored after every mutant, whatever happens.
"""

import pathlib
import re
import subprocess
import sys

ROOT = pathlib.Path(__file__).resolve().parent.parent
PLAN = ROOT / "compiler/pw-core/src/page_values.rs"

# (what is undone, file, anchor, replacement)
MUTANTS = [
    (
        "the plan does not ask what the browser renders again",
        PLAN,
        # Re-anchored by ADR-0229, which gives it the subjects the browser
        # computes.
        "    rendered_again(&template.chunks, &signals, &browser, &Reach::Top)?;\n",
        "",
    ),
    (
        "a loop a query decides is not a frame",
        PLAN,
        "                    _ => Reach::Frame,\n"
        "                };\n"
        "                rendered_again(body, signals, browser, &inner)?;\n",
        "                    _ => Reach::Top,\n"
        "                };\n"
        "                rendered_again(body, signals, browser, &inner)?;\n",
    ),
    (
        "a block a signal decides may read anything",
        PLAN,
        "                if let Some(other) = own.iter().find(|v| !signal(v) && !bound.contains(&root(v))) {\n",
        "                if let Some(other) = own.iter().find(|v| !signal(v) && !bound.contains(&root(v)) && false) {\n",
    ),
    (
        "a loop's name is not its block's",
        PLAN,
        "                        Reach::Live([bound.clone(), vec![binding.clone()]].concat())\n",
        "                        Reach::Live(bound.clone())\n",
    ),
    (
        "an arm's names are not its block's",
        PLAN,
        "                        Reach::Live(bound) => Reach::Live([bound.clone(), names].concat()),\n",
        "                        Reach::Live(bound) => Reach::Live(bound.clone()),\n",
    ),
    (
        "a list a signal holds is live outside its block",
        PLAN,
        "                            Part::Text { .. } | Part::Conditional { .. } | Part::Match { .. },\n",
        "                            Part::Text { .. } | Part::Conditional { .. } | Part::Match { .. } | Part::Each { .. },\n",
    ),
    (
        "an attribute a signal decides is live",
        PLAN,
        "                            Part::Text { .. } | Part::Conditional { .. } | Part::Match { .. },\n",
        "                            Part::Text { .. } | Part::Conditional { .. } | Part::Match { .. } | Part::Attribute { .. },\n",
    ),
    (
        "the plan does not know what a handler captures",
        PLAN,
        # Re-anchored by ADR-0232: the lowering is given the signatures.
        "    } = crate::template_ir::lowered(hirs, ws, sigs, captures, unit, id)\n",
        "    } = crate::template_ir::lowered(hirs, ws, sigs, &crate::template_ir::Handlers::new(), unit, id)\n",
    ),
]

TESTS = [
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

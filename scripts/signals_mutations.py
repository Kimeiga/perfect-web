#!/usr/bin/env python3
"""Mutation controls for ADR-0133: a page's own UI state, its first slice.

Each mutant undoes one piece: the three rules (a signal is written only by
a handler, read only where the browser reads it again, and is what a
handler changes), a signal read without a capture, a handler's read and
write of a signal, the plan's live parts and what a block reads, and the
first value's encoding. The tests in `signals.rs` must then fail.

Run from the repository root; `just e14-signals` records the output. The
source is restored after every mutant, whatever happens.
"""

import pathlib
import re
import subprocess
import sys

ROOT = pathlib.Path(__file__).resolve().parent.parent
SIGNALS = ROOT / "compiler/pw-core/src/signals.rs"
CHECK = ROOT / "compiler/pw-core/src/check.rs"
LOWER = ROOT / "compiler/pw-core/src/backend/lower.rs"
PLAN = ROOT / "compiler/pw-core/src/page_values.rs"
JS = ROOT / "compiler/pw-core/src/backend/js_pure.rs"

# (what is undone, file, anchor, replacement)
MUTANTS = [
    (
        "a signal may be written anywhere",
        SIGNALS,
        "                    if !matches!(place, Place::Handler(_)) {\n",
        "                    if false {\n",
    ),
    (
        "a signal may be read anywhere",
        SIGNALS,
        "                if places.get(&id).copied().unwrap_or(Place::Body) == Place::Body {\n",
        "                if false {\n",
    ),
    (
        "a handler may change a binding of its body",
        SIGNALS,
        "                    && !within(body, handler, *declared)\n",
        "                    && false\n",
    ),
    (
        "a signal must be captured",
        CHECK,
        "vec![(n.as_str(), body.expr_span(e), b.is_some() && !signal(b))]",
        "vec![(n.as_str(), body.expr_span(e), b.is_some())]",
    ),
    (
        "a handler cannot read a signal",
        LOWER,
        "                None if self.signals.contains_key(n) => {\n",
        "                None if false => {\n",
    ),
    (
        "a handler cannot change a signal",
        LOWER,
        "            && let Some(ty) = self.signals.get(n).cloned()\n",
        "            && let Some(ty) = None::<Type>\n",
    ),
    (
        "a part a signal decides is a query's",
        PLAN,
        "        if signals.contains(&root) {\n",
        "        if false {\n",
    ),
    (
        "a first value's case is not named",
        JS,
        "                out.insert(\"$case\".into(), J::String(name));\n",
        "                out.insert(\"$case\".into(), J::String(String::new()));\n",
    ),
]

TESTS = [
    ["cargo", "test", "--quiet", "--locked", "-p", "pw-core", "--test", "signals"],
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

#!/usr/bin/env python3
"""Mutation controls for ADR-0204: an element holds only the children HTML
permits, as the page holds them.

Each mutant undoes one piece:
- the rule run at all, and the permitted list read for what a view renders;
- a view's use read by what it renders, and not by its name;
- a block's rows and branches read as the element's children;
- a view read through the blocks at its top, and through the views it uses
  there, each resolved where that view is declared;
- a view met again on the way read once, so one that contains itself ends.

Every mutant must fail `children_as_rendered.rs`.

Run from the repository root; `just e14-rendered-children` records the
output. The source is restored after every mutant, whatever happens.
"""

import pathlib
import re
import subprocess
import sys

ROOT = pathlib.Path(__file__).resolve().parent.parent
CHECK = ROOT / "compiler/pw-core/src/check.rs"

# (what, file, anchor, replacement)
MUTANTS = [
    (
        "the rule is not run",
        CHECK,
        "        per_unit.extend(nesting(&workspace, &hirs, i, &u.hir));\n",
        "",
    ),
    (
        "a view's use is read by its name",
        CHECK,
        "                let (rendered, message) = if is_view_tag(child) {\n",
        "                let (rendered, message) = if false && is_view_tag(child) {\n",
    ),
    (
        "what a view renders is never refused",
        CHECK,
        "                    let Some(top) = tops.into_iter().find(|t| !allowed.contains(&t.as_str()))\n",
        "                    let Some(top) = tops.into_iter().find(|_| false)\n",
    ),
    (
        "a block's rows are not the element's children",
        CHECK,
        "            for &c in children {\n                held_at(body, c, out);\n",
        "            for &c in children.iter().take(0) {\n                held_at(body, c, out);\n",
    ),
    (
        "a view is read without the blocks at its top",
        CHECK,
        "        held_at(body, r, &mut held);\n",
        "        held.push(r);\n",
    ),
    (
        "a view is read without the views it uses",
        CHECK,
        "            view_tops(workspace, hirs, def.unit, tag, seen, out);\n",
        "",
    ),
    (
        "a view's views are resolved where it is used",
        CHECK,
        "            view_tops(workspace, hirs, def.unit, tag, seen, out);\n",
        "            view_tops(workspace, hirs, unit, tag, seen, out);\n",
    ),
    (
        "a view met again is read again",
        CHECK,
        "    if decl.kind != DeclKind::View || !seen.insert(def) {\n",
        "    if decl.kind != DeclKind::View || (!seen.insert(def) && false) {\n",
    ),
]

TESTS = ["cargo", "test", "--quiet", "--locked", "-p", "pw-core", "--test", "children_as_rendered"]


def run():
    """(built, passed, failed). A test binary that aborts, as one
    overflowing its stack does, is a failure."""
    r = subprocess.run(TESTS, cwd=ROOT, capture_output=True, text=True)
    out = r.stdout + r.stderr
    m = re.search(r"test result: \w+\. (\d+) passed; (\d+) failed", out)
    if m is None:
        if "error[" in out or "could not compile" in out:
            return False, 0, 0
        return True, 0, 1
    return True, int(m.group(1)), int(m.group(2))


def main():
    import mutation_baseline  # keeps what each command says, for a red baseline
    built, passed, failed = run()
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
            built, passed, failed = run()
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

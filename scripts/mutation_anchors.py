#!/usr/bin/env python3
"""Every mutation control's anchor matches its file exactly once.

A mutation script (`scripts/*_mutations.py`) undoes one piece of the
compiler by replacing an exact anchor. When later work reformats or moves
that code, the anchor stops matching, and the script says so only when it
runs. Nothing ran a script after its own evidence was recorded, so on
2026-09-25 five anchors in four scripts had drifted unnoticed. This check
reads the anchors and builds nothing, so `just ci` runs it on every change.

A mutant is `(..., path, anchor, replacement)`. The check fails on an anchor
that does not match exactly once, on a replacement equal to its anchor, and
on a mutant of any other shape.
"""

import importlib.util
import pathlib
import sys

sys.dont_write_bytecode = True

ROOT = pathlib.Path(__file__).resolve().parent.parent


def problems_in(script):
    """(mutants read, problems found) for one script."""
    spec = importlib.util.spec_from_file_location(script.stem, script)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    mutants = getattr(module, "MUTANTS", None)
    if not mutants:
        return 0, [f"{script.name}: no MUTANTS"]
    problems = []
    for mutant in mutants:
        shaped = (
            isinstance(mutant, tuple)
            and len(mutant) >= 4
            and isinstance(mutant[-3], pathlib.Path)
            and isinstance(mutant[-2], str)
            and isinstance(mutant[-1], str)
        )
        if not shaped:
            problems.append(f"{script.name}: a mutant of another shape: {mutant!r:.80}")
            continue
        what = " ".join(str(m) for m in mutant[:-3])
        path, anchor, replacement = mutant[-3:]
        if anchor == replacement:
            problems.append(f"{script.name}: {what}: the replacement is its anchor")
            continue
        found = path.read_text().count(anchor)
        if found != 1:
            problems.append(
                f"{script.name}: {what}: {found} matches in {path.relative_to(ROOT)}"
            )
    return len(mutants), problems


def main():
    scripts = sorted((ROOT / "scripts").glob("*_mutations.py"))
    total, problems = 0, []
    for script in scripts:
        read, found = problems_in(script)
        total += read
        problems.extend(found)
    for p in problems:
        print(p)
    if problems:
        print(f"FAIL: {len(problems)} of {total} mutants cannot run as written")
        return 1
    print(f"mutation anchors: {total} mutants in {len(scripts)} scripts, each matching once")
    return 0


if __name__ == "__main__":
    sys.exit(main())

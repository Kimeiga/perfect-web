#!/usr/bin/env python3
"""**Every recipe's evidence is in the repository** (2026-10-09).

A recipe writes its evidence to `docs/evidence/<milestone>/<name>.txt`, and
STATUS cites the recipe. On 2026-10-09 twenty-five recipes cited by their
ADRs had no file on `master`: their runs were cited and their evidence never
fetched (ADR-0281's merge flow fetches it now). This lists each recipe, in
`justfile` and `just/*.just`, whose evidence file is missing.

A branch's new recipe is recorded at its merge, so `master` alone is held to
it: `scripts/tests/test_evidence_present.py` runs on `master`'s CI, and here
on demand: `python3 scripts/evidence_present.py`.
"""

import pathlib
import re
import sys

ROOT = pathlib.Path(__file__).resolve().parent.parent

# A recipe's header, at the start of a line, its parameters and their
# defaults (`run *flags`, `arg=""`), and not an assignment (`x := 1`); and its
# body, each line of which is indented.
RECIPE = re.compile(
    r"^([a-z0-9][a-z0-9-]*)(?:[ \t]+[^\n:]*)?:(?!=)[^\n]*\n((?:[ \t]+[^\n]*\n|\n)*)", re.M
)
# Where a recipe's body writes its evidence.
WRITES = re.compile(r">\s*(docs/evidence/[^\s;\"']+\.txt)")


def recipes(root=ROOT):
    """Each (file, recipe, evidence path) the recipes write."""
    files = [root / "justfile"] + sorted((root / "just").glob("*.just"))
    for f in files:
        if not f.exists():
            continue
        for m in RECIPE.finditer(f.read_text()):
            for path in sorted(set(WRITES.findall(m.group(2)))):
                yield f.name, m.group(1), path


def recorded(root, path):
    """Is it in the tree? A path a recipe's parameter names
    (`harness-{{TASK}}.txt`) is, where any file of its shape is."""
    if "{{" in path:
        shape = re.sub(r"\{\{[^}]*\}\}", "*", path)
        return any(root.glob(shape))
    return (root / path).exists()


def missing(root=ROOT):
    """Each recipe whose evidence file is not in the tree."""
    return [(f, name, path) for f, name, path in recipes(root) if not recorded(root, path)]


def main():
    found = list(recipes())
    gone = missing()
    print(f"evidence-present: {len(found)} evidence files written by recipes; {len(gone)} missing")
    for f, name, path in gone:
        print(f"  {f}: {name} -> {path}")
    return 1 if gone else 0


if __name__ == "__main__":
    sys.exit(main())

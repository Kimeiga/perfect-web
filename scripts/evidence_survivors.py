#!/usr/bin/env python3
"""**No committed evidence records a mutant that survived** (the integrator's
finding at train-6's landing, 2026-10-14).

A mutation script exits non-zero where a mutant survives, so its recipe
fails and `evidence_fetch.py` copies none of what it wrote. Evidence that
records a survivor was copied some other way, or recorded before its
script's fix and never recorded again: `docs/evidence/E10/names.txt` said
"19 of 20 mutants killed" from 2026-10-07 to train-6's landing, a week in
which no push planned its recipe and the nightly, which ran it green, keeps
no evidence.

Prints each such line, `path:line: text`, and exits 1 where there is one.

    python3 scripts/evidence_survivors.py
"""

import pathlib
import re
import sys

ROOT = pathlib.Path(__file__).resolve().parent.parent

# What a mutation script says of a mutant it could not kill: its verdict, an
# anchor that no longer matches, and its count.
VERDICT = re.compile(r": (SURVIVED|ANCHOR NOT FOUND EXACTLY ONCE in .+)$")
COUNT = re.compile(r"^(\d+) of (\d+) mutants killed")


def survivors(root: pathlib.Path = ROOT) -> list[tuple[str, int, str]]:
    """Each evidence line that records a mutant not killed, as (path from
    the root, line number, the line)."""
    out = []
    for path in sorted((root / "docs/evidence").rglob("*.txt")):
        rel = str(path.relative_to(root))
        for n, line in enumerate(path.read_text(errors="replace").splitlines(), 1):
            count = COUNT.match(line)
            if VERDICT.search(line) or (count and count.group(1) != count.group(2)):
                out.append((rel, n, line))
    return out


def main() -> int:
    found = survivors()
    for rel, n, line in found:
        print(f"{rel}:{n}: {line}")
    return 1 if found else 0


if __name__ == "__main__":
    sys.exit(main())

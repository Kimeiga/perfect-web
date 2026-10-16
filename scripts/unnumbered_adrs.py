#!/usr/bin/env python3
"""**`master` holds no unnumbered ADR** (track evidence-integrity, finding 6,
2026-10-15).

A track writes its ADR unnumbered, `docs/DECISIONS/ADR-XXXX-<slug>.md`, and
names it so in its code, its recipes and its evidence; the integrator
numbers it at the merge and rewrites each mention (docs/PARALLEL.md). A
mention the rewrite missed survives the merge, naming a decision no reader
can find: `compiler/pw-core/tests/statements_separated.rs` held one from
2026-10-09 (train-9 numbers it).

Each tracked file is read for the placeholder outside an inline code span:
a mention in backticks, as the documents that describe the numbering write
it, is about the placeholder, and one outside them names a decision. And a
file under `docs/DECISIONS/` whose name is the placeholder's is an ADR not
numbered. A track's branch holds both until its merge, so the test that
holds the repository to it runs on `master`'s CI, and a train's (`held`).

Prints each, `path:line: text`, and exits 1 where there is one.

    python3 scripts/unnumbered_adrs.py
"""

import os
import pathlib
import re
import subprocess
import sys

ROOT = pathlib.Path(__file__).resolve().parent.parent

# The placeholder, spelled in two parts so that this file names no decision.
PLACEHOLDER = "ADR-" + "XXXX"
MENTION = re.compile(re.escape(PLACEHOLDER))
# An inline code span: what is between two backticks on one line.
CODE_SPAN = re.compile(r"`[^`\n]*`")


def held(ref: str | None) -> bool:
    """Whether the run of `ref` (`GITHUB_REF`) is held to no unnumbered ADR:
    `master`'s, and a train's, whose ADRs the integrator numbered before it
    was pushed (ADR-0281). A track's branch keeps the placeholder until its
    merge."""
    return ref == "refs/heads/master" or (ref or "").startswith("refs/heads/track/train-")


def tracked(root: pathlib.Path = ROOT) -> list[str]:
    """Every file git tracks in `root`, by its path from it."""
    out = subprocess.run(
        ["git", "ls-files", "-z"], cwd=root, capture_output=True, check=True
    ).stdout
    return sorted(p for p in out.decode("utf-8", errors="replace").split("\0") if p)


def unnumbered(root: pathlib.Path = ROOT, files: list[str] | None = None) -> list[tuple[str, int, str]]:
    """Each unnumbered ADR in `files` (every tracked file): a mention outside
    an inline code span, as (path, line, the line), and an ADR file of the
    placeholder's name, as (path, 0, its name)."""
    found = []
    for rel in tracked(root) if files is None else files:
        if rel.startswith("docs/DECISIONS/") and pathlib.PurePosixPath(rel).name.startswith(PLACEHOLDER):
            found.append((rel, 0, pathlib.PurePosixPath(rel).name))
        path = root / rel
        try:
            data = path.read_bytes()
        except (IsADirectoryError, FileNotFoundError):
            continue
        if b"\0" in data or PLACEHOLDER.encode() not in data:
            continue
        for n, line in enumerate(data.decode("utf-8", errors="replace").splitlines(), 1):
            if MENTION.search(CODE_SPAN.sub("", line)):
                found.append((rel, n, line.strip()))
    return found


def main() -> int:
    found = unnumbered()
    for rel, n, text in found:
        print(f"{rel}:{n}: {text}")
    print(f"unnumbered ADRs: {len(found)} (GITHUB_REF={os.environ.get('GITHUB_REF', '')!r} is held: "
          f"{held(os.environ.get('GITHUB_REF'))})")
    return 1 if found else 0


if __name__ == "__main__":
    sys.exit(main())

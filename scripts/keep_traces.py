#!/usr/bin/env python3
"""**A failing browser run's output, kept where the next run does not clear
it** (track ci-diagnostics, found 2026-10-19).

Playwright clears its output directory, `test-results/`, at the start of every
run (`createRemoveOutputDirsTask`, which only watch mode skips). A recipe that
runs a suite three times (`e10-browser`), and every Playwright run after it in
a CI shard, a mutation control's among them, left the last run's output
there, and the shard uploaded that once at the end: a failure followed by a
passing run lost its trace. Train-10's intermittent Chromium failure did, and
its shard's `traces-23` held only a mutation control's intended failures.

`scripts/run_suite.sh` runs this after each run that failed, and
`mutation_baseline.explain()` after a baseline that came out red:

    python3 scripts/keep_traces.py SOURCE --root ROOT --label LABEL --tag TAG --run RUN

copies each entry of SOURCE (one directory per failed test, with its
`trace.zip`, and the run's `.last-run.json`) into ROOT/LABEL/TAG/RUN, where
nothing clears it, and says what it kept. LABEL is the recipe's name
(`PW_RECIPE`, which `ci_recipes.py` sets, or `local`), TAG the call's UTC
time, RUN the run's (`run-2`). The upload steps of `verify.yml` name every
`kept-traces/`.

**It is bounded.** ROOT holds at most `PW_KEPT_TRACES_KIB` (default 32 MiB:
ten times train-10's whole `traces-23`, about 3 MB) in all, whatever shard it is in: an
entry that does not fit is not copied and DEST says so (`NOT-KEPT.txt`, with
each entry's size), so that a run in which hundreds of tests failed keeps its
first and names the rest, and a shard cannot fill the artifact store.

**A mutation control's runs are not kept.** A mutant's suite fails by design,
once per mutant, hundreds in a shard, and its trace says nothing the kill
does not; they would swamp the artifact. The one run of a mutation script
that is kept is its baseline, when it came out red: that failure is real and
stops the script. Only `explain()` keeps one.

It never changes a run's status: a caller that cannot keep says so and goes
on.
"""

from __future__ import annotations

import argparse
import os
import pathlib
import re
import shutil
import sys
import typing

# What a root holds at most, in KiB, unless `PW_KEPT_TRACES_KIB` says another.
BUDGET_KIB = 32 * 1024

NOT_KEPT = "NOT-KEPT.txt"


def budget_kib() -> int:
    """The budget in force: `PW_KEPT_TRACES_KIB` where it is a count, else
    the default."""
    said = os.environ.get("PW_KEPT_TRACES_KIB", "")
    return int(said) if said.isdigit() else BUDGET_KIB


def label_of(text: str | None) -> str:
    """A recipe's name as a directory's, whatever it was given: its letters,
    digits, `.`, `_` and `-`, and not a path (`..`), or `local` where it has
    none."""
    cleaned = re.sub(r"[^A-Za-z0-9._-]", "_", text or "").strip(".")
    return cleaned or "local"


def tree_bytes(path: pathlib.Path) -> int:
    """What a file, or a directory with everything in it, takes: its size,
    its links not followed."""
    if not os.path.lexists(path):
        return 0
    if path.is_symlink() or path.is_file():
        return path.lstat().st_size
    total = 0
    for here, _dirs, files in os.walk(path):
        total += sum(os.lstat(os.path.join(here, f)).st_size for f in files)
    return total


def size(nbytes: int) -> str:
    """A size as a reader takes it: `2.9 MiB`."""
    if nbytes < 1024:
        return f"{nbytes} B"
    if nbytes < 1024 * 1024:
        return f"{nbytes / 1024:.1f} KiB"
    return f"{nbytes / 1024 / 1024:.1f} MiB"


def destination(root: pathlib.Path, label: str | None, tag: str, run: str) -> pathlib.Path:
    """Where a run's output is kept: ROOT/LABEL/TAG/RUN, each cleaned
    (`label_of`), so that no name leaves ROOT."""
    return root / label_of(label) / label_of(tag) / label_of(run)


class Kept(typing.NamedTuple):
    """What a call kept of a run's output: where, which entries (by name and
    size in bytes), which were left, and the budget in bytes."""

    dest: pathlib.Path
    kept: list
    skipped: list
    budget: int

    def says(self) -> str:
        """One line: where the run's output is, how much, and what was left."""
        if not self.kept and not self.skipped:
            return "there was no output to keep"
        said = f"kept {len(self.kept)} of {len(self.kept) + len(self.skipped)} entries of its output "
        said += f"({size(sum(n for _, n in self.kept))}) in {self.dest}"
        if self.skipped:
            said += (
                f"; {len(self.skipped)} ({size(sum(n for _, n in self.skipped))}) "
                f"were over the {size(self.budget)} a kept-traces holds (see {NOT_KEPT})"
            )
        return said


def keep(source: pathlib.Path, dest: pathlib.Path, root: pathlib.Path, budget: int | None = None) -> Kept:
    """Copy each entry of `source` into `dest` while `root` holds no more than
    `budget` bytes (the budget in force where none is given), the entries in
    name order, and list in `dest` the ones that did not fit. Nothing is
    created where `source` is missing or empty."""
    budget = budget if budget is not None else budget_kib() * 1024
    entries = sorted(source.iterdir()) if source.is_dir() else []
    if not entries:
        return Kept(dest, [], [], budget)
    used = tree_bytes(root)
    kept: list[tuple[str, int]] = []
    skipped: list[tuple[str, int]] = []
    dest.mkdir(parents=True, exist_ok=True)
    for entry in entries:
        need = tree_bytes(entry)
        if used + need > budget:
            skipped.append((entry.name, need))
            continue
        target = dest / entry.name
        if entry.is_dir() and not entry.is_symlink():
            shutil.copytree(entry, target, symlinks=True, dirs_exist_ok=True)
        else:
            shutil.copy2(entry, target, follow_symlinks=False)
        used += need
        kept.append((entry.name, need))
    if skipped:
        listing = "".join(f"{name}\t{n} bytes\n" for name, n in skipped)
        (dest / NOT_KEPT).write_text(
            f"Not kept: {len(skipped)} entries of the run's output, which would have taken {root} past "
            f"its budget of {budget} bytes (PW_KEPT_TRACES_KIB).\n{listing}"
        )
    return Kept(dest, kept, skipped, budget)


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description="Keep a failing browser run's output.")
    parser.add_argument("source", type=pathlib.Path, help="the run's output directory (test-results)")
    parser.add_argument("--root", type=pathlib.Path, required=True, help="the directory whose size is bounded")
    parser.add_argument("--label", default=os.environ.get("PW_RECIPE"), help="the recipe (default $PW_RECIPE, else local)")
    parser.add_argument("--tag", required=True, help="the call's, to tell one call's runs from another's")
    parser.add_argument("--run", required=True, help="the run's, as `run-2`")
    parser.add_argument("--budget-kib", type=int, default=None, help="the most --root may hold, in KiB")
    args = parser.parse_args(argv)
    budget = args.budget_kib * 1024 if args.budget_kib is not None else None
    try:
        dest = destination(args.root, args.label, args.tag, args.run)
        kept = keep(args.source, dest, args.root, budget)
    except OSError as error:
        print(f"could not keep the run's output: {error}")
        return 1
    print(kept.says())
    return 0


if __name__ == "__main__":
    sys.exit(main())

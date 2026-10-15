#!/usr/bin/env python3
"""**An evidence file's mutants are its script's** (track evidence-integrity,
finding 2, 2026-10-15).

`docs/evidence/E10/privacy-by-resolution.txt`, recorded at 334bf88 on
2026-09-26, said "4 of 4 mutants killed", one of them "the body's label is
read by spelling", which e0752896 (ADR-0256, 2026-10-07) retired from
`scripts/privacy_resolution_mutations.py`. Nothing compared an evidence
file with the script that wrote it: the record claimed a mutant its script
no longer plants, and a count its script no longer makes.

For each recipe that runs a mutation script, each `k of n mutants killed`
line of each evidence file the recipe writes is read with the verdict lines
above it, back to the count before it:
- each verdict names a mutant of one of the recipe's scripts, each mutant
  once: by its description, the longest of the words before its path,
  beside nothing but its other words, a suite's tag in brackets, and
  separators (`[core]`, `next-react: `, `V1  `);
- `n` is as many mutants as that script defines and as many verdicts as the
  block holds, and `k` as many as say KILLED;
- and each script the recipe runs is counted in one block.

A recipe whose evidence disagrees is **stale and planned** where a
verification run records it again: it runs on CI (not `LOCAL_ONLY`), every
file it writes names the commit it was recorded at (or is missing), and the
planner holds that evidence not current in the tree here, so a train's
push plans it (`ci_plan.py --stale`). Evidence of record comes only from a
run (ADR-0281), so such a recipe is named and fails nothing. Any other
disagreement fails: no plan would record it again.

Prints each disagreement, `path:line: what`, then each recipe's state, and
exits 1 where one fails.

    python3 scripts/evidence_counts.py
"""

import importlib.util
import pathlib
import re
import sys

ROOT = pathlib.Path(__file__).resolve().parent.parent

COUNT = re.compile(r"^(\d+) of (\d+) mutants killed")
# What a script says of each mutant (`mutation_anchors.py`'s shapes, and the
# verdicts the scripts print).
VERDICT = re.compile(
    r"^(.*): (KILLED \(.*\)|SURVIVED|DID NOT BUILD.*|ANCHOR NOT FOUND EXACTLY ONCE in .+)$"
)
# A suite's tag beside a mutant's description: `[core]`, `[server]`.
TAG = re.compile(r"\[[\w./ -]*\]")
SCRIPT = re.compile(r"scripts/([a-z0-9_]+_mutations\.py)")


def load(path: pathlib.Path):
    spec = importlib.util.spec_from_file_location(path.stem, path)
    module = importlib.util.module_from_spec(spec)
    assert spec.loader is not None
    spec.loader.exec_module(module)
    return module


def planner(root: pathlib.Path = ROOT):
    return load(root / "scripts/ci_plan.py")


def words(mutant: tuple) -> tuple[str, list[str]]:
    """A mutant's description, the longest of its words before its path,
    and its other words."""
    said = [w for w in mutant[:-3] if isinstance(w, str)]
    if not said:
        return "", []
    i = max(range(len(said)), key=lambda k: len(said[k]))
    return said[i], said[:i] + said[i + 1 :]


def names(said: str, mutant: tuple) -> bool:
    """Whether a verdict's `said` names `mutant`: its description, beside
    nothing but its other words, a suite's tag and separators."""
    description, others = words(mutant)
    if not description or description not in said:
        return False
    rest = said.replace(description, " ", 1)
    for w in others:
        rest = rest.replace(w, " ", 1)
    return TAG.sub(" ", rest).strip(" :") == ""


def blocks(text: str) -> list[tuple[int, int, int, list[tuple[int, str, str]]]]:
    """Each count in `text`: (its line, k, n, the verdicts above it back to
    the count before, each (line, what it names, its verdict))."""
    out, verdicts = [], []
    for at, line in enumerate(text.splitlines(), 1):
        v = VERDICT.match(line)
        if v:
            verdicts.append((at, v.group(1), v.group(2)))
            continue
        c = COUNT.match(line)
        if c:
            out.append((at, int(c.group(1)), int(c.group(2)), verdicts))
            verdicts = []
    return out


def disagree(mutants: list, k: int, n: int, verdicts: list) -> list[tuple[int | None, str]]:
    """What disagrees between one count, the verdicts above it and the
    script's `mutants`: (a line, or None for the count's, and what)."""
    out: list[tuple[int | None, str]] = []
    if n != len(mutants):
        out.append((None, f"counts {n} mutants; its script defines {len(mutants)}"))
    if len(verdicts) != n:
        out.append((None, f"counts {n} mutants above {len(verdicts)} verdicts"))
    killed = sum(1 for _, _, verdict in verdicts if verdict.startswith("KILLED"))
    if k != killed:
        out.append((None, f"counts {k} killed; {killed} verdicts say KILLED"))
    taken: set[int] = set()
    for at, said, _ in verdicts:
        found = next(
            (i for i, m in enumerate(mutants) if i not in taken and names(said, m)), None
        )
        if found is None:
            again = any(names(said, m) for m in mutants)
            out.append((at, f"{said!r} names {'a mutant named before' if again else 'no mutant of its script'}"))
        else:
            taken.add(found)
    return out


def owner(scripts: dict[str, list], verdicts: list, n: int) -> str | None:
    """The script of `scripts` whose mutants most of a block's verdicts name
    (of two that tie, one with as many mutants as counted), or none where
    they name none."""

    def score(s: str) -> tuple[int, bool]:
        named = sum(1 for _, said, _ in verdicts if any(names(said, m) for m in scripts[s]))
        return named, len(scripts[s]) == n

    best = max(scripts, key=score, default=None)
    return best if best is not None and score(best)[0] > 0 else None


def disagreements(
    root: pathlib.Path = ROOT, plan=None
) -> dict[str, list[tuple[str, int, str]]]:
    """Each recipe whose evidence disagrees with its scripts, with each
    disagreement: (a file, its line, what)."""
    plan = plan or planner(root)
    found: dict[str, list[tuple[str, int, str]]] = {}
    loaded: dict[str, list] = {}
    for recipe, body in sorted(plan.bodies(root).items()):
        run = sorted(set(SCRIPT.findall(body)))
        files = plan.evidence_files(body)
        if not run or not files:
            continue
        scripts = {}
        for s in run:
            if s not in loaded:
                loaded[s] = list(getattr(load(root / "scripts" / s), "MUTANTS", []))
            scripts[s] = loaded[s]
        said: list[tuple[str, int, str]] = []
        counted: set[str] = set()
        for rel in files:
            path = root / rel
            if not path.is_file():
                said.append((rel, 0, "no such file: the recipe's evidence is not recorded"))
                continue
            for at, k, n, verdicts in blocks(path.read_text(errors="replace")):
                s = owner(scripts, verdicts, n)
                if s is None:
                    said.append((rel, at, f"its verdicts name the mutants of none of {', '.join(run)}"))
                    continue
                if s in counted:
                    said.append((rel, at, f"{s}'s mutants are counted twice"))
                    continue
                counted.add(s)
                for line, what in disagree(scripts[s], k, n, verdicts):
                    said.append((rel, line or at, f"{s}: {what}"))
        if any(path.is_file() for path in (root / f for f in files)):
            for s in run:
                if s not in counted:
                    said.append((files[0], 0, f"counts none of {s}'s mutants, which the recipe runs"))
        if said:
            found[recipe] = said
    return found


def planned_again(recipes: list[str], root: pathlib.Path = ROOT, plan=None) -> set[str]:
    """The recipes of `recipes` a train's push plans again, as the planner
    judges the tree here once it is committed: run on CI, and stale
    (`ci_plan.stale`, read from the tree here, not the head)."""
    plan = plan or planner(root)
    names = [r for r in recipes if r not in plan.LOCAL_ONLY]
    if not names:
        return set()

    def show(head: str, path: str) -> str | None:
        try:
            return (root / path).read_text(errors="replace")
        except OSError:
            return None

    def read(head: str, path: str) -> str | None:
        m = plan.RECORDED_AT.search(show(head, path) or "")
        return m.group(1) if m else None

    return set(
        plan.stale(
            names,
            "HEAD",
            body=plan.bodies(root),
            show=show,
            read=read,
            descends_from=lambda sha, head: plan.descends(sha, "HEAD", root),
            reach=lambda sha, head: set(plan.recipes_for(plan.hunks(sha, None), plan.NEAR, names)),
        )
    )


def failing(found: dict, again: set[str]) -> list[str]:
    """The recipes of `found` whose disagreement no plan records again."""
    return sorted(r for r in found if r not in again)


def main() -> int:
    found = disagreements()
    again = planned_again(sorted(found))
    fails = failing(found, again)
    for recipe, said in sorted(found.items()):
        for rel, at, what in said:
            print(f"{rel}:{at}: {what}")
        if recipe in fails:
            print(f"  {recipe}: FAIL: no plan records it again")
        else:
            print(f"  {recipe}: stale, and planned at a train's push (ci_plan.py --stale)")
    print(
        f"evidence counts: {len(found)} recipes disagree with their scripts, "
        f"{len(found) - len(fails)} of them stale and planned, {len(fails)} failing"
    )
    return 1 if fails else 0


if __name__ == "__main__":
    sys.exit(main())

#!/usr/bin/env python3
"""Plan a verification run (ADR-0245): evidence recipes, in shards.

The recipes are `just`'s own list, kept where they record evidence
(`e<N>-…`) and are not measurements of the machine that records them
(`LOCAL_ONLY`). Which of them:
- every one (`--all`), as the nightly run plans;
- those a range of commits touches (`--changed BASE HEAD`), as a push plans,
  the chain's choice: each recipe running a mutation script that the range
  changed, whose mutant sits within `--near` lines of a changed line, or
  whose tests the range changed, a Rust test or a browser spec (ADR-0281);
  each recipe that runs a browser spec the range changed; and each recipe
  whose own lines changed;
- or those named.

Each is dealt to the shard with the least work so far, the costliest first,
by an estimate of its cost: the mutants the scripts it runs plant, each a
build and a test run. One that needs a database (`NEEDS_DATABASE`) is a
shard of its own, which the run gives a PostgreSQL of its own (ADR-0278),
while the shards go round; the rest are dealt into the shards left. A
recipe that drives a browser, or reads what the build makes (the servers,
`pw`), shares its shards with its kind (ADR-0249): only those shards install
the browsers and build, most of a shard's setup, and the others do neither.
Prints, for `$GITHUB_OUTPUT`, `shards=<json>`, a list of `{"index": i,
"recipes": [...], "browsers": b, "build": b, "database": b}`, empty where
nothing is to run.

    python3 scripts/ci_plan.py --shards 17 --all
    python3 scripts/ci_plan.py --shards 17 --changed BASE HEAD
    python3 scripts/ci_plan.py --shards 17 e14-feed e14-read-whole
"""

import argparse
import importlib.util
import json
import pathlib
import re
import subprocess
import sys

ROOT = pathlib.Path(__file__).resolve().parent.parent

# Measurements of the machine they run on: a time, a load, a memory curve,
# each naming its machine in a `host:` line. A shared runner's numbers are
# not comparable from one run to the next, so these are recorded on the
# development machine and never fetched from a run.
LOCAL_ONLY = {
    "e7-performance",
    "e9-latency",
    "e10-bench",
    "e10-close-bench",
    "e10-load",
    "e10-memory",
    # The kiokun track's (docs/PARALLEL.md, W6's plan; ADR-0281's fifth
    # decision): it reads the owner's kiokun-data checkout, which no runner
    # has. What needs the owner's local data is recorded here; what a
    # committed sample shows runs on CI.
    "e14-kiokun-inventory",
    # The kiokun track's sample of the whole dictionary: the owner's data, and
    # this machine's times.
    "e14-kiokun-sample",
    # And its page head against kiokun.com's own code, copied from the
    # owner's checkout.
    "e14-kiokun-seo",
}

# Recipes run against a database (ADR-0246). Each is a shard of its own,
# beside a PostgreSQL of its own (ADR-0278), set up as any shard is for its
# recipe (ADR-0258): `e14-identity` and `e14-uploads` (ADR-0260) drive
# browsers.
NEEDS_DATABASE = {
    "e14-feed-postgres",
    "e14-identity",
    "e14-uploads",
    "e14-notifications",
    "e14-messages",
}

EVIDENCE_RECIPE = re.compile(r"^e[0-9]+-[a-z0-9-]+$")


def recipes() -> list[str]:
    """Every recipe `just` knows, by name."""
    out = subprocess.run(
        ["just", "--summary"], cwd=ROOT, capture_output=True, text=True, check=True
    ).stdout
    return out.split()


IMPORT = re.compile(r"""^import\??\s+(['"])(.+?)\1""")

HEADER = re.compile(r"^([a-z0-9][a-z0-9-]*)\s*(?:[^:=]*)?:(?!=)")


def justfile_parts(root: pathlib.Path = ROOT) -> list[str]:
    """The justfile and each file it imports (ADR-0253), each by its path
    from the repository: a parallel track's recipes live in a file of its
    own, which `just` reads as part of the justfile."""
    parts = ["justfile"]
    for line in (root / "justfile").read_text().splitlines():
        m = IMPORT.match(line)
        if m and (root / m.group(2)).is_file():
            parts.append(m.group(2))
    return parts


def bodies(root: pathlib.Path = ROOT) -> dict[str, str]:
    """Each recipe's body, by name, in every part of the justfile."""
    out: dict[str, str] = {}
    for part in justfile_parts(root):
        name = None
        for line in (root / part).read_text().splitlines():
            m = HEADER.match(line)
            if m and not line.startswith((" ", "\t")):
                name = m.group(1)
                out[name] = ""
                continue
            if name and (line.startswith((" ", "\t")) or not line.strip()):
                out[name] += line + "\n"
            else:
                name = None
    return out


def recipes_at(text: list[str], lines: list[tuple[int, int]], names: list[str]) -> set[str]:
    """The recipes of `names` whose own lines are among `lines`, in a part of
    the justfile: for each, the nearest header above it."""
    out = set()
    for a, b in lines:
        for line in range(a, b + 1):
            for k in range(min(line, len(text)) - 1, -1, -1):
                m = HEADER.match(text[k])
                if m and not text[k].startswith((" ", "\t")):
                    if m.group(1) in names:
                        out.add(m.group(1))
                    break
                if text[k].strip() and not text[k].startswith((" ", "\t", "#")):
                    break
    return out


def mutants(script: str) -> int:
    """How many mutants `scripts/<script>` plants, or 0 where it cannot be read."""
    path = ROOT / "scripts" / script
    try:
        spec = importlib.util.spec_from_file_location(path.stem, path)
        module = importlib.util.module_from_spec(spec)
        assert spec.loader is not None
        spec.loader.exec_module(module)
        return len(getattr(module, "MUTANTS", []))
    except Exception:
        return 0


def cost(body: str) -> int:
    """A recipe's work: one for itself, and one for each mutant it plants."""
    scripts = set(re.findall(r"scripts/([a-z0-9_]+_mutations\.py)", body))
    return 1 + sum(mutants(s) for s in scripts)


def hunks(base: str, head: str) -> dict[str, list[tuple[int, int]]]:
    """Each file the range changed, with its changed lines' ranges in `head`
    (a deletion is the line it stood before)."""
    diff = subprocess.run(
        ["git", "diff", "-U0", base, head], cwd=ROOT, capture_output=True, text=True, check=True
    ).stdout
    out: dict[str, list[tuple[int, int]]] = {}
    current = None
    for line in diff.splitlines():
        if line.startswith("+++ "):
            current = None if line == "+++ /dev/null" else line[len("+++ b/"):]
            if current:
                out.setdefault(current, [])
            continue
        m = re.match(r"@@ -\d+(?:,\d+)? \+(\d+)(?:,(\d+))? @@", line)
        if m and current:
            start, count = int(m.group(1)), int(m.group(2) or 1)
            out[current].append((start, start + max(count, 1) - 1))
    return out


def changed_specs(changed: dict[str, list[tuple[int, int]]]) -> set[str]:
    """The browser specs a change reaches, by file name: `pages.spec.mjs`."""
    return {pathlib.Path(p).name for p in changed if p.endswith(".spec.mjs")}


def touched_scripts(changed: dict[str, list[tuple[int, int]]], near: int) -> set[str]:
    """The mutation scripts a change reaches: the script itself, a mutant
    within `near` lines of a changed line, or a test file it runs, a Rust
    test or a browser spec (ADR-0281)."""
    tests = {pathlib.Path(p).stem for p in changed if "/tests/" in p and p.endswith(".rs")}
    specs = changed_specs(changed)
    out = set()
    for path in sorted((ROOT / "scripts").glob("*_mutations.py")):
        rel = str(path.relative_to(ROOT))
        if rel in changed:
            out.add(path.name)
            continue
        text = path.read_text()
        if any(f'"--test", "{t}"' in text or f"--test {t}" in text for t in tests):
            out.add(path.name)
            continue
        # A browser spec it runs: the mutants it plants are killed there.
        if any(s in text for s in specs):
            out.add(path.name)
            continue
        try:
            spec = importlib.util.spec_from_file_location(path.stem, path)
            module = importlib.util.module_from_spec(spec)
            assert spec.loader is not None
            spec.loader.exec_module(module)
        except Exception:
            continue
        for mutant in getattr(module, "MUTANTS", []):
            target = next((x for x in mutant if isinstance(x, pathlib.Path)), None)
            if target is None or len(mutant) < 2 or not isinstance(mutant[-2], str):
                continue
            rel_target = str(target.resolve().relative_to(ROOT)) if target.is_absolute() else str(target)
            if rel_target not in changed:
                continue
            source = (ROOT / rel_target).read_text()
            at = source.find(mutant[-2])
            if at < 0:
                out.add(path.name)
                break
            first = source.count("\n", 0, at) + 1
            last = first + mutant[-2].count("\n")
            if any(first - near <= b and a <= last + near for a, b in changed[rel_target]):
                out.add(path.name)
                break
    return out


def changed_recipes(base: str, head: str, near: int, names: list[str]) -> list[str]:
    """The recipes of `names` a range of commits touches (see the module)."""
    return recipes_for(hunks(base, head), near, names)


def recipes_for(changed: dict[str, list[tuple[int, int]]], near: int, names: list[str]) -> list[str]:
    """The recipes of `names` that `changed`, each file's changed lines,
    touches (see the module)."""
    scripts = touched_scripts(changed, near)
    body = bodies()
    specs = changed_specs(changed)
    out = {
        name
        for name in names
        if any(f"scripts/{s}" in body.get(name, "") for s in scripts)
        # A recipe that runs a browser spec the range changed (ADR-0281).
        or any(s in body.get(name, "") for s in specs)
    }
    # A recipe whose own lines changed: written or changed in the range, in
    # any part of the justfile.
    for part in justfile_parts():
        if part in changed:
            text = (ROOT / part).read_text().splitlines()
            out |= recipes_at(text, changed[part], names)
    return sorted(out)


def deal(names: list[str], costs: dict[str, int], shards: int) -> list[list[str]]:
    """Deal `names` into `shards`, the costliest first, each to the least
    loaded shard; ties to the lower index, so a deal is the same every
    time."""
    loads = [0] * shards
    dealt: list[list[str]] = [[] for _ in range(shards)]
    for name in sorted(names, key=lambda n: (-costs.get(n, 1), n)):
        i = min(range(shards), key=lambda k: (loads[k], k))
        loads[i] += costs.get(name, 1)
        dealt[i].append(name)
    return [sorted(r) for r in dealt if r]


def needs(body: str) -> tuple[bool, bool]:
    """What a recipe's shard must set up (ADR-0249): browsers, for one that
    runs Playwright; the build, for one that runs it, or reads what it
    makes: the servers, the store's pages, `pw` itself."""
    browsers = "playwright" in body
    build = browsers or bool(re.search(r"run\.sh|feed\.sh|keyed-store\.sh|target/debug/", body))
    return browsers, build


def plan(
    names: list[str],
    costs: dict[str, int],
    shards: int,
    need: dict[str, tuple[bool, bool]] | None = None,
    database: frozenset[str] | set[str] = frozenset(),
) -> list[dict]:
    """Deal `names` into at most `shards`, each kind of recipe (browsers and
    a build, a build alone, neither) into shards of its own, as many as its
    share of the work, at least one where it has a recipe.

    A recipe of `database` is a shard of its own, beside a PostgreSQL of its
    own (ADR-0278), while the shards go round, leaving one for the rest
    where there are any; past that, they share theirs, dealt as any are.
    Theirs are numbered after the others'."""
    need = need or {}
    on_database = [n for n in names if n in database]
    names = [n for n in names if n not in database]
    beside = min(len(on_database), max(1, shards - (1 if names else 0))) if on_database else 0
    shards = max(1, shards - beside)
    kinds: dict[tuple[bool, bool], list[str]] = {}
    for n in names:
        kinds.setdefault(need.get(n, (False, False)), []).append(n)
    total = sum(costs.get(n, 1) for n in names) or 1
    order = sorted(kinds, reverse=True)
    share = {
        k: min(len(kinds[k]), max(1, round(shards * sum(costs.get(n, 1) for n in kinds[k]) / total)))
        for k in order
    }
    # More shards than there are: the largest share gives one back, until
    # they fit.
    while sum(share.values()) > max(shards, len(order)):
        k = max(order, key=lambda k: (share[k], k))
        share[k] -= 1
    out = []
    for k in order:
        for recipes in deal(kinds[k], costs, share[k]):
            out.append(
                {"index": len(out), "recipes": recipes, "browsers": k[0], "build": k[1], "database": False}
            )
    # Each sets up what its recipes need, as a shard of their kind would.
    for recipes in deal(on_database, costs, beside):
        setup = [need.get(n, (False, False)) for n in recipes]
        out.append(
            {
                "index": len(out),
                "recipes": recipes,
                "browsers": any(b for b, _ in setup),
                "build": any(b for _, b in setup),
                "database": True,
            }
        )
    return out


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--shards", type=int, default=16)
    parser.add_argument("--all", action="store_true")
    parser.add_argument("--changed", nargs=2, metavar=("BASE", "HEAD"))
    parser.add_argument("--near", type=int, default=30)
    parser.add_argument("recipes", nargs="*")
    args = parser.parse_args()
    known = recipes()
    evidence = [r for r in known if EVIDENCE_RECIPE.match(r) and r not in LOCAL_ONLY]
    if args.recipes:
        unknown = [r for r in args.recipes if r not in known]
        if unknown:
            print(f"ci_plan: no recipe {unknown}", file=sys.stderr)
            return 1
        names = args.recipes
    elif args.changed:
        names = changed_recipes(args.changed[0], args.changed[1], args.near, evidence)
    elif args.all:
        names = evidence
    else:
        print("ci_plan: say --all, --changed BASE HEAD, or the recipes", file=sys.stderr)
        return 1
    body = bodies()
    costs = {n: cost(body.get(n, "")) for n in names}
    need = {n: needs(body.get(n, "")) for n in names}
    print("shards=" + json.dumps(plan(names, costs, args.shards, need, NEEDS_DATABASE)))
    return 0


if __name__ == "__main__":
    sys.exit(main())

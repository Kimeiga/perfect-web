#!/usr/bin/env python3
"""Hold kiokun.com's parity inventory to the SvelteKit app it inventories.

The kiokun track's first ADR (docs/PARALLEL.md, W6's plan) lists every route
of kiokun.com's SvelteKit app, each marked built in Pleris, partial or
missing. This script reads the app's routes from a kiokun-data checkout and
the inventory's rows from the ADR, and fails unless each names the other:
- every route the app has is a row of the inventory;
- every route the inventory names is one the app has;
- every row is marked `built`, `partial` or `missing`, once.

A route is a directory under `sveltekit-app/src/routes` holding a `+page`
(a page) or a `+server.ts` (an endpoint), named as SvelteKit names it:
`/`, `/[word]`, `/api/search`. A row is a table row whose first cell is a
route in backticks and whose third cell is its status. The inventory's
feature rows, whose first cell is a feature's name, are counted by status
too, so the counts the ADR states are this script's.

It reads the checkout's committed tree at HEAD, never its working tree
(the integrator's ruling of 2026-10-10): the evidence names a commit, and
is reproducible only if what was read is that commit. Work the owner has
not committed is named as not read, and does not fail the check. It writes
nothing there: no tool of the app's is run, and git is asked only for the
commit, the committed tree and the files changed, without taking its
optional locks.

usage: kiokun_inventory.py ADR APP_DIR
  ADR      the inventory, docs/DECISIONS/ADR-0285-kiokun-parity-inventory.md
  APP_DIR  the checkout's sveltekit-app
"""

import os
import pathlib
import re
import subprocess
import sys

STATUSES = ("built", "partial", "missing")

# A table row whose first cell is a route in backticks.
ROW = re.compile(r"^\|\s*`(/[^`]*)`\s*\|")


def git(app: pathlib.Path, *args: str) -> subprocess.CompletedProcess:
    """git, in the app's directory, without its optional locks.

    `GIT_OPTIONAL_LOCKS=0` keeps `git status` from refreshing the
    checkout's index, its one write (git's documentation, `git(1)`).
    """
    env = {**os.environ, "GIT_OPTIONAL_LOCKS": "0"}
    return subprocess.run(["git", "-C", str(app), *args], capture_output=True, text=True, env=env)


def committed_routes(app: pathlib.Path) -> list[str]:
    """Each file under the app's `src/routes` in its commit at HEAD, as a
    path from `src/routes`. `ls-tree` reads the commit; `ls-files` would
    read the index, which can hold a file staged and not committed."""
    r = git(app, "ls-tree", "-r", "--name-only", "HEAD", "--", "src/routes")
    if r.returncode != 0:
        raise SystemExit(f"kiokun_inventory: {app} has no commit to read: {r.stderr.strip()}")
    paths = [line.removeprefix("src/routes/") for line in r.stdout.splitlines()]
    if not paths:
        raise SystemExit(f"kiokun_inventory: no routes under {app}/src/routes at HEAD")
    return paths


def routes_of(paths: list[str]) -> dict[str, str]:
    """Each route the files make, by its id, and its kind: page or
    endpoint. A route is a directory holding a `+page` or a `+server`."""
    names: dict[str, set[str]] = {}
    for path in paths:
        directory, _, name = path.rpartition("/")
        names.setdefault(directory, set()).add(name)
    found: dict[str, str] = {}
    for directory in sorted(names):
        page = any(n.startswith("+page.") for n in names[directory])
        endpoint = "+server.ts" in names[directory] or "+server.js" in names[directory]
        if not (page or endpoint):
            continue
        rid = "/" + directory if directory else "/"
        found[rid] = "page and endpoint" if page and endpoint else ("page" if page else "endpoint")
    return found


def app_routes(app: pathlib.Path) -> dict[str, str]:
    """Each route the app has in its commit at HEAD."""
    return routes_of(committed_routes(app))


def inventory_rows(text: str) -> list[tuple[str, str, int]]:
    """Each route row of the inventory: its route, its status and its line.

    The status is the third cell, bold or plain; anything else is kept as
    written, so the check names it.
    """
    rows = []
    for n, line in enumerate(text.splitlines(), 1):
        m = ROW.match(line)
        if not m:
            continue
        cells = [c.strip() for c in line.strip().strip("|").split("|")]
        status = cells[2].strip("*").strip().lower() if len(cells) > 2 else ""
        rows.append((m.group(1), status, n))
    return rows


def feature_counts(text: str) -> dict[str, int]:
    """The count of each status over the inventory's feature rows: a table
    row whose first cell is no route and whose third cell is a status."""
    counts = {s: 0 for s in STATUSES}
    for line in text.splitlines():
        if not line.startswith("|") or ROW.match(line):
            continue
        cells = [c.strip() for c in line.strip().strip("|").split("|")]
        status = cells[2].strip("*").strip().lower() if len(cells) > 2 else ""
        if status in counts:
            counts[status] += 1
    return counts


def check(adr_text: str, routes: dict[str, str]) -> tuple[list[str], dict[str, int]]:
    """The problems found, and the count of each status."""
    problems: list[str] = []
    counts = {s: 0 for s in STATUSES}
    seen: dict[str, int] = {}
    for route, status, line in inventory_rows(adr_text):
        if route in seen:
            problems.append(f"line {line}: {route} is listed twice (first at line {seen[route]})")
            continue
        seen[route] = line
        if status not in STATUSES:
            problems.append(f"line {line}: {route} is marked {status!r}, not one of {', '.join(STATUSES)}")
        else:
            counts[status] += 1
        if route not in routes:
            problems.append(f"line {line}: {route} is no route of the app")
    for route in sorted(routes):
        if route not in seen:
            problems.append(f"{route} ({routes[route]}) is not in the inventory")
    return problems, counts


def commit(app: pathlib.Path) -> str:
    """The checkout's commit, which is what is read."""
    return git(app, "rev-parse", "HEAD").stdout.strip() or "unknown (not a git checkout)"


def not_read(app: pathlib.Path) -> list[str]:
    """The routes' files the working tree has changed or added since HEAD:
    the owner's work, not read."""
    return [line[3:] for line in git(app, "status", "--porcelain", "--", "src/routes").stdout.splitlines()]


def main(argv: list[str]) -> int:
    if len(argv) != 3:
        print("usage: kiokun_inventory.py ADR APP_DIR", file=sys.stderr)
        return 2
    adr, app = pathlib.Path(argv[1]), pathlib.Path(argv[2])
    routes = app_routes(app)
    text = adr.read_text(encoding="utf-8")
    problems, counts = check(text, routes)
    features = feature_counts(text)
    pages = sum(1 for k in routes.values() if "page" in k)
    endpoints = sum(1 for k in routes.values() if "endpoint" in k)
    print(f"the app: {app} at {commit(app)}, its committed tree")
    unread = not_read(app)
    if unread:
        print(f"the checkout has uncommitted work, not read: {', '.join(unread)}")
    print(f"the inventory: {adr.name}")
    print(f"routes: {len(routes)} ({pages} pages, {endpoints} endpoints)")
    print(
        f"marked: {counts['built']} built, {counts['partial']} partial, "
        f"{counts['missing']} missing"
    )
    print(
        f"features: {sum(features.values())}, marked {features['built']} built, "
        f"{features['partial']} partial, {features['missing']} missing"
    )
    for p in problems:
        print(f"PROBLEM: {p}")
    print("inventory: OK" if not problems else f"inventory: {len(problems)} problem(s)")
    return 0 if not problems else 1


if __name__ == "__main__":
    sys.exit(main(sys.argv))

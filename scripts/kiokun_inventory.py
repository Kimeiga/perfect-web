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

It reads the checkout and writes nothing there: no tool of the app's is run,
and git is asked only for the commit and the files changed, without
taking its optional locks.

usage: kiokun_inventory.py ADR APP_DIR
  ADR      the inventory, docs/DECISIONS/ADR-XXXX-kiokun-parity-inventory.md
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


def app_routes(app: pathlib.Path) -> dict[str, str]:
    """Each route the app has, by its id, and its kind: page or endpoint."""
    routes_dir = app / "src" / "routes"
    if not routes_dir.is_dir():
        raise SystemExit(f"kiokun_inventory: no routes under {routes_dir}")
    found: dict[str, str] = {}
    for d in sorted([routes_dir, *[p for p in routes_dir.rglob("*") if p.is_dir()]]):
        names = {p.name for p in d.iterdir() if p.is_file()}
        page = any(n.startswith("+page.") for n in names)
        endpoint = "+server.ts" in names or "+server.js" in names
        if not (page or endpoint):
            continue
        rel = d.relative_to(routes_dir).as_posix()
        rid = "/" if rel == "." else "/" + rel
        found[rid] = "page and endpoint" if page and endpoint else ("page" if page else "endpoint")
    return found


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
    """The checkout's commit, and the files changed in its working tree.

    `GIT_OPTIONAL_LOCKS=0` keeps `git status` from refreshing the
    checkout's index, its one write (git's documentation, `git(1)`).
    """
    env = {**os.environ, "GIT_OPTIONAL_LOCKS": "0"}
    head = subprocess.run(
        ["git", "-C", str(app), "rev-parse", "HEAD"], capture_output=True, text=True, env=env
    ).stdout.strip()
    if not head:
        return "unknown (not a git checkout)"
    changed = subprocess.run(
        ["git", "-C", str(app), "status", "--porcelain", "--", "."],
        capture_output=True,
        text=True,
        env=env,
    ).stdout.splitlines()
    return head + (f" + uncommitted changes to {len(changed)} file(s)" if changed else "")


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
    print(f"the app: {app} at {commit(app)}")
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

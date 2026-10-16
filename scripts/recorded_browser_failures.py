#!/usr/bin/env python3
"""**What the evidence recorded of failed browser tests, while their recipes
passed** (track evidence-integrity-2, the integrator's ruling 6 of
2026-10-15).

Until then 81 of the 83 recipes that ran a browser suite ended it `|| true`,
and the benchmark's controls were run the same way: a failed test failed
neither its recipe nor the run, and its evidence said so to no one. This
reads every version of every evidence file the history holds (each commit's
own change on the first-parent line, a merge's against its first parent, and
each side branch's commit), section by section (`== ` headings), and names
each section version that records a failed, flaky, interrupted or not-run
browser test, a failure's listing (`1) [webkit] › ...`, a list reporter's
`✘`), or a benchmark control that failed (`negative FAIL`): once, at the
first commit that held it, with the last that still held it, the recipe that
wrote it (`produced by:`) and the commit it was recorded at (`commit:`).

In a shallow checkout (a CI runner's) it reads what the checkout holds, and
says how many versions it read.

    python3 scripts/recorded_browser_failures.py
"""

from __future__ import annotations

import pathlib
import re
import subprocess
import sys

ROOT = pathlib.Path(__file__).resolve().parent.parent
EVIDENCE = "docs/evidence"

COUNT = re.compile(r"^ +(\d+) (failed|flaky|interrupted|did not run)\b")
LISTING = re.compile(r"^ +\d+\) \[[\w-]+\] › ")
CROSS = re.compile(r"^ +✘ ")
CONTROL = re.compile(r"^T\d+ [\w-]+: .*\bFAIL\b")


def sections(text: str):
    """Each section of an evidence file: (its heading, its lines)."""
    head, lines = "(top)", []
    for line in text.splitlines():
        if line.startswith("== "):
            yield head, lines
            head, lines = line, []
        else:
            lines.append(line)
    yield head, lines


def failures(lines: list[str]) -> list[str]:
    """What a section records of a failed browser test or control."""
    out = []
    for line in lines:
        m = COUNT.match(line)
        if (m and int(m.group(1)) > 0) or LISTING.match(line) or CROSS.match(line) or CONTROL.match(line):
            out.append(line.strip())
    return out


def versions(root: pathlib.Path) -> list[tuple[str, str, str]]:
    """Each (commit, date, path) at which the history changed an evidence
    file: the first-parent line's own changes, merges' against their first
    parent, and every side branch's commits; oldest first, a parent before
    its children (many commits share a date)."""
    order = {
        sha: i
        for i, sha in enumerate(
            subprocess.run(
                ["git", "rev-list", "--topo-order", "--reverse", "HEAD"],
                cwd=root,
                capture_output=True,
                text=True,
                check=True,
            ).stdout.split()
        )
    }
    out = set()
    for extra in (["-m", "--first-parent"], ["--no-merges"]):
        log = subprocess.run(
            ["git", "log", "--full-history", *extra, "--format=C %H %ad", "--date=short", "--name-only", "--", EVIDENCE],
            cwd=root,
            capture_output=True,
            text=True,
            errors="replace",
            check=True,
        ).stdout
        sha = date = ""
        for line in log.splitlines():
            if line.startswith("C "):
                _, sha, date = line.split()
            elif line.strip():
                out.add((sha, date, line.strip()))
    return sorted(out, key=lambda v: (order.get(v[0], -1), v[2]))


def found(root: pathlib.Path = ROOT) -> tuple[list[dict], int]:
    """Each recorded failure, first held to last, and how many versions were
    read."""
    seen: dict[tuple, dict] = {}
    read = versions(root)
    cat = subprocess.Popen(["git", "cat-file", "--batch"], cwd=root, stdin=subprocess.PIPE, stdout=subprocess.PIPE)
    with cat:
        for sha, date, path in read:
            cat.stdin.write(f"{sha}:{path}\n".encode())
            cat.stdin.flush()
            header = cat.stdout.readline().decode()
            if header.rstrip().endswith("missing"):
                continue
            body = cat.stdout.read(int(header.split()[2]))
            cat.stdout.read(1)
            text = body.decode("utf-8", errors="replace")
            produced = re.search(r"^produced by: (.+)$", text, re.M)
            recorded = re.search(r"^commit: ([0-9a-f]{7,40})(.*)$", text, re.M)
            for heading, lines in sections(text):
                lines_failed = failures(lines)
                if not lines_failed:
                    continue
                key = (path, heading, tuple(lines_failed))
                entry = seen.setdefault(
                    key,
                    {
                        "path": path,
                        "heading": heading,
                        "lines": lines_failed,
                        "first": (sha, date),
                        "recipe": produced.group(1) if produced else "?",
                        "recorded": (recorded.group(1)[:8] + (" + changes" if "uncommitted" in recorded.group(2) else ""))
                        if recorded
                        else "?",
                    },
                )
                entry["last"] = (sha, date)
    # Whether the tree's own copy still records it.
    for e in seen.values():
        now = subprocess.run(["git", "show", f"HEAD:{e['path']}"], cwd=root, capture_output=True)
        text = now.stdout.decode("utf-8", errors="replace") if now.returncode == 0 else ""
        held = [lines for heading, lines in sections(text) if heading == e["heading"]]
        e["now"] = any(failures(lines) == e["lines"] for lines in held)
    first = {sha: i for i, (sha, _, _) in enumerate(read)}
    return sorted(seen.values(), key=lambda e: (first[e["first"][0]], e["path"])), len(read)


def main() -> int:
    entries, read = found()
    for e in entries:
        until = "still committed" if e["now"] else f"until {e['last'][0][:8]}"
        print(
            f"{e['first'][1]} {e['first'][0][:8]} ({until}) {e['path']} ({e['recipe']}, "
            f"recorded at {e['recorded']}) {e['heading'][3:70]}:"
        )
        for line in e["lines"][:6]:
            print(f"    {line[:150]}")
    print(
        f"recorded browser failures: {len(entries)} ({sum(1 for e in entries if e['now'])} still committed), "
        f"in {read} versions of evidence files read"
    )
    return 0


if __name__ == "__main__":
    sys.exit(main())

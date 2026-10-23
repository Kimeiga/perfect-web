#!/usr/bin/env python3
"""Block newly introduced Python lint defects without hiding legacy findings.

Compares Ruff violation counts by code and message for each changed file,
against the PR's base. Locations are intentionally excluded because unrelated
edits move lines. Duplicates are counted, not collapsed.
"""

import argparse
from collections import Counter
import json
import pathlib
import subprocess
import sys


RULES = "E4,E7,E9,F"


def added_findings(before: list[dict], after: list[dict]) -> list[tuple[str, str, int]]:
    """Return increases only; repeated existing issues do not disappear."""
    def counted(items: list[dict]) -> Counter:
        return Counter((item["code"], item["message"]) for item in items)

    diff = counted(after) - counted(before)
    return [(code, message, count) for (code, message), count in sorted(diff.items())]


def run(*argv: str, source: str | None = None, ok: tuple[int, ...] = (0,)) -> str:
    result = subprocess.run(argv, input=source, text=True, capture_output=True, check=False)
    if result.returncode not in ok:
        raise RuntimeError(f"{' '.join(argv)}: exit {result.returncode}: {result.stderr[-1200:]}")
    return result.stdout


def lint(text: str, filename: str) -> list[dict]:
    output = run(
        "ruff", "check", "--select", RULES, "--output-format", "json",
        "--stdin-filename", filename, "-",
        source=text, ok=(0, 1),
    )
    data = json.loads(output)
    if not isinstance(data, list):
        raise RuntimeError("Ruff returned no list of diagnostics")
    return data


def changed_python(base: str) -> list[str]:
    run("git", "cat-file", "-e", f"{base}^{{commit}}")
    return sorted({
        p for p in run(
            "git", "diff", "--name-only", "--diff-filter=ACMR",
            f"{base}...HEAD", "--",
        ).splitlines()
        if p.endswith(".py") and pathlib.Path(p).is_file()
    })


def previous(base: str, path: str) -> str:
    result = subprocess.run(
        ["git", "show", f"{base}:{path}"],
        text=True, capture_output=True, check=False,
    )
    if result.returncode == 0:
        return result.stdout
    if result.returncode == 128 and ("does not exist" in result.stderr or "exists on disk" in result.stderr):
        return ""
    raise RuntimeError(f"Cannot read base file {path}: {result.stderr[-400:]}")


def check(base: str) -> int:
    paths = changed_python(base)
    print(f"Ruff new-only: {len(paths)} changed Python files (base {base[:12]})")
    defects: list[tuple[str, str, str, int]] = []
    for path in paths:
        before = lint(previous(base, path), path) if previous(base, path) else []
        after = lint(pathlib.Path(path).read_text(), path)
        defects.extend((path, code, message, count) for code, message, count in added_findings(before, after))
    for path, code, message, count in defects:
        print(f"{path}: +{count} {code} {message}")
    total = sum(n for _, _, _, n in defects)
    print(f"New Ruff findings: {total}")
    return 1 if total else 0


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--base", required=True, help="PR base SHA, available in this checkout")
    args = parser.parse_args()
    try:
        return check(args.base)
    except (RuntimeError, ValueError, OSError) as exc:
        print(f"Ruff comparison invalid: {exc}", file=sys.stderr)
        return 2


if __name__ == "__main__":
    raise SystemExit(main())

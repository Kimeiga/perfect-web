#!/usr/bin/env python3
"""A small replayable record of regressions actually discovered during review.

Default mode validates the corpus without needing Rust or any network.
--replay checks explicitly whitelisted commands at the recorded before/after
commits in temporary worktrees; a green baseline is not a replay result.
"""

import argparse
import json
import pathlib
import re
import subprocess
import sys
import tempfile


ROOT = pathlib.Path(__file__).resolve().parents[1]
DEFAULT = ROOT / "docs" / "quality" / "pr-regressions.json"
SHA = re.compile(r"^[0-9a-f]{40}$")
KINDS = {"format", "lint", "dependency", "cross_browser", "security", "behavior", "evidence"}
COMMANDS = {"cargo", "just", "pnpm", "python3"}


def validate(data: dict) -> list[dict]:
    if data.get("schema") != 1 or not isinstance(data.get("cases"), list):
        raise ValueError("regression corpus must be schema 1 with a cases list")
    cases = data["cases"]
    ids: set[str] = set()
    for case in cases:
        if not isinstance(case, dict):
            raise ValueError("a regression case must be an object")
        identifier = case.get("id")
        if not isinstance(identifier, str) or not identifier or identifier in ids:
            raise ValueError("regression case ids must be unique nonempty strings")
        ids.add(identifier)
        if case.get("kind") not in KINDS:
            raise ValueError(f"{identifier}: unknown kind")
        if not str(case.get("source", "")).startswith("https://github.com/"):
            raise ValueError(f"{identifier}: source must be a GitHub URL")
        if not SHA.fullmatch(str(case.get("bad", ""))):
            raise ValueError(f"{identifier}: missing full bad revision")
        fixed = case.get("fixed")
        if fixed is not None and not SHA.fullmatch(str(fixed)):
            # Descriptive/non-replayable cases can give no fixed commit.
            raise ValueError(f"{identifier}: fixed revision must be full SHA")
        command = case.get("replay")
        if command is not None:
            if (not isinstance(command, list) or not command or
                    not all(isinstance(x, str) and x for x in command) or
                    command[0] not in COMMANDS or not fixed):
                raise ValueError(f"{identifier}: replay command or fixed SHA invalid")
            if case["bad"] == fixed:
                raise ValueError(f"{identifier}: before equals after")
    return cases


def checked(args: list[str], cwd: pathlib.Path, timeout: int = 240) -> int:
    result = subprocess.run(args, cwd=cwd, check=False, timeout=timeout)
    return result.returncode


def replay_case(case: dict) -> bool:
    """Execute the same check on two immutable revisions; do not edit the repo."""
    cmd = case["replay"]
    if cmd is None:
        raise ValueError("case has no replay command")
    with tempfile.TemporaryDirectory(prefix="pw-regression-") as directory:
        results: list[int] = []
        for name, sha in (("bad", case["bad"]), ("fixed", case["fixed"])):
            path = pathlib.Path(directory) / name
            try:
                if checked(["git", "worktree", "add", "--detach", str(path), sha], ROOT) != 0:
                    raise RuntimeError(f"cannot check out historical commit {sha}")
                results.append(checked(cmd, path))
            finally:
                if path.exists():
                    checked(["git", "worktree", "remove", "--force", str(path)], ROOT)
        print(f"{case['id']}: old={results[0]}, fixed={results[1]}")
        return results[0] != 0 and results[1] == 0


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--file", type=pathlib.Path, default=DEFAULT)
    parser.add_argument("--replay", metavar="ID",
                        help="replay only this case, requiring a local toolchain")
    args = parser.parse_args()
    try:
        cases = validate(json.loads(args.file.read_text()))
        if args.replay:
            case = next((item for item in cases if item["id"] == args.replay), None)
            if case is None or case["replay"] is None:
                raise ValueError("unknown or non-replayable regression")
            return 0 if replay_case(case) else 1
        print(f"Validated {len(cases)} historical cases; "
              f"{sum(c['replay'] is not None for c in cases)} replayable")
        return 0
    except (OSError, ValueError, RuntimeError, subprocess.TimeoutExpired) as exc:
        print(f"Invalid historical regression evidence: {exc}", file=sys.stderr)
        return 2


if __name__ == "__main__":
    raise SystemExit(main())

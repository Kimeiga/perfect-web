#!/usr/bin/env python3
"""**The accepted corpus, against the one list of the build's refusals** (ADR-0361; the coordinator's ruling of
2026-10-20, verify 39442025240). Check-is-build makes `pw check` the complete check, what builds, and it refuses the
precisely listed unsupported accepted programs (PW6601). examples/accepted/build-refusals.json lists each, with its exact message and why; this
check runs `pw check` on the accepted corpus as one program and passes only where every program not listed reports no
error and each listed one is refused with exactly its code and message. It prints the list's size.

Recipes that ran `pw check` on examples/accepted/*.pw and required it clean run this instead.
The list is an executable-support census; semantic conformance remains a separate gate.

    python3 scripts/accepted_corpus.py
"""

import json
import pathlib
import re
import subprocess
import sys

ROOT = pathlib.Path(__file__).resolve().parent.parent
LIST = ROOT / "examples/accepted/build-refusals.json"
ANSI = re.compile(r"\x1b\[[0-9;]*[A-Za-z]")
ERROR = re.compile(r"^error: \[(PW\d+)\] (.*)$")
WHERE = re.compile(r"^\s*--> (.+?):\d+:\d+\s*$")


def program():
    """The accepted corpus as one program, as every recipe checked it."""
    files = sorted(ROOT.glob("packages/pw-std/*.pw")) + sorted(ROOT.glob("packages/pw-platform-web/*.pw"))
    files += [ROOT / "examples/domain.pw"] + sorted(ROOT.glob("examples/lib/*.pw"))
    return files + sorted(ROOT.glob("examples/accepted/*.pw"))


def errors(text):
    """Each error `pw check` reports: (file name, code, message)."""
    lines = ANSI.sub("", text).splitlines()
    found = []
    for i, line in enumerate(lines):
        m = ERROR.match(line)
        if not m:
            continue
        where = next((w for w in (WHERE.match(n) for n in lines[i + 1:i + 4]) if w), None)
        found.append((pathlib.Path(where.group(1)).name if where else "?", m.group(1), m.group(2)))
    return sorted(found)


def compare(expected, got):
    """(unexpected, missing): each error the list does not name, and each listed refusal not reported exactly."""
    return [e for e in got if e not in expected], [e for e in expected if e not in got]


def main():
    listed = json.loads(LIST.read_text())["refused"]
    expected = sorted((entry["file"], entry["code"], entry["message"]) for entry in listed)
    run = subprocess.run(
        ["cargo", "run", "--quiet", "--locked", "-p", "pw-cli", "--", "check", *map(str, program()), "--plain"],
        cwd=ROOT, capture_output=True, text=True,
    )
    got = errors(run.stdout + run.stderr)
    accepted = sorted(p.name for p in ROOT.glob("examples/accepted/*.pw"))
    unexpected, missing = compare(expected, got)
    if unexpected or missing or run.returncode != (1 if expected else 0):
        print(f"accepted corpus: NOT as listed in {LIST.relative_to(ROOT)} (pw check exit {run.returncode})")
        for file, code, message in unexpected:
            print(f"  unexpected: {file}: [{code}] {message}")
        for file, code, message in missing:
            print(f"  missing:    {file}: [{code}] {message}")
        return 1
    print(f"accepted corpus: {len(accepted)} programs, {len(accepted) - len(listed)} with no error, "
          f"{len(listed)} refused exactly as listed ({LIST.relative_to(ROOT)}: {len(listed)} entries)")
    return 0


if __name__ == "__main__":
    sys.exit(main())

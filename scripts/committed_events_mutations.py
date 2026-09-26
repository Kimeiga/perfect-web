#!/usr/bin/env python3
"""Mutation controls for ADR-0104: the dev server commits the events a
command declares.

Each mutant undoes one piece: committing the declared events rather than
`CartChanged` whatever was declared, reading the command's own edges,
reading its `emits` edges, carrying the session as `current_session()`'s
value, and refusing a value the server cannot compute. The dev server's
tests must then fail.

Run from the repository root; `just e10-committed-events` records the
output. The source is restored after every mutant, whatever happens.
"""

import pathlib
import re
import subprocess
import sys

ROOT = pathlib.Path(__file__).resolve().parent.parent
SERVER = ROOT / "spikes/own-renderer/server/src/main.rs"

# (what is undone, file, anchor, replacement)
MUTANTS = [
    (
        "`CartChanged` is committed whatever is declared",
        SERVER,
        "                Ok::<_, String>(events)\n",
        "                let _ = events;\n"
        "                Ok::<_, String>(vec![pw_materialize::Event::new(\n"
        "                    &[\"Events\", \".CartChanged\"].concat(),\n"
        "                    &[session],\n"
        "                )])\n",
    ),
    (
        "any command's events are this one's",
        SERVER,
        "        .filter(|e| e.kind == pw_materialize::EdgeKind::Emits && e.from == command)",
        "        .filter(|e| e.kind == pw_materialize::EdgeKind::Emits)",
    ),
    (
        "any edge is an event",
        SERVER,
        "        .filter(|e| e.kind == pw_materialize::EdgeKind::Emits && e.from == command)",
        "        .filter(|e| e.from == command)",
    ),
    (
        "the session is not the value",
        SERVER,
        "                    \"current_session()\" => Ok(session),",
        "                    \"current_session()\" => Ok(\"\"),",
    ),
    (
        "a value is guessed",
        SERVER,
        "                    other => Err(format!(",
        "                    _ if true => Ok(session),\n                    other => Err(format!(",
    ),
]

TESTS = [
    ["cargo", "test", "--quiet", "--locked", "-p", "pw-dev-server"],
]


def run_tests():
    """(built, passed, failed) over every test command."""
    built, passed, failed = True, 0, 0
    for cmd in TESTS:
        r = subprocess.run(cmd, cwd=ROOT, capture_output=True, text=True)
        out = r.stdout + r.stderr
        results = re.findall(r"test result: \w+\. (\d+) passed; (\d+) failed", out)
        if not results:
            built = False
            continue
        for p, f in results:
            passed += int(p)
            failed += int(f)
    return built, passed, failed


def main():
    built, passed, failed = run_tests()
    print(f"baseline: {passed} passed, {failed} failed")
    if not built or failed or not passed:
        print("FAIL: the unmutated baseline is not green; no mutant can mean anything")
        return 1

    survivors = 0
    for what, path, anchor, replacement in MUTANTS:
        original = path.read_text()
        if original.count(anchor) != 1:
            print(f"{what}: ANCHOR NOT FOUND EXACTLY ONCE in {path.name}")
            survivors += 1
            continue
        try:
            path.write_text(original.replace(anchor, replacement, 1))
            built, passed, failed = run_tests()
        finally:
            path.write_text(original)
        if not built:
            verdict = "KILLED (does not build)"
        elif failed:
            verdict = f"KILLED ({failed} of {passed + failed} tests fail)"
        else:
            verdict = "SURVIVED"
            survivors += 1
        print(f"{what}: {verdict}")

    print(f"{len(MUTANTS) - survivors} of {len(MUTANTS)} mutants killed")
    return 1 if survivors else 0


if __name__ == "__main__":
    sys.exit(main())

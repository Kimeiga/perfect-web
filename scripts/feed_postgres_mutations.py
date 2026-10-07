#!/usr/bin/env python3
"""Mutation controls for ADR-0246: the feed's data in PostgreSQL, held to
what its source states.

Each mutant undoes one piece: the events committed in the command's own
transaction, the transaction's isolation set on each, the host's refusal to
serve a database that gives less than the program states, and a refused
commit answered as refused. The PostgreSQL tests of each must then fail.

They need a database: `PW_FEED_DATABASE_URL`, a throwaway one, which each
test uses in a schema of its own. Without it every test passes doing
nothing, and no mutant could be killed, so this refuses to run.

Run from the repository root; `just e14-feed-postgres` records the output.
The source is restored after every mutant, whatever happens.
"""

import os
import pathlib
import re
import subprocess
import sys

ROOT = pathlib.Path(__file__).resolve().parent.parent
LAYER = ROOT / "spikes/own-renderer/server/src/feed_pg.rs"
SERVER = ROOT / "spikes/own-renderer/server/src/main.rs"

# (what is undone, file, anchor, replacement)
MUTANTS = [
    (
        "the events commit outside the command's transaction",
        LAYER,
        "                let row = c\n"
        "                    .query_one(\n"
        '                        "INSERT INTO outbox',
        "                let row = self\n"
        "                    .pool\n"
        "                    .take()?\n"
        "                    .query_one(\n"
        '                        "INSERT INTO outbox',
    ),
    (
        "a command's transaction is the connection's default",
        LAYER,
        'Isolation::Serializable => c.batch_execute("BEGIN ISOLATION LEVEL SERIALIZABLE"),',
        'Isolation::Serializable => c.batch_execute("BEGIN"),',
    ),
    (
        "the host serves whatever its database gives",
        SERVER,
        "        let short = data::held_to(&declared, &data.grants(), &data.provides()?);\n"
        "        if !short.is_empty() {\n",
        "        let short = data::held_to(&declared, &data.grants(), &data.provides()?);\n"
        "        if short.is_empty() && !short.is_empty() {\n",
    ),
    (
        "a refused commit is answered as committed",
        LAYER,
        '        c.batch_execute("COMMIT").map_err(pg)?;\n',
        '        let _ = c.batch_execute("COMMIT");\n',
    ),
]

TESTS = [
    [
        "cargo", "test", "--quiet", "--locked", "-p", "pw-dev-server", "--",
        "tests::feed_pg::",
    ],
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
    if not os.environ.get("PW_FEED_DATABASE_URL"):
        print("FAIL: PW_FEED_DATABASE_URL is not set; without a database every test")
        print("passes doing nothing, and no mutant can mean anything")
        return 2
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

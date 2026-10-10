#!/usr/bin/env python3
"""Mutation controls for kiokun.com's parity inventory check, and for the
oracles' copy of kiokun.com's files.

A test that passes with the check removed is not evidence for it. Each
mutant undoes one thing `kiokun_inventory.py` holds the inventory to, or
one thing `kiokun_app_source.py` holds an oracle's copy to (the commit at
HEAD, never the working tree), and at least one of their tests
(scripts/tests/test_kiokun_inventory.py, test_kiokun_app_source.py) must
then fail.

Run from the repository root. The sources are restored after every mutant,
whatever happens.
"""

import pathlib
import re
import subprocess
import sys

ROOT = pathlib.Path(__file__).resolve().parent.parent
SCRIPT = ROOT / "scripts/kiokun_inventory.py"
PLAN = ROOT / "scripts/ci_plan.py"
SOURCE = ROOT / "scripts/kiokun_app_source.py"

# (what is undone, file, anchor, replacement)
MUTANTS = [
    (
        "any directory is a route",
        SCRIPT,
        "        if not (page or endpoint):",
        "        if False:",
    ),
    (
        "an endpoint is not a route",
        SCRIPT,
        '        endpoint = "+server.ts" in names[directory] or "+server.js" in names[directory]',
        "        endpoint = False",
    ),
    (
        "the index is read, which holds files staged and not committed",
        SCRIPT,
        '    r = git(app, "ls-tree", "-r", "--name-only", "HEAD", "--", "src/routes")',
        '    r = git(app, "ls-files", "--", "src/routes")',
    ),
    (
        "the owner's work is not named as not read",
        SCRIPT,
        '    return [line[3:] for line in git(app, "status", "--porcelain", "--", "src/routes").stdout.splitlines()]',
        "    return []",
    ),
    (
        "an oracle copies the working tree",
        SOURCE,
        '        shown = git(app, "show", f"HEAD:./{path}")\n',
        '        shown = subprocess.run(["cat", str(app / path)], capture_output=True)\n',
    ),
    (
        "an oracle's copy names no uncommitted work",
        SOURCE,
        "    if unread:\n",
        "    if False:\n",
    ),
    (
        "an oracle's digest is of nothing",
        SOURCE,
        '        digests.append(f"{path} {hashlib.sha256(shown.stdout).hexdigest()[:12]}")\n',
        '        digests.append(f"{path} {hashlib.sha256(b\"\").hexdigest()[:12]}")\n',
    ),
    (
        "a route listed twice passes",
        SCRIPT,
        "        if route in seen:",
        "        if False:",
    ),
    (
        "a row's status may be any word",
        SCRIPT,
        "        if status not in STATUSES:",
        "        if status == '':",
    ),
    (
        "a route the app does not have passes",
        SCRIPT,
        "        if route not in routes:",
        "        if False:",
    ),
    (
        "a route the inventory leaves out passes",
        SCRIPT,
        "        if route not in seen:",
        "        if False:",
    ),
    (
        "a route row is counted as a feature",
        SCRIPT,
        '        if not line.startswith("|") or ROW.match(line):',
        '        if not line.startswith("|"):',
    ),
    (
        "the check passes whatever it finds",
        SCRIPT,
        "    return 0 if not problems else 1",
        "    return 0",
    ),
    (
        "the recipe is planned on a runner, which has no checkout",
        PLAN,
        '    "e14-kiokun-inventory",\n',
        "",
    ),
]

TESTS = [
    [
        sys.executable,
        "-m",
        "unittest",
        "scripts/tests/test_kiokun_inventory.py",
        "scripts/tests/test_kiokun_app_source.py",
    ]
]


def run_tests():
    """(built, passed, failed) over every test command."""
    built, passed, failed = True, 0, 0
    for cmd in TESTS:
        r = subprocess.run(cmd, cwd=ROOT, capture_output=True, text=True)
        out = r.stdout + r.stderr
        ran = re.search(r"^Ran (\d+) tests?", out, re.M)
        if ran is None:
            built = False
            continue
        bad = re.search(r"^FAILED \((.*)\)", out, re.M)
        n_bad = sum(int(n) for n in re.findall(r"=(\d+)", bad.group(1))) if bad else 0
        passed += int(ran.group(1)) - n_bad
        failed += n_bad
    return built, passed, failed


def main():
    import mutation_baseline  # keeps what each command says, for a red baseline
    built, passed, failed = run_tests()
    print(f"baseline: {passed} passed, {failed} failed")
    if not built or failed or not passed:
        print("FAIL: the unmutated baseline is not green; no mutant can mean anything")
        mutation_baseline.explain()
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
            verdict = "KILLED (does not run)"
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

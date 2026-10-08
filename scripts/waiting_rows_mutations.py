#!/usr/bin/env python3
"""Mutation controls for ADR-0275: a row shown before the server answers
waits.

A post or a reply the page shows before the server answers has an id the
page made, `pending-..`, which no server has. Until ADR-0275 its Like and
Delete acted on it, and found no such post (ADR-0274), and its links named a
post and a user no server has. Each mutant undoes one piece: which rows
wait (every row, none, or a reply's alone); the timeline row's Like and
Delete enabled while it waits; and its author's and its handle's links. The
feed's browser tests must then fail.

Each mutant is the program's own source, so the feed is built again
(`feed.sh`) before its suite runs, in Chromium alone: the test of the row
that waits, and of a reply shown before the server answers. `just e14-waiting-rows` runs the suite in three engines,
and records this script's output. Run from the repository root. The source
is restored after every mutant, whatever happens, and the feed built again
as it is at the end.
"""

import os
import pathlib
import re
import signal
import subprocess
import sys

ROOT = pathlib.Path(__file__).resolve().parent.parent
APP = ROOT / "examples/feed/app.pw"

# (what is undone, file, anchor, replacement)
MUTANTS = [
    (
        "a row the server made waits too",
        APP,
        '    String.starts_with(id.value, "pending-")\n',
        '    String.starts_with(id.value, "")\n',
    ),
    (
        "no row waits",
        APP,
        '    String.starts_with(id.value, "pending-")\n',
        '    String.starts_with(id.value, "pending-none-")\n',
    ),
    (
        "a reply waits, and a post the timeline shows does not",
        APP,
        '    String.starts_with(id.value, "pending-")\n',
        '    String.starts_with(id.value, "pending-reply-")\n',
    ),
    (
        "a waiting row's Like acts",
        APP,
        '<button type="button" disabled={waits(p.id)} on:press={resumable(captures = { p }) => match like(p.id) {',
        '<button type="button" on:press={resumable(captures = { p }) => match like(p.id) {',
    ),
    (
        "a waiting row's Delete acts",
        APP,
        '<button type="button" class="delete" disabled={waits(p.id)} on:press={resumable(captures = { p }) => match delete(p.id) {',
        '<button type="button" class="delete" on:press={resumable(captures = { p }) => match delete(p.id) {',
    ),
    (
        "a waiting row's author links to the post",
        APP,
        '<a class="author">{p.author.name}</a>',
        '<a class="author" href="/post/{p.id}">{p.author.name}</a>',
    ),
    (
        "a waiting row's handle links to its author",
        APP,
        '<a class="handle">{p.author.handle}</a>',
        '<a class="handle" href="/user/{p.author.id}">{p.author.handle}</a>',
    ),
]

FEED_BUILD = ["bash", "spikes/own-renderer/feed.sh"]
SUITE = [
    "pnpm", "exec", "playwright", "test", "e2e/feed.spec.mjs", "--project=chromium",
    "--grep", "waits, and acts once it is the server's|a reply shows before the server answers, and is",
    "--reporter=line",
]

# How long one command may run. Past it, the command and everything it
# started are stopped, and the run counts as not built.
BOUND = 900


def bounded(cmd, cwd):
    """(exit status or None past the bound, output) of one command."""
    p = subprocess.Popen(
        cmd, cwd=cwd, stdout=subprocess.PIPE, stderr=subprocess.STDOUT, text=True,
        start_new_session=True,
    )
    try:
        out, _ = p.communicate(timeout=BOUND)
        return p.returncode, out
    except subprocess.TimeoutExpired:
        os.killpg(p.pid, signal.SIGKILL)
        out, _ = p.communicate()
        return None, out


def run_tests():
    """(built, passed, failed): the feed built from the source as it is,
    then its two rows' tests in Chromium."""
    code, _ = bounded(FEED_BUILD, ROOT)
    if code != 0:
        return False, 0, 0
    _, out = bounded(SUITE, ROOT / "spikes/own-renderer")
    out = re.sub(r"\x1b\[[0-9;]*[A-Za-z]", "", out)
    passed = sum(int(n) for n in re.findall(r"^\s+(\d+) passed", out, re.M))
    failed = sum(int(n) for n in re.findall(r"^\s+(\d+) (?:failed|did not run)", out, re.M))
    return passed + failed > 0, passed, failed


def main():
    import mutation_baseline  # keeps what each command says, for a red baseline
    # Stopped from outside, the source is still restored: a stop is an
    # exception here, which the `finally` below meets.
    signal.signal(signal.SIGTERM, lambda *_: sys.exit(143))
    built, passed, failed = run_tests()
    print(f"baseline: {passed} passed, {failed} failed")
    if not built or failed or not passed:
        print("FAIL: the unmutated baseline is not green; no mutant can mean anything")
        mutation_baseline.explain()
        return 1

    survivors = 0
    try:
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
            print(f"{what}: {verdict}", flush=True)
    finally:
        # The feed the mutants built, built again as it is.
        bounded(FEED_BUILD, ROOT)

    print(f"{len(MUTANTS) - survivors} of {len(MUTANTS)} mutants killed")
    return 1 if survivors else 0


if __name__ == "__main__":
    sys.exit(main())

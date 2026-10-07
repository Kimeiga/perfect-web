#!/usr/bin/env python3
"""Mutation controls for ADR-0257: the follows timeline.

Each mutant undoes one piece: the timeline of those a reader follows taking
only theirs and its own; a follow and an unfollow kept; a follow of oneself
or of no one refused; a page counting who follows and whom, and found only
for one the feed knows; the relation deciding the button; and a follow telling
another reader of the page. The tests in
`spikes/own-renderer/server/src/tests/follows.rs` must then fail. The
PostgreSQL layer's follows are `feed_postgres_mutations.py`'s, which runs
where a database is named.

Run from the repository root; `just e14-follows` records the output. The
source is restored after every mutant, whatever happens.
"""

import pathlib
import re
import subprocess
import sys

ROOT = pathlib.Path(__file__).resolve().parent.parent
FEED = ROOT / "spikes/own-renderer/server/src/feed.rs"
PG = ROOT / "spikes/own-renderer/server/src/feed_pg.rs"
APP = ROOT / "examples/feed/app.pw"

# (what is undone, file, anchor, replacement)
MUTANTS = [
    (
        "the timeline of those you follow is everyone's",
        FEED,
        "                        r.author == reader\n"
        "                            || s.follows.contains(&(reader.clone(), r.author.clone()))\n",
        "                        r.author == reader || !r.author.is_empty()\n",
    ),
    (
        "a reader's own posts are not in its timeline",
        FEED,
        "                        r.author == reader\n"
        "                            || s.follows.contains(&(reader.clone(), r.author.clone()))\n",
        "                        s.follows.contains(&(reader.clone(), r.author.clone()))\n",
    ),
    (
        "a follow is not kept",
        FEED,
        "                    self.state.follows.insert((follower, followee));\n",
        "                    let _ = (follower, followee);\n",
    ),
    (
        "an unfollow is not kept",
        FEED,
        "                    self.state.follows.remove(&(follower, followee));\n",
        "                    let _ = (follower, followee);\n",
    ),
    (
        "a reader may follow itself",
        FEED,
        "                    if follower == *user || !known(&s, user) {\n",
        "                    if !known(&s, user) {\n",
    ),
    (
        "one the feed does not know may be followed",
        FEED,
        "                    if follower == *user || !known(&s, user) {\n",
        "                    if follower == *user {\n",
    ),
    (
        "a page counts no follower",
        FEED,
        "    let followers = state.follows.iter().filter(|(_, b)| b == id).count();\n",
        "    let followers = 0usize;\n",
    ),
    (
        "a page counts whom they follow as who follows them",
        FEED,
        "    let following = state.follows.iter().filter(|(a, _)| a == id).count();\n",
        "    let following = state.follows.iter().filter(|(_, b)| b == id).count();\n",
    ),
    (
        "the reader follows no one",
        FEED,
        "    } else if state\n"
        "        .follows\n"
        "        .contains(&(reader.to_string(), user.to_string()))\n"
        "    {\n",
        "    } else if false {\n",
    ),
    (
        "the reader's own page is another's",
        FEED,
        "    let case = if reader == user {\n",
        "    let case = if false {\n",
    ),
    (
        "a page of no one is someone's",
        FEED,
        "            [Val::String(id)] => Ok(vec![match known(&s, id) {\n",
        "            [Val::String(id)] => Ok(vec![match !id.is_empty() {\n",
    ),
    (
        "a follow tells no other reader of the page",
        APP,
        "    invalidates_on Followed(_, id), Unfollowed(_, id), Followed(id, _),\n",
        "    invalidates_on Unfollowed(_, id), Followed(id, _),\n",
    ),
]

TESTS = [
    ["cargo", "test", "--quiet", "--locked", "-p", "pw-dev-server", "--", "tests::follows::"],
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

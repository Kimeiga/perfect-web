#!/usr/bin/env python3
"""Mutation controls for ADR-0238: a command speculates on several entries,
a page on those it shows.

Each mutant undoes one piece: the arms parsed as nodes of their own, each
lowered, each transition paired with its own target and its name typed as
that target's value; an arm whose target a page does not show left to the
server, and a page that shows none given no module; and the feed's like on
the thread page.

The compiler's tests must fail for each ("cargo"); the feed's, in three
engines, for the feed's ("browser").

Run from the repository root; `just e14-speculated-arms` records the output.
The source is restored after every mutant, whatever happens.
"""

import os
import pathlib
import re
import signal
import subprocess
import sys

ROOT = pathlib.Path(__file__).resolve().parent.parent
SPECULATION = ROOT / "compiler/pw-core/src/backend/speculation.rs"
LOWER = ROOT / "compiler/pw-core/src/lower.rs"
HIR = ROOT / "compiler/pw-core/src/hir.rs"
VALUES = ROOT / "compiler/pw-core/src/values.rs"
GRAMMAR = ROOT / "compiler/pw-syntax/src/grammar.rs"
FEED = ROOT / "examples/feed/app.pw"

# (what is undone, suite, file, anchor, replacement)
MUTANTS = [
    (
        "an arm whose target the page does not show refuses the page",
        "cargo",
        SPECULATION,
        "                if !bindings.iter().any(|b| b.resource == resource) {\n"
        "                    continue;\n"
        "                }\n",
        "",
    ),
    (
        "a page that speculates on nothing has a module",
        "cargo",
        SPECULATION,
        "            if let Encoding::Encoded(m) = &module\n"
        "                && m.commands.is_empty()\n"
        "            {\n"
        "                continue;\n"
        "            }\n",
        "",
    ),
    (
        "the arms are no nodes of their own",
        "cargo",
        GRAMMAR,
        "        self.start(K::TransitionArm);\n",
        "        self.start(K::ErrorExpr);\n",
    ),
    (
        "an arm with no transition after its arrow takes the comma after it",
        "cargo",
        GRAMMAR,
        "        if !self.at_eof() && !self.at(Kind::Comma) {\n",
        "        if !self.at_eof() {\n",
    ),
    (
        "only a clause's first arm is lowered",
        "cargo",
        LOWER,
        "                .filter(|c| c.kind() == K::TransitionArm)\n",
        "                .filter(|c| c.kind() == K::TransitionArm)\n"
        "                .take(1)\n",
    ),
    (
        "each transition is paired with the clause's first target",
        "cargo",
        HIR,
        "                        if let Some(t) = target.take() {\n",
        "                        if let Some(t) = p.roots.first().filter(|_| target.take().is_some()) {\n",
    ),
    (
        "an arm's name is the clause's last target's value",
        "cargo",
        VALUES,
        "                    .take_while(|r| !std::ptr::eq(*r, root))\n",
        "                    .take_while(|_| true)\n",
    ),
    (
        "the feed's like on the thread page is not speculated",
        "browser",
        FEED,
        "    optimistic    Timeline(current_session(), _) as feed => liked(feed, post),\n"
        "                  FollowingTimeline(current_session(), _) as feed => liked(feed, post),\n"
        "                  Thread(post) as thread => liked_thread(thread, post)\n",
        "    optimistic    Timeline(current_session(), _) as feed => liked(feed, post),\n"
        "                  FollowingTimeline(current_session(), _) as feed => liked(feed, post)\n",
    ),
]

CARGO = [
    [
        "cargo", "test", "--quiet", "--locked", "-p", "pw-core",
        "--test", "speculated_arms",
    ],
    [
        "cargo", "test", "--quiet", "--locked", "-p", "pw-syntax", "--lib", "--",
        "an_optimistic_clause_has_an_arm_for_each_entry",
    ],
]

SPEC = ["e2e/feed.spec.mjs"]

# How long one command may run. Past the bound it is killed with everything
# it started, and the run counts as failing.
BOUND = 1200


def bounded(cmd, **kw):
    """(output, returncode), or (output, None) when it ran past the bound."""
    p = subprocess.Popen(cmd, start_new_session=True, stdout=subprocess.PIPE,
                         stderr=subprocess.STDOUT, text=True, **kw)
    try:
        out, _ = p.communicate(timeout=BOUND)
        return out, p.returncode
    except subprocess.TimeoutExpired:
        os.killpg(p.pid, signal.SIGKILL)
        out, _ = p.communicate()
        return out, None


def cargo_tests():
    """(built, passed, failed) over every test command."""
    built, passed, failed = True, 0, 0
    for cmd in CARGO:
        out, code = bounded(cmd, cwd=ROOT)
        results = re.findall(r"test result: \w+\. (\d+) passed; (\d+) failed", out or "")
        if code is None:
            failed += 1
            continue
        if not results:
            built = False
            continue
        for p, f in results:
            passed += int(p)
            failed += int(f)
    return built, passed, failed


def build():
    """The store's and the feed's pages, with the runtime they load, and the
    server, as the source says."""
    _, page = bounded(["bash", "spikes/own-renderer/run.sh"], cwd=ROOT,
                      env={**os.environ, "BUILD_ONLY": "1"})
    _, feed = bounded(["bash", "spikes/own-renderer/feed.sh"], cwd=ROOT)
    _, server = bounded(["cargo", "build", "--quiet", "--locked", "-p", "pw-dev-server"], cwd=ROOT)
    return page == 0 and feed == 0 and server == 0


def browser_tests():
    """(built, passed, failed) for the spec in three engines, after a build.
    A spec skipped, as one whose build is stale is, ran nothing: not built."""
    if not build():
        return False, 0, 0
    out, code = bounded(
        ["pnpm", "exec", "playwright", "test", *SPEC, "--reporter=line"],
        cwd=ROOT / "spikes/own-renderer",
    )
    if code is None:
        return True, 0, 1
    out = re.sub(r"\x1b\[[0-9;]*[A-Za-z]", "", out)
    passed = sum(int(n) for n in re.findall(r"(\d+) passed", out))
    failed = sum(int(n) for n in re.findall(r"(\d+) failed", out))
    return passed + failed > 0, passed, failed


SUITES = {"cargo": cargo_tests, "browser": browser_tests}


def main():
    # Stopped from outside, the source is still restored: SIGTERM raises
    # here, and the `finally` below runs.
    signal.signal(signal.SIGTERM, lambda *_: sys.exit(1))
    for suite in sorted({m[1] for m in MUTANTS}):
        built, passed, failed = SUITES[suite]()
        print(f"baseline ({suite}): {passed} passed, {failed} failed", flush=True)
        if not built or failed or not passed:
            print("FAIL: the unmutated baseline is not green; no mutant can mean anything")
            return 1

    survivors = 0
    try:
        for what, suite, path, anchor, replacement in MUTANTS:
            original = path.read_text()
            if original.count(anchor) != 1:
                print(f"{what}: ANCHOR NOT FOUND EXACTLY ONCE in {path.name}")
                survivors += 1
                continue
            try:
                path.write_text(original.replace(anchor, replacement, 1))
                built, passed, failed = SUITES[suite]()
            finally:
                path.write_text(original)
            if not built:
                verdict = "KILLED (does not build)"
            elif failed:
                verdict = f"KILLED ({failed} of {passed + failed} tests fail)"
            else:
                verdict = "SURVIVED"
                survivors += 1
            print(f"{what} [{suite}]: {verdict}", flush=True)
    finally:
        # The pages and the server as the source says, whatever a mutant
        # left built.
        build()

    print(f"{len(MUTANTS) - survivors} of {len(MUTANTS)} mutants killed")
    return 1 if survivors else 0


if __name__ == "__main__":
    sys.exit(main())

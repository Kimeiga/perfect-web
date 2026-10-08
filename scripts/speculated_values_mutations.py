#!/usr/bin/env python3
"""Mutation controls for ADR-0235: what a speculated value computes, the
page's module computes wherever the page shows it.

Each mutant undoes one piece: a block's subject computed from a speculated
value whole compiled; its block a region; a value the module computes held
by its region; one inside a block no region renders refused by name; and the
module exporting what it computes.

The compiler's tests must fail for each ("cargo"), the module run under Node.

Run from the repository root; `just e14-speculated-values` records the
output. The source is restored after every mutant, whatever happens.
"""

import os
import pathlib
import re
import signal
import subprocess
import sys

ROOT = pathlib.Path(__file__).resolve().parent.parent
SPECULATION = ROOT / "compiler/pw-core/src/backend/speculation.rs"

# (what is undone, suite, file, anchor, replacement)
MUTANTS = [
    (
        "a block's subject computed from a speculated value is refused",
        "cargo",
        SPECULATION,
        "            if speculates(root) && (what == \"an attribute's value\" || read != root) {\n",
        "            if speculates(root) && (what != \"a value\" || read != root) {\n",
    ),
    (
        "a block whose subject is computed from a speculated value is no region",
        "cargo",
        SPECULATION,
        "        if let Some(binding) = whole.get(&read.path) {\n",
        "        if let Some(binding) = whole.get(&read.path).filter(|_| false) {\n",
    ),
    (
        "a value the module computes is not one its region holds",
        "cargo",
        SPECULATION,
        "            if !held(root) && !computes(&path) {\n",
        "            if !held(root) {\n",
    ),
    (
        "a value inside a block no region renders is skipped",
        "cargo",
        SPECULATION,
        "        if !own && !in_a_region(part) {\n",
        "        if !own && !in_a_region(part) && false {\n",
    ),
    (
        "the module exports nothing it computes",
        "cargo",
        SPECULATION,
        "        let mine: Vec<String> = in_regions\n"
        "            .iter()\n"
        "            .filter(|(b, ..)| b == name)\n",
        "        let mine: Vec<String> = in_regions\n"
        "            .iter()\n"
        "            .filter(|_| false)\n",
    ),
]

CARGO = [
    [
        "cargo", "test", "--quiet", "--locked", "-p", "pw-core",
        "--test", "speculated_values",
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
    import mutation_baseline  # keeps what each command says, for a red baseline
    # Stopped from outside, the source is still restored: SIGTERM raises
    # here, and the `finally` below runs.
    signal.signal(signal.SIGTERM, lambda *_: sys.exit(1))
    for suite in sorted({m[1] for m in MUTANTS}):
        built, passed, failed = SUITES[suite]()
        print(f"baseline ({suite}): {passed} passed, {failed} failed", flush=True)
        if not built or failed or not passed:
            print("FAIL: the unmutated baseline is not green; no mutant can mean anything")
            mutation_baseline.explain()
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

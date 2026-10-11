#!/usr/bin/env python3
"""Which test targets kill each mutant of a mutation script (NEXT,
infrastructure: each mutant given the tests that can see its rule).

A mutation script that runs a crate's tests whole for every mutant pays
for every test target each time: `pw-core`'s 211 are each linked again
per mutant, so `e14-cart-lines` took 16,552 s of a verify. W6's ruling of
2026-10-09 gives a mutant that takes long the tests that can see its rule.
Which tests those are is measured here, not guessed: each mutant is
applied, the command is run whole with `--no-fail-fast`, and each test
target with a failing test is recorded. A script narrowed to the union
of its mutants' killers kills each of them, by construction; it is then
run whole again before its evidence is recorded.

    python3 scripts/mutant_killers.py scripts/cart_lines_mutations.py \\
        --suite core -- cargo test --locked -p pw-core

`--suite` keeps the mutants whose suite (the element before the file) is
that name; without it, every mutant. The command is run as given, with
`--no-fail-fast` added after `test`. Prints, for each mutant, the targets
that fail under it, and where one target alone fails, its failing tests by
name (a suite of one target, the development server's, is narrowed by
test name), then the union as `--test` arguments. A mutant no
target fails is a survivor of the whole suite, and is said so: narrowing
cannot help it, and the script's own run reports it.

Run from the repository root. The source is restored after every mutant,
whatever happens, and on SIGTERM.
"""

import argparse
import importlib.util
import json
import os
import pathlib
import re
import signal
import subprocess
import sys

sys.dont_write_bytecode = True

ROOT = pathlib.Path(__file__).resolve().parent.parent

# How long one run of the command may take before it is a hang, which is a
# kill (a mutant that loops is seen), as the scripts' own `run_tests` say.
TIMEOUT = 3600

# `Running tests/labels.rs (target/debug/deps/labels-0123abcd)`, or
# `Running unittests src/lib.rs (..)`: the target whose results follow.
RUNNING = re.compile(r"^\s*Running (\S+)(?: \(([^)]*)\))?")
RESULT = re.compile(r"test result: (ok|FAILED)\. (\d+) passed; (\d+) failed")
# `test tests::a_change_reaches_each_page ... FAILED`: a failing test, by the
# name a filter matches (`-- --exact <name>`), for a suite of one target
# such as the development server's.
FAILING = re.compile(r"^test (\S+) \.\.\. FAILED")


def tests_failed(output):
    """Each failing test's name, in the order the run reported them."""
    names = []
    for line in output.splitlines():
        m = FAILING.match(line.strip())
        if m and m.group(1) not in names:
            names.append(m.group(1))
    return names


# Cargo's own account at the end of a `--no-fail-fast` run: `error: 2
# targets failed:`, then each target's arguments, `` `-p pw-core --test
# labels` ``, one a line.
SUMMARY = re.compile(r"^error: \d+ targets? failed:")
NAMED = re.compile(r"`(?:-p \S+ )?(--test \S+|--lib|--bin \S+|--doc)`")


def targets_failed(output):
    """(each target with a failing test, whether any target ran). By cargo's
    closing account where it gives one, else by the target each failing
    result follows."""
    lines = output.splitlines()
    for i, line in enumerate(lines):
        if SUMMARY.match(line.strip()):
            named = []
            for after in lines[i + 1 :]:
                m = NAMED.search(after)
                if not m:
                    break
                arg = m.group(1)
                named.append(arg.split(" ", 1)[1] if arg.startswith("--test ") else arg)
            ran = any(RESULT.search(l) for l in lines)
            return named, ran
    failed, current, ran = [], None, False
    for line in output.splitlines():
        m = RUNNING.match(line)
        if m:
            path, binary = m.group(1), m.group(2) or ""
            # The `--test` name of an integration test is its file's stem;
            # a library's or binary's unit tests are named by their source.
            if path.startswith("tests/") or "/tests/" in path:
                current = pathlib.Path(path).stem
            else:
                current = path if not binary else f"{path} ({pathlib.Path(binary).name})"
            continue
        r = RESULT.search(line)
        if r:
            ran = True
            if r.group(1) == "FAILED" and current and current not in failed:
                failed.append(current)
    return failed, ran


def run(command):
    """(output, timed out) of one run of `command`, its process group
    stopped at `TIMEOUT`."""
    p = subprocess.Popen(
        command,
        cwd=ROOT,
        stdout=subprocess.PIPE,
        stderr=subprocess.STDOUT,
        text=True,
        start_new_session=True,
    )
    try:
        out, _ = p.communicate(timeout=TIMEOUT)
        return out, False
    except subprocess.TimeoutExpired:
        os.killpg(p.pid, signal.SIGKILL)
        out, _ = p.communicate()
        return out, True


def mutants_of(script, suite):
    spec = importlib.util.spec_from_file_location(script.stem, script)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    chosen = []
    for mutant in module.MUTANTS:
        what = mutant[0]
        path, anchor, replacement = mutant[-3:]
        their_suite = mutant[-4] if len(mutant) >= 5 else None
        if suite is None or their_suite == suite:
            chosen.append((what, pathlib.Path(path), anchor, replacement))
    return chosen


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("script")
    parser.add_argument("--suite")
    parser.add_argument("--json", help="also write the killers here")
    parser.add_argument("command", nargs=argparse.REMAINDER)
    args = parser.parse_args()
    command = [c for c in args.command if c != "--"]
    if "test" not in command:
        parser.error("the command is a `cargo test` command")
    # Every target runs, whatever fails before it.
    at = command.index("test") + 1
    command = command[:at] + ["--no-fail-fast"] + command[at:]

    script = pathlib.Path(args.script).resolve()
    mutants = mutants_of(script, args.suite)
    if not mutants:
        print(f"no mutants of suite {args.suite!r} in {script.name}")
        return 1

    applied = {}

    def restore(*_):
        for path, original in applied.items():
            path.write_text(original)
        applied.clear()

    signal.signal(signal.SIGTERM, lambda *a: (restore(), sys.exit(1)))

    out, hung = run(command)
    failed, ran = targets_failed(out)
    if hung or failed or not ran:
        print("FAIL: the unmutated suite is not green: " + (", ".join(failed) or "no test ran"))
        print(out[-3000:])
        return 1
    print(f"baseline: green ({' '.join(command)})", flush=True)

    killers, report = [], {}
    for what, path, anchor, replacement in mutants:
        original = path.read_text()
        if original.count(anchor) != 1:
            print(f"{what}: ANCHOR NOT FOUND EXACTLY ONCE in {path.name}")
            report[what] = None
            continue
        try:
            applied[path] = original
            path.write_text(original.replace(anchor, replacement, 1))
            out, hung = run(command)
        finally:
            restore()
        failed, ran = targets_failed(out)
        tests = tests_failed(out)
        if hung:
            verdict = "a hang (every target is a killer: narrow none)"
            failed = ["*"]
        elif not ran:
            verdict = "does not build (no target needed)"
        elif failed:
            verdict = "killed by " + ", ".join(failed)
            if len(failed) == 1:
                shown = " ".join(tests[:8]) + (" .." if len(tests) > 8 else "")
                verdict += f" ({len(tests)} tests: {shown})"
        else:
            verdict = "SURVIVED the whole suite"
        print(f"{what}: {verdict}", flush=True)
        report[what] = {"targets": failed, "tests": tests}
        for t in failed:
            if t not in killers:
                killers.append(t)

    print()
    if "*" in killers:
        print("narrowing: none (a mutant hung, and every target may be what sees it)")
    else:
        names = sorted(k for k in killers if not k.startswith("--") and " " not in k)
        others = sorted(k for k in killers if k.startswith("--") or " " in k)
        print("narrowed: " + " ".join([f"--test {k}" for k in names] + others))
    if args.json:
        pathlib.Path(args.json).write_text(json.dumps(report, indent=2) + "\n")
    return 0


if __name__ == "__main__":
    sys.exit(main())

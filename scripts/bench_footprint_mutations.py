#!/usr/bin/env python3
"""Mutation controls for the benchmark's changed lines (E14-E's footprint):
a run counts the files an agent makes beside the ones it edits, and nothing
the sandbox ignores, and refuses changes git cannot list.

Each mutant undoes one piece: a made file not counted; an ignored file
counted; removed lines not counted; a failure to list the changes counted
as none. `benchmarks/harness/test/changed-lines.test.mjs` must fail each, by
an assertion of its witness test in a complete run: a run past its bound, a
test that fails by an error, another test failing, or a test missing from
the baseline's inventory kills nothing.

Run from the repository root; `just e14-bench-footprint` records the output.
The source is restored after every mutant, whatever happens.
"""

import os
import pathlib
import re
import signal
import subprocess
import sys

import mutation_source

ROOT = pathlib.Path(__file__).resolve().parent.parent
HARNESS = ROOT / "benchmarks/harness"
BENCH = HARNESS / "bench.mjs"
TEST = "test/changed-lines.test.mjs"

MADE = "a file the agent makes counts with the lines it edits, and an ignored one does not"
REFUSED = "changes git cannot list are refused, not counted as none"

# (what is undone, file, anchor, replacement); each one's witness test is in
# WITNESSES.
MUTANTS = [
    (
        "a file the agent makes is not counted",
        BENCH,
        '  const marked = sh("git", ["add", "--intent-to-add", "--all"], { cwd: app });\n',
        '  const marked = { ok: true, out: "" };\n',
    ),
    (
        "what the sandbox ignores is counted",
        BENCH,
        '  const marked = sh("git", ["add", "--intent-to-add", "--all"], { cwd: app });\n',
        '  const marked = sh("git", ["add", "--intent-to-add", "--all", "--force"], { cwd: app });\n',
    ),
    (
        "removed lines are not counted",
        BENCH,
        '    ? diff.split("\\n").reduce((n, l) => n + l.split("\\t").slice(0, 2)',
        '    ? diff.split("\\n").reduce((n, l) => n + l.split("\\t").slice(0, 1)',
    ),
    (
        "changes git cannot list are counted as none",
        BENCH,
        "  if (!marked.ok) throw new Error(`the changed files could not be listed: ${tail(marked.out)}`);\n",
        "",
    ),
]

# The test each mutant must fail, by an assertion.
WITNESSES = {
    "a file the agent makes is not counted": MADE,
    "what the sandbox ignores is counted": MADE,
    "removed lines are not counted": MADE,
    "changes git cannot list are counted as none": REFUSED,
}

# How long one run may take before it is a hang: killed, and no kill.
BOUND = 600

TOP = re.compile(r"^(not ok|ok) \d+ - (.*)$")


def run_suite():
    """(exit status, {test: None where it passed, else its failure's code}),
    or (None, {}) past the bound."""
    child = subprocess.Popen(
        ["node", "--test", "--test-reporter=tap", TEST],
        cwd=HARNESS, start_new_session=True,
        stdout=subprocess.PIPE, stderr=subprocess.STDOUT, text=True,
    )
    try:
        out, _ = child.communicate(timeout=BOUND)
    except subprocess.TimeoutExpired:
        os.killpg(child.pid, signal.SIGKILL)
        child.communicate()
        return None, {}
    except BaseException:
        os.killpg(child.pid, signal.SIGKILL)
        child.communicate()
        raise
    results, current = {}, None
    for line in out.splitlines():
        top = TOP.match(line)
        if top:
            current = top.group(2).replace("\\#", "#")
            results[current] = None if top.group(1) == "ok" else "failed"
            continue
        code = re.match(r"^  code: '([A-Z_]+)'$", line)
        if code and current is not None and results.get(current) == "failed":
            results[current] = code.group(1)
    if any(r is not None for r in results.values()):
        print(out, flush=True)
    return child.returncode, results


def main():
    import mutation_baseline  # keeps what each command says, for a red baseline
    signal.signal(signal.SIGTERM, lambda *_: sys.exit(1))
    status, baseline = run_suite()
    passed = sum(1 for r in baseline.values() if r is None)
    print(f"baseline: {passed} passed, {len(baseline) - passed} failed", flush=True)
    if status != 0 or not baseline or any(r is not None for r in baseline.values()):
        print("FAIL: the unmutated baseline is not green; no mutant can mean anything")
        mutation_baseline.explain()
        return 1
    missing = sorted(set(WITNESSES.values()) - set(baseline))
    if missing:
        print("FAIL: a mutant names a test the suite does not have: " + "; ".join(missing))
        return 1

    killed = refused = survivors = 0
    original = BENCH.read_text()
    for what, path, anchor, replacement in MUTANTS:
        witness = WITNESSES[what]
        if original.count(anchor) != 1:
            print(f"{what}: REFUSED (anchor not unique in {path.name})", flush=True)
            refused += 1
            continue
        with mutation_source.changed(path, original, original.replace(anchor, replacement, 1)):
            status, results = run_suite()
        if status is None or set(results) != set(baseline):
            verdict = "REFUSED (incomplete run; no kill)"
            refused += 1
        elif results.get(witness) == "ERR_ASSERTION":
            failing = sum(1 for r in results.values() if r is not None)
            verdict = f"KILLED (by an assertion of its test; {failing} of {len(results)} tests fail)"
            killed += 1
        else:
            print(f"  (its test {witness!r}: {results.get(witness) or 'passed'})", flush=True)
            verdict = "SURVIVED"
            survivors += 1
        print(f"{what}: {verdict}", flush=True)
    print(f"{killed} of {len(MUTANTS)} mutants killed; {refused} refused; {survivors} survived")
    return int(bool(refused or survivors))


if __name__ == "__main__":
    sys.exit(main())

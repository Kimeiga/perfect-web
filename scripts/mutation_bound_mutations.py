#!/usr/bin/env python3
"""Mutation controls for the bound in memory on a mutation script's
processes (`mutation_bound.py`).

Each mutant undoes one thing the bound does: stopping a process past it,
watching what the script's processes started, saying each stop once, a
process at the bound within it, the script and another's processes left
unread, a second reading before a stop, a number used again watched again,
the bound itself, a watch that fails saying so, the macOS footprint's place
in its structure, a kill by the bound named apart (after the verdict it may
have decided, in the script's last line, and in the run's summary), and
`mutation_baseline` starting the bound.

The tests that watch real processes stop only processes they marked, so a
mutant that watches every process stops none of the machine's. At least one test of
`scripts/tests/test_mutation_bound.py` or `test_mutation_baseline.py` must
then fail.

Run from the repository root; `just e14-mutation-bound` records the output.
The sources are restored after every mutant, whatever happens.
"""

import pathlib
import re
import signal
import subprocess
import sys

ROOT = pathlib.Path(__file__).resolve().parent.parent
BOUND = ROOT / "scripts/mutation_bound.py"
BASELINE = ROOT / "scripts/mutation_baseline.py"
SUMMARY = ROOT / "scripts/ci_summary.py"

# (what is undone, file, anchor, replacement)
MUTANTS = [
    (
        "a process past the bound is said stopped and runs on",
        BOUND,
        "                self.kill(pid, signal.SIGKILL)\n",
        "                pass\n",
    ),
    (
        "only the processes the script started itself are watched",
        BOUND,
        "        stack.extend(children.get(pid, []))\n",
        "        pass\n",
    ),
    (
        "a process stopped is said again at each look",
        BOUND,
        "            self.seen.add(pid)\n",
        "",
    ),
    (
        "a process at the bound is stopped",
        BOUND,
        "            if held is None or held <= self.bound:\n",
        "            if held is None or held < self.bound:\n",
    ),
    (
        "the script itself is watched",
        BOUND,
        "        for pid in sorted(descendants(parents, self.root) - self.seen):\n",
        "        for pid in sorted((descendants(parents, self.root) | {self.root}) - self.seen):\n",
    ),
    (
        "every process is watched",
        BOUND,
        "        for pid in sorted(descendants(parents, self.root) - self.seen):\n",
        "        for pid in sorted(set(parents) - self.seen):\n",
    ),
    (
        "a process is stopped on the first reading alone",
        BOUND,
        "            if not descends(self.platform.parents(), pid, self.root):\n                continue\n",
        "",
    ),
    (
        "a number used again is never watched again",
        BOUND,
        "        self.seen &= set(parents)\n",
        "",
    ),
    (
        "the bound is the machine's memory",
        BOUND,
        "BOUND = min(4 * GiB, total_memory() // 4)\n",
        "BOUND = total_memory()\n",
    ),
    (
        "a watch that cannot read the processes stops without a word",
        BOUND,
        '                self.say(f"  the memory bound stopped watching: {e!r}")\n',
        "                pass\n",
    ),
    (
        "the macOS footprint is read where the resident size is",
        BOUND,
        '                "resident_size",\n                "phys_footprint",\n',
        '                "phys_footprint",\n                "resident_size",\n',
    ),
    (
        "a stop is not named after the verdict it may have decided",
        BOUND,
        "                self.decided.append(what)\n",
        "                pass\n",
    ),
    (
        "a run the bound did not touch says nothing",
        BOUND,
        '                return "memory bound: no process was stopped"\n',
        '                return ""\n',
    ),
    (
        "an indented note is read as a verdict",
        BOUND,
        '    VERDICT = re.compile(r"^(?P<what>\\S.*?): (?:KILLED|SURVIVED)\\b")\n',
        '    VERDICT = re.compile(r"^(?P<what>.*?): (?:KILLED|SURVIVED)\\b")\n',
    ),
    (
        "the run's summary leaves out what the bound stopped",
        SUMMARY,
        "                    if line.startswith(BOUND) and line != UNTOUCHED\n",
        "                    if False\n",
    ),
    (
        "a mutation script starts no bound",
        BASELINE,
        "mutation_bound.start()\n",
        "pass\n",
    ),
]

TESTS = [
    [
        sys.executable,
        "-m",
        "unittest",
        "scripts/tests/test_mutation_bound.py",
        "scripts/tests/test_mutation_baseline.py",
        "scripts/tests/test_ci_scripts.py",
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
    # Stopped from outside, the sources are still restored: SIGTERM raises
    # here, and the `finally` below runs.
    signal.signal(signal.SIGTERM, lambda *_: sys.exit(1))
    built, passed, failed = run_tests()
    print(f"baseline: {passed} passed, {failed} failed", flush=True)
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
        print(f"{what}: {verdict}", flush=True)

    print(f"{len(MUTANTS) - survivors} of {len(MUTANTS)} mutants killed")
    return 1 if survivors else 0


if __name__ == "__main__":
    sys.exit(main())

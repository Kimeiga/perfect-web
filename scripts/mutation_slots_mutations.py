#!/usr/bin/env python3
"""Mutation controls for a mutation script's mutants run in parallel slots
(`mutation_slots.py`, ADR-XXXX).

Each mutant undoes one thing the slots do: the tree never mutated (the
script's loop run in the tree, or the tree's copy of the script run in a
slot); each slot its own (ports, temporary directory, target, git, a copy
brought to the tree by content, a run's leftovers removed, `node_modules`
linked, the tree's target not copied); the ask for slots and the mutants
dealt kept from what a slot runs; the mutants dealt (each once, slot 0 a
mutant of each suite, a run given arguments in one slot) and narrowed; the
output the script's (in its order, its baseline slot 0's, each mutant's
lines its own, a verdict naming no suite the next's, the count summed, the
memory bound's line, a red baseline said as the script says it); a run
stopped from outside stopping its slots and what they started; and two runs
not sharing the slots at once.

At least one test of `scripts/tests/test_mutation_slots.py` must then fail.
Its suite runs with `PYTHONDONTWRITEBYTECODE=1`: a mutant of the same size
written in the second its bytecode was would otherwise run as the bytecode
says.

Run from the repository root; `just e14-parallel-mutants` records the
output. The source is restored after every mutant, whatever happens.
"""

import os
import pathlib
import re
import signal
import subprocess
import sys

ROOT = pathlib.Path(__file__).resolve().parent.parent
SLOTS = ROOT / "scripts/mutation_slots.py"

# (what is undone, file, anchor, replacement)
MUTANTS = [
    (
        "a run asked for slots runs the script's loop in the tree",
        SLOTS,
        "    n = asked()\n    if n <= 1:\n        return\n",
        "    n = asked()\n    if n <= 1 or True:\n        return\n",
    ),
    (
        "a slot runs the tree's copy of the script",
        SLOTS,
        "                [sys.executable, str(slot / relative), *self.argv],\n",
        "                [sys.executable, str(self.script), *self.argv],\n",
    ),
    (
        "a slot runs every mutant, not those dealt to it",
        SLOTS,
        "    mutants[:] = [mutants[i] for i in indices]\n",
        "    mutants[:] = list(mutants)\n",
    ),
    (
        "a slot's mutants are not checked against the count they were dealt from",
        SLOTS,
        "    if not total.isdigit() or int(total) != len(mutants):\n",
        "    if not total.isdigit():\n",
    ),
    (
        "no slots is a number of slots",
        SLOTS,
        "    if not raw.isdigit() or int(raw) < 1:\n",
        "    if not raw.isdigit():\n",
    ),
    (
        "the ask for slots reaches what a slot runs",
        SLOTS,
        "    env.pop(SLOTS, None)\n",
        "    pass\n",
    ),
    (
        "the mutants dealt reach what a slot runs",
        SLOTS,
        "    pick = os.environ.pop(PICK, None)\n",
        "    pick = os.environ.get(PICK)\n",
    ),
    (
        "every slot listens on the same ports",
        SLOTS,
        '            "PORT": str(port + k * STRIDE),\n',
        '            "PORT": str(port),\n',
    ),
    (
        "every slot shares one temporary directory",
        SLOTS,
        '            "TMPDIR": str(tmp),\n',
        '            "TMPDIR": env.get("TMPDIR", "/tmp"),\n',
    ),
    (
        "a slot builds in the tree's target",
        SLOTS,
        '            "CARGO_TARGET_DIR": str(slot / "target"),\n',
        '            "CARGO_TARGET_DIR": str(ROOT / "target"),\n',
    ),
    (
        "slot 0 is not dealt a mutant of each suite first",
        SLOTS,
        "        if suite(m) not in seen:\n",
        "        if suite(m) not in seen and i == 0:\n",
    ),
    (
        "a run given arguments is dealt to slots",
        SLOTS,
        "    if script.name in SEQUENTIAL or sys.argv[1:]:\n",
        "    if script.name in SEQUENTIAL:\n",
    ),
    (
        "the mutants are said in the slots' order, not the script's",
        SLOTS,
        "            for i in range(len(self.mutants)):\n",
        "            for i in [i for d in self.dealt for i in d]:\n",
    ),
    (
        "the slots do not wait for every baseline",
        SLOTS,
        "                self.lock.wait_for(lambda: all(s.said or s.skipped or s.ended for s in self.slots))\n",
        "                pass\n",
    ),
    (
        "a red baseline in one slot is not said as the script says it",
        SLOTS,
        "                red = next((s for s in self.slots if s.ended and s.code and not s.said), None)\n",
        "                red = None\n",
    ),
    (
        "a mutant's lines before its verdict are another's",
        SLOTS,
        "        self.said[i] = self.since + [raw]\n",
        "        self.said[i] = [raw]\n",
    ),
    (
        "a slot's baseline is all it said before its first verdict",
        SLOTS,
        "        cut = marks[-1] + 1 if marks else (0 if ended else len(self.head))\n",
        "        cut = len(self.head)\n",
    ),
    (
        "a verdict that names no suite is no mutant's",
        SLOTS,
        "    if pending and begins(line, pending[0][1]):\n        return 0\n",
        "    if False:\n        return 0\n",
    ),
    (
        "a longer description is taken for a shorter one",
        SLOTS,
        '    return after.startswith(":") or after in (" [", "  ")\n',
        "    return True\n",
    ),
    (
        "the count is the first slot's",
        SLOTS,
        "                if m:\n                    killed += int(m.group(1))\n",
        "                if m and s.k == 0:\n                    killed += int(m.group(1))\n",
    ),
    (
        "the memory bound's line forgets a stop",
        SLOTS,
        "            w.since += 1\n",
        "            pass\n",
    ),
    (
        "the run ends well though a slot's mutant survived",
        SLOTS,
        "            return next((s.code for s in self.slots if s.code), 0)\n",
        "            return 0\n",
    ),
    (
        "a slot is brought to the tree by whether a file is there, not by its content",
        SLOTS,
        "                    if not dp.exists() or not same(sp, dp):\n",
        "                    if not dp.exists():\n",
    ),
    (
        "what a run left in a slot is kept",
        SLOTS,
        "                if e.name not in wanted:\n                    remove(pathlib.Path(e.path))\n",
        "                if e.name not in wanted and False:\n                    remove(pathlib.Path(e.path))\n",
    ),
    (
        "node_modules is copied, not linked",
        SLOTS,
        '                if e.name == "node_modules" and e.is_dir(follow_symlinks=False):\n',
        "                if False:\n",
    ),
    (
        "the tree's target is copied into a slot",
        SLOTS,
        'def mirror(src, dst, keep=(".git", "target"), skip=()):\n',
        'def mirror(src, dst, keep=(".git",), skip=()):\n',
    ),
    (
        "a slot's git tracks nothing",
        SLOTS,
        '    git("read-tree", head, cwd=slot)\n',
        "",
    ),
    (
        "two runs share the slots at once",
        SLOTS,
        "            fcntl.flock(self.held, fcntl.LOCK_EX | fcntl.LOCK_NB)\n",
        "            pass\n",
    ),
    (
        "a run stopped from outside leaves its slots running",
        SLOTS,
        "            previous[sig] = signal.signal(sig, self.stopped_from_outside)\n",
        "            previous[sig] = signal.getsignal(sig)\n",
    ),
    (
        "a slot's script is not stopped by its signal, but waited out",
        SLOTS,
        "                    os.killpg(s.proc.pid, signal.SIGTERM)\n",
        "                    pass\n",
    ),
    (
        "what a stopped slot started in a session of its own is left running",
        SLOTS,
        "                for pid, started in left:\n                    if started_at(pid) == started:\n",
        "                for pid, started in left:\n                    if False:\n",
    ),
]

TESTS = [sys.executable, "-m", "unittest", "scripts/tests/test_mutation_slots.py"]

# How long one run of the tests may take; past it, it is killed with what it
# started, and counts as failing.
BOUND = 900


def run_tests():
    """(ran, passed, failed)."""
    env = {k: v for k, v in os.environ.items() if not k.startswith("PW_MUTANT_")}
    env["PYTHONDONTWRITEBYTECODE"] = "1"
    p = subprocess.Popen(TESTS, cwd=ROOT, env=env, start_new_session=True,
                         stdout=subprocess.PIPE, stderr=subprocess.STDOUT, text=True)
    try:
        out, _ = p.communicate(timeout=BOUND)
    except subprocess.TimeoutExpired:
        os.killpg(p.pid, signal.SIGKILL)
        p.communicate()
        return True, 0, 1
    ran = re.search(r"^Ran (\d+) tests?", out, re.M)
    if ran is None:
        return False, 0, 0
    bad = re.search(r"^FAILED \((.*)\)", out, re.M)
    n_bad = sum(int(n) for n in re.findall(r"=(\d+)", bad.group(1))) if bad else 0
    return True, int(ran.group(1)) - n_bad, n_bad


def main():
    import mutation_baseline  # keeps what each command says, for a red baseline
    # Stopped from outside, the source is still restored: SIGTERM raises
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

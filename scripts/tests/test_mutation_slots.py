"""`mutation_slots.py`: a mutation script's mutants, run in parallel slots,
say what the script says run in order, and never mutate the tree.

A toy tree holds a toy script of the scripts' shape (`MUTANTS`, a baseline
per suite in the order of its suites' names, one verdict a mutant, the
count) beside copies of `mutation_baseline.py`, `mutation_bound.py` and
`mutation_slots.py`, and is committed to a git of its own. Its suite asks
git what is tracked, and records where it ran and with what environment.
One of its mutants must survive: a comment changed (the control).
"""

import importlib.util
import json
import os
import pathlib
import re
import shutil
import signal
import subprocess
import sys
import tempfile
import textwrap
import time
import unittest

SCRIPTS = pathlib.Path(__file__).resolve().parent.parent
ROOT = SCRIPTS.parent
sys.path.insert(0, str(SCRIPTS))


def load(name: str):
    spec = importlib.util.spec_from_file_location(name, SCRIPTS / f"{name}.py")
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


slots = load("mutation_slots")

CALC = """\
def add(a, b):
    return a + b


def double(a):
    return a * 2


# what the toy computes
"""

CHECK = """\
import json, os, pathlib, subprocess, sys, time
sys.path.insert(0, "src")
import calc
seen = os.environ.get("TOY_SEEN")
if seen:
    record = {
        "cwd": os.getcwd(),
        "pid": os.getpid(),
        **{k: os.environ.get(k) for k in ("PORT", "TMPDIR", "CARGO_TARGET_DIR",
                                          "PW_MUTANT_SLOTS", "PW_MUTANT_PICK", "PW_MUTANT_SLOT")},
    }
    path = pathlib.Path(seen) / f"{os.getpid()}-{time.monotonic_ns()}.json"
    path.write_text(json.dumps(record))
hold = os.environ.get("TOY_HOLD")
if hold and "a - b" in pathlib.Path("src/calc.py").read_text():
    pathlib.Path(hold).write_text("held")
    time.sleep(60)
tracked = subprocess.run(["git", "ls-files", "src/calc.py"], capture_output=True, text=True).stdout.strip()
results = [calc.add(2, 3) == 5, calc.double(4) == 8, tracked == "src/calc.py"]
if os.environ.get("TOY_RED") and os.environ.get("PW_MUTANT_SLOT") != "0" and os.getcwd().endswith("/1"):
    results.append(False)
print(f"test result: ok. {sum(results)} passed; {len(results) - sum(results)} failed")
"""

TOY = '''\
#!/usr/bin/env python3
"""A toy mutation script, of the scripts' shape."""

import os
import pathlib
import re
import signal
import subprocess
import sys

ROOT = pathlib.Path(__file__).resolve().parent.parent
CALC = ROOT / "src/calc.py"

MUTANTS = [
    ("add subtracts", "unit", CALC, "    return a + b\\n", "    return a - b\\n"),
    ("double adds", "unit", CALC, "    return a * 2\\n", "    return a + 2\\n"),
    ("a comment is changed", "lint", CALC, "# what the toy computes\\n", "# what it computes\\n"),
    ("double triples", "unit", CALC, "    return a * 2\\n", "    return a * 3\\n"),
    ("add ignores b", "unit", CALC, "    return a + b\\n", "    return a + a\\n"),
    ("double is negated", "lint", CALC, "    return a * 2\\n", "    return -a * 2\\n"),
    ("add adds one", "unit", CALC, "    return a + b\\n", "    return a + b + 1\\n"),
]


def run_tests(_suite):
    # `-B`: no bytecode kept, which a mutant of the same size written in the
    # same second as it would not make stale.
    out = subprocess.run([sys.executable, "-B", "tests/check.py"], cwd=ROOT, capture_output=True, text=True).stdout
    found = re.findall(r"test result: \\w+\\. (\\d+) passed; (\\d+) failed", out)
    if not found:
        return False, 0, 0
    return True, sum(int(p) for p, _ in found), sum(int(f) for _, f in found)


def main():
    import mutation_baseline  # keeps what each command says, for a red baseline
    signal.signal(signal.SIGTERM, lambda *_: sys.exit(1))
    chosen = [m for m in MUTANTS if not sys.argv[1:] or any(a in m[0] for a in sys.argv[1:])]
    for suite in sorted({m[1] for m in chosen}):
        built, passed, failed = run_tests(suite)
        print(f"baseline ({suite}): {passed} passed, {failed} failed", flush=True)
        if not built or failed or not passed:
            print("FAIL: the unmutated baseline is not green; no mutant can mean anything")
            mutation_baseline.explain()
            return 1

    survivors = 0
    for what, suite, path, anchor, replacement in chosen:
        original = path.read_text()
        if original.count(anchor) != 1:
            print(f"{what}: ANCHOR NOT FOUND EXACTLY ONCE in {path.name}")
            survivors += 1
            continue
        try:
            path.write_text(original.replace(anchor, replacement, 1))
            built, passed, failed = run_tests(suite)
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

    print(f"{len(chosen) - survivors} of {len(chosen)} mutants killed")
    return 1 if survivors else 0


if __name__ == "__main__":
    sys.exit(main())
'''


def toy_tree(where: pathlib.Path) -> pathlib.Path:
    """A toy tree, committed, with the scripts' machinery beside its script."""
    root = where / "tree"
    (root / "scripts").mkdir(parents=True)
    (root / "src").mkdir()
    (root / "tests").mkdir()
    (root / "node_modules" / "pkg").mkdir(parents=True)
    (root / "node_modules" / "pkg" / "index.js").write_text("module.exports = 1;\n")
    for name in ("mutation_baseline", "mutation_bound", "mutation_slots"):
        shutil.copy(SCRIPTS / f"{name}.py", root / "scripts" / f"{name}.py")
    (root / "scripts" / "toy_mutations.py").write_text(TOY)
    (root / "src" / "calc.py").write_text(CALC)
    (root / "tests" / "check.py").write_text(CHECK)
    (root / ".gitignore").write_text("node_modules/\ntarget/\n")
    git = ["git", "-c", "user.name=toy", "-c", "user.email=toy@example.invalid"]
    subprocess.run(git + ["init", "-q"], cwd=root, check=True)
    subprocess.run(git + ["add", "-A"], cwd=root, check=True)
    subprocess.run(git + ["commit", "-q", "-m", "toy"], cwd=root, check=True)
    # An uncommitted change the slots must carry: the run is of the tree
    # as it is, not as committed.
    (root / "src" / "notes.txt").write_text("not committed\n")
    return root


def run_toy(root, slots_asked=None, args=(), extra=None, timeout=120):
    env = {k: v for k, v in os.environ.items() if not k.startswith("PW_MUTANT_")}
    env.update(extra or {})
    env["PORT"] = "5000"
    if slots_asked is not None:
        env["PW_MUTANT_SLOTS"] = str(slots_asked)
    return subprocess.run(
        [sys.executable, "scripts/toy_mutations.py", *args],
        cwd=root,
        env=env,
        capture_output=True,
        timeout=timeout,
    )


class Toy(unittest.TestCase):
    """The toy script, in order and in slots."""

    def setUp(self):
        self.dir = pathlib.Path(tempfile.mkdtemp(prefix="pw-slots-test-"))
        self.root = toy_tree(self.dir)
        self.seen = self.dir / "seen"
        self.seen.mkdir()
        self.calc = self.root / "src" / "calc.py"
        self.before = (self.calc.read_bytes(), self.calc.stat().st_mtime_ns)

    def tearDown(self):
        shutil.rmtree(self.dir, ignore_errors=True)

    def records(self):
        return [json.loads(p.read_text()) for p in sorted(self.seen.iterdir())]

    def test_slots_say_what_the_script_says_in_order(self):
        alone = run_toy(self.root)
        dealt = run_toy(self.root, 3, extra={"TOY_SEEN": str(self.seen)})
        self.assertEqual(dealt.stdout.decode(), alone.stdout.decode())
        self.assertEqual(dealt.returncode, alone.returncode)
        out = alone.stdout.decode()
        # The control survives, and the run says so, as the parsers read it.
        self.assertIn("a comment is changed [lint]: SURVIVED\n", out)
        self.assertIn("6 of 7 mutants killed\n", out)
        self.assertTrue(out.endswith("memory bound: no process was stopped\n"), out)
        self.assertEqual(alone.returncode, 1)
        verdicts = [l.split(" [")[0] for l in out.splitlines() if " [" in l]
        self.assertEqual(verdicts, [m[0] for m in TOY_MUTANTS])

    def test_the_tree_is_never_mutated_and_each_slot_is_its_own(self):
        run_toy(self.root, 3, extra={"TOY_SEEN": str(self.seen)})
        self.assertEqual((self.calc.read_bytes(), self.calc.stat().st_mtime_ns), self.before)
        records = self.records()
        self.assertGreaterEqual(len(records), 7 + 3)
        base = self.root / "target" / "mutant-slots"
        for r in records:
            slot = pathlib.Path(r["cwd"])
            self.assertEqual(slot.parent, base, r)
            k = int(slot.name)
            self.assertEqual(r["PORT"], str(5000 + 200 * k))
            self.assertEqual(r["TMPDIR"], str(base / f"tmp-{k}"))
            self.assertEqual(r["CARGO_TARGET_DIR"], str(slot / "target"))
            # Nothing the slot runs is asked to deal again.
            self.assertIsNone(r["PW_MUTANT_SLOTS"])
            self.assertIsNone(r["PW_MUTANT_PICK"])
            self.assertIsNone(r["PW_MUTANT_SLOT"])
        self.assertEqual({pathlib.Path(r["cwd"]).name for r in records}, {"0", "1", "2"})
        # The uncommitted change reached each slot; node_modules is linked.
        for k in range(3):
            self.assertEqual((base / str(k) / "src" / "notes.txt").read_text(), "not committed\n")
            self.assertTrue((base / str(k) / "node_modules").is_symlink())

    def test_one_slot_asked_for_is_the_script_as_it_was(self):
        alone = run_toy(self.root, 1)
        self.assertFalse((self.root / "target" / "mutant-slots").exists())
        self.assertIn(b"6 of 7 mutants killed\n", alone.stdout)

    def test_a_script_choosing_its_mutants_is_said_as_it_chose(self):
        alone = run_toy(self.root, args=["double"])
        # The run in order rewrote its file; the slots' leaves it.
        self.before = (self.calc.read_bytes(), self.calc.stat().st_mtime_ns)
        dealt = run_toy(self.root, 3, args=["double"])
        self.assertEqual(dealt.stdout.decode(), alone.stdout.decode())
        self.assertIn(b"3 of 3 mutants killed\n", alone.stdout)
        # In one slot: still not in the tree.
        base = self.root / "target" / "mutant-slots"
        self.assertTrue((base / "0").is_dir())
        self.assertFalse((base / "1").exists())
        self.assertEqual((self.calc.read_bytes(), self.calc.stat().st_mtime_ns), self.before)

    def test_a_slots_red_baseline_is_said_as_the_script_says_it(self):
        dealt = run_toy(self.root, 3, extra={"TOY_RED": "1"})
        out = dealt.stdout.decode()
        self.assertEqual(dealt.returncode, 1)
        self.assertIn("FAIL: the unmutated baseline is not green; no mutant can mean anything\n", out)
        self.assertNotIn("mutants killed", out)
        self.assertEqual((self.calc.read_bytes(), self.calc.stat().st_mtime_ns), self.before)

    def test_a_run_stopped_from_outside_leaves_no_mutant_and_no_process(self):
        held = self.dir / "held"
        env = {k: v for k, v in os.environ.items() if not k.startswith("PW_MUTANT_")}
        env.update({"PW_MUTANT_SLOTS": "3", "TOY_HOLD": str(held), "TOY_SEEN": str(self.seen), "PORT": "5000"})
        p = subprocess.Popen(
            [sys.executable, "scripts/toy_mutations.py"],
            cwd=self.root,
            env=env,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
        )
        deadline = time.monotonic() + 60
        while not held.exists() and time.monotonic() < deadline:
            time.sleep(0.05)
        self.assertTrue(held.exists(), "a slot holds its mutant")
        p.send_signal(signal.SIGTERM)
        p.communicate(timeout=90)
        self.assertEqual((self.calc.read_bytes(), self.calc.stat().st_mtime_ns), self.before)
        for r in self.records():
            with self.assertRaises(ProcessLookupError, msg=r):
                os.kill(r["pid"], 0)
        # The next run puts the slot back as the tree is, whatever was left.
        again = run_toy(self.root, 3)
        self.assertIn(b"6 of 7 mutants killed\n", again.stdout)


TOY_MUTANTS = [
    ("add subtracts",),
    ("double adds",),
    ("a comment is changed",),
    ("double triples",),
    ("add ignores b",),
    ("double is negated",),
    ("add adds one",),
]


class Verdicts(unittest.TestCase):
    """A mutant's verdict line, in each shape the scripts print."""

    def test_each_shape_the_scripts_print(self):
        p = pathlib.Path("x.rs")
        cases = [
            (("a mutant", p, "a", "b"), "a mutant: KILLED (3 of 9 tests fail)"),
            (("a mutant", "server", p, "a", "b"), "a mutant [server]: SURVIVED"),
            (("E9 gate 3", "a mutant", p, "a", "b"), "E9 gate 3  a mutant: KILLED (does not build)"),
            (("next-react", "a mutant", p, "a", "b"), "next-react: a mutant: SURVIVED"),
            (("a mutant", "server", p, "a", "b"), "a mutant: NOT RUN (PW_FEED_DATABASE_URL is not set)"),
            (("a mutant", p, "a", "b"), "a mutant: DID NOT BUILD, so it shows nothing"),
            (("a mutant", p, "a", "b"), "a mutant: ANCHOR NOT FOUND EXACTLY ONCE in x.rs"),
        ]
        for mutant, line in cases:
            self.assertTrue(slots.says(line, mutant), line)

    def test_a_longer_description_is_not_a_shorter_ones(self):
        p = pathlib.Path("x.rs")
        self.assertFalse(slots.says("a cart is read [server]: KILLED (1 of 2 tests fail)", ("a cart", "server", p, "a", "b")))
        self.assertFalse(slots.says("baseline (server): 3 passed, 0 failed", ("baseline", "server", p, "a", "b")))
        self.assertFalse(slots.says("  memory bound: stopped `x`, which held more than 4 GiB", ("x", p, "a", "b")))

    def test_a_slots_lines_are_each_mutants(self):
        p = pathlib.Path("x.rs")
        mutants = [(f"m{i}", "s", p, "a", "b") for i in range(5)]
        s = slots.Slot(1, [1, 2, 4], mutants)
        for line in [
            b"postgres: none named\n",
            b"baseline (s): 3 passed, 0 failed\n",
            b"  memory bound: stopped `t`, which held more than 4 GiB\n",
            b"m1 [s]: KILLED (1 of 3 tests fail)\n",
            # m2 is not run: the script chose not to.
            b"m4 [s]: SURVIVED\n",
            b"1 of 2 mutants killed\n",
            b"memory bound: no process was stopped\n",
        ]:
            s.hear(line)
        s.end(1)
        self.assertEqual(s.baseline, [b"postgres: none named\n", b"baseline (s): 3 passed, 0 failed\n"])
        self.assertEqual(
            s.said[1],
            [b"  memory bound: stopped `t`, which held more than 4 GiB\n", b"m1 [s]: KILLED (1 of 3 tests fail)\n"],
        )
        self.assertEqual(s.said[4], [b"m4 [s]: SURVIVED\n"])
        self.assertEqual(s.skipped, {2})
        self.assertEqual(s.closing, [b"1 of 2 mutants killed\n", b"memory bound: no process was stopped\n"])


class Pieces(unittest.TestCase):
    def test_dealt_once_each_slot_0_holding_every_suite(self):
        p = pathlib.Path("x")
        mutants = [(f"m{i}", s, p, "a", "b") for i, s in enumerate("sssssstsssssbsss")]
        for n in (1, 2, 3, 4, 7, 40):
            dealt = slots.deal(mutants, n)
            self.assertEqual(sorted(i for d in dealt for i in d), list(range(len(mutants))))
            self.assertLessEqual(len(dealt), min(n, len(mutants)))
            self.assertTrue(all(dealt))
            self.assertEqual({mutants[i][1] for i in dealt[0]}, {"s", "t", "b"})
            for d in dealt:
                self.assertEqual(d, sorted(d))
            sizes = [len(d) for d in dealt]
            self.assertLessEqual(max(sizes) - min(sizes), 3, sizes)
        # Without suites (four elements a mutant), one suite.
        four = [(f"m{i}", p, "a", "b") for i in range(5)]
        self.assertEqual(slots.deal(four, 2), [[0, 2, 4], [1, 3]])

    def test_narrowed_in_place_from_the_count_dealt(self):
        mutants = [("a",), ("b",), ("c",)]
        alias = mutants
        slots.narrow(mutants, "3:0,2")
        self.assertEqual(alias, [("a",), ("c",)])
        with self.assertRaises(SystemExit):
            slots.narrow([("a",)], "3:0")

    def test_slots_asked_for(self):
        self.assertEqual(slots.asked({}), 1)
        self.assertEqual(slots.asked({"PW_MUTANT_SLOTS": "4"}), 4)
        for bad in ("0", "x", "-2", "1.5"):
            with self.assertRaises(SystemExit):
                slots.asked({"PW_MUTANT_SLOTS": bad})

    def test_a_slots_environment(self):
        slot, tmp = pathlib.Path("/s/2"), pathlib.Path("/s/tmp-2")
        env = slots.env_of(2, slot, tmp, [1, 5], 9, {"PORT": "9341", "PW_MUTANT_SLOTS": "4", "KEEP": "1"})
        self.assertNotIn("PW_MUTANT_SLOTS", env)
        self.assertEqual(env["PW_MUTANT_PICK"], "9:1,5")
        self.assertEqual(env["PORT"], str(9341 + 2 * 200))
        self.assertEqual(env["CARGO_TARGET_DIR"], "/s/2/target")
        self.assertEqual(env["TMPDIR"], "/s/tmp-2")
        self.assertEqual(env["KEEP"], "1")
        self.assertEqual(slots.env_of(1, slot, tmp, [], 1, {})["PORT"], str(3141 + 200))

    def test_a_slot_is_brought_to_the_tree_by_content(self):
        with tempfile.TemporaryDirectory() as d:
            src, dst = pathlib.Path(d) / "src", pathlib.Path(d) / "dst"
            (src / "a").mkdir(parents=True)
            (src / "a" / "same.txt").write_text("same\n")
            (src / "a" / "changed.txt").write_text("new\n")
            (src / "run.sh").write_text("#!/bin/sh\n")
            os.chmod(src / "run.sh", 0o755)
            (src / "link").symlink_to("a/same.txt")
            (src / "pkg" / "node_modules").mkdir(parents=True)
            (src / ".git").mkdir()
            (src / "target").mkdir()
            (src / "target" / "big").write_text("built\n")
            slots.mirror(src, dst)
            (dst / "a" / "changed.txt").write_text("old\n")
            (dst / "a" / "gone.txt").write_text("left by a run\n")
            (dst / "target").mkdir()
            (dst / "target" / "own").write_text("the slot's build\n")
            past = time.time_ns() - 10**10
            os.utime(dst / "a" / "same.txt", ns=(past, past))
            os.utime(dst / "a" / "changed.txt", ns=(past, past))
            written, removed = slots.mirror(src, dst)
            self.assertEqual((written, removed), (1, 1))
            self.assertEqual((dst / "a" / "changed.txt").read_text(), "new\n")
            self.assertGreater((dst / "a" / "changed.txt").stat().st_mtime_ns, past)
            self.assertEqual((dst / "a" / "same.txt").stat().st_mtime_ns, past)
            self.assertFalse((dst / "a" / "gone.txt").exists())
            self.assertEqual(os.readlink(dst / "link"), "a/same.txt")
            self.assertEqual(os.readlink(dst / "pkg" / "node_modules"), str(src / "pkg" / "node_modules"))
            self.assertEqual((dst / "run.sh").stat().st_mode & 0o777, 0o755)
            self.assertFalse((dst / ".git").exists())
            self.assertEqual((dst / "target" / "own").read_text(), "the slot's build\n")
            self.assertFalse((dst / "target" / "big").exists())

    def test_the_bounds_line_counts_what_was_said(self):
        mutation_bound = load("mutation_bound")
        run = slots.Run.__new__(slots.Run)
        run.watch = mutation_bound.Watch(platform=None)
        for line in [
            "baseline (s): 3 passed, 0 failed",
            "m0 [s]: KILLED (1 of 3 tests fail)",
            "  memory bound: stopped `graphs`, which held more than 4 GiB",
            "m1 [s]: KILLED (1 of 3 tests fail)",
            "m2 [s]: SURVIVED",
        ]:
            run.account(line)
        self.assertEqual(
            run.watch.summary(),
            "memory bound: a process was stopped in 1 of 3 mutants' runs, each perhaps killed by the bound alone: m1 [s]",
        )

    def test_each_slots_ports_lie_within_its_stride(self):
        config = (ROOT / "spikes/own-renderer/playwright.config.mjs").read_text()
        offsets = [int(n) for n in re.findall(r"PORT \+ (\d+)", config)]
        self.assertTrue(offsets)
        # Three engines from the largest offset: PORT + n + 2.
        self.assertLess(max(offsets) + 2, slots.STRIDE)

    def test_the_scripts_kept_in_order_exist(self):
        for name in slots.SEQUENTIAL:
            self.assertTrue((SCRIPTS / name).exists(), name)


if __name__ == "__main__":
    unittest.main()

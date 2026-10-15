"""`mutation_baseline.py`: what made a mutation script's baseline red, said
as its commands said it.

Each test states one output it reads, or one way a command ends that is
kept; and every script that reports a red baseline says what failed. And
no bytecode a mutant left decides what a later run runs (`Bytecode`).
"""

import importlib.util
import io
import pathlib
import re
import os
import shutil
import subprocess
import sys
import tempfile
import threading
import time
import unittest

SCRIPTS = pathlib.Path(__file__).resolve().parent.parent
# As a script run from `scripts/` finds what it imports beside it.
sys.path.insert(0, str(SCRIPTS))


def load(name: str):
    spec = importlib.util.spec_from_file_location(name, SCRIPTS / f"{name}.py")
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


baseline = load("mutation_baseline")

PANIC = (
    "running 3 tests\n"
    "test a_burst_tells_once ... ok\n"
    "test the_committed_components_are_current ... FAILED\n\n"
    "failures:\n\n---- the_committed_components_are_current stdout ----\n"
    "thread 'the_committed_components_are_current' (5) panicked at "
    "compiler/pw-core/tests/evidence_is_current.rs:207:9:\n"
    "docs/evidence/E10/store.page.add_to_cart.wasm is stale\n"
    "test result: FAILED. 2 passed; 1 failed\n"
)


def python(code):
    """A command that runs `code` in a Python of its own."""
    return [sys.executable, "-c", code]


def explained():
    """What `explain()` prints, over the commands kept."""
    out = io.StringIO()
    baseline.explain(out)
    return out.getvalue()


class Failures(unittest.TestCase):
    def test_a_failing_test_and_its_panic(self):
        said = baseline.failures(PANIC)
        self.assertIn("failed: the_committed_components_are_current", said)
        self.assertIn(
            "  the_committed_components_are_current at "
            "compiler/pw-core/tests/evidence_is_current.rs:207:9: "
            "docs/evidence/E10/store.page.add_to_cart.wasm is stale",
            said,
        )
        # The control: a passing run says nothing.
        self.assertEqual(
            baseline.failures("test a ... ok\ntest result: ok. 1 passed; 0 failed\n"), []
        )

    def test_a_build_that_failed(self):
        out = (
            "error[E0425]: cannot find value `told` in this scope\n"
            "  --> spikes/own-renderer/server/src/main.rs:2192:21\n"
            "error: could not compile `pw-dev-server`\n"
        )
        self.assertEqual(
            baseline.failures(out)[:2],
            [
                "build: error[E0425]: cannot find value `told` in this scope "
                "--> spikes/own-renderer/server/src/main.rs:2192:21",
                "build: error: could not compile `pw-dev-server`",
            ],
        )

    def test_a_browser_suites_failure_read_through_its_escapes(self):
        out = (
            "\x1b[1A\x1b[2K  1) [webkit] › e2e/feed.spec.mjs:379:1 › Load more shows the next page \n"
            "    Error: \x1b[2mexpect(\x1b[22mreceived).toBeGreaterThan(expected)\n"
            "\x1b[1A\x1b[2K  1 failed\n"
        )
        self.assertEqual(
            baseline.failures(out),
            [
                "failed: [webkit] › e2e/feed.spec.mjs:379:1 › Load more shows the next page "
                "Error: expect(received).toBeGreaterThan(expected)"
            ],
        )

    def test_a_failing_tests_records_are_shown_as_it_printed_them(self):
        out = (
            "[chromium] › e2e/feed.spec.mjs:663:1 › a follow reaches another reader\n"
            "\x1b[2mpw-record (the reader told) {\"trail\":[\"12 ms stream opened since 3\"]}\x1b[22m\n"
            "  1) [chromium] › e2e/feed.spec.mjs:663:1 › a follow reaches another reader \n"
            "    Error: expect(locator).toHaveText(expected) failed\n"
            "  1 failed\n"
        )
        self.assertEqual(
            baseline.records(out),
            ['pw-record (the reader told) {"trail":["12 ms stream opened since 3"]}'],
        )
        # The last four, each cut to its bound.
        many = "".join(f"pw-record {i} {'x' * baseline.RECORD}\n" for i in range(6))
        found = baseline.records(many)
        self.assertEqual([l.split()[1] for l in found], ["2", "3", "4", "5"])
        self.assertTrue(all(l.endswith(" (cut)") for l in found))
        # The control: output with none shows none.
        self.assertEqual(baseline.records(PANIC), [])

    def test_a_failed_commands_output_with_no_result_is_shown_as_it_ended(self):
        said = baseline.failures("compiling\nsomething went wrong\n")
        self.assertEqual(said[0], "no test result; its output ended:")
        self.assertIn("  something went wrong", said)
        # The control: a command that ended well and said no result, a build,
        # failed nothing.
        self.assertEqual(baseline.failures("compiling\nbuilt\n", failed=False), [])


class Heard(unittest.TestCase):
    def setUp(self):
        baseline.heard.clear()

    def test_what_a_command_said_is_kept_with_how_it_ended(self):
        cmd = python("import sys; print('test a ... FAILED'); sys.exit(101)")
        subprocess.run(cmd, capture_output=True, text=True)
        self.assertEqual(list(baseline.heard), [[cmd, 101, "test a ... FAILED\n"]])
        # In bytes, and from its error stream, the same.
        cmd = python("import sys; sys.stderr.write('error: no\\n'); sys.exit(2)")
        subprocess.run(cmd, capture_output=True)
        self.assertEqual(baseline.heard[-1], [cmd, 2, "error: no\n"])

    def test_a_command_stopped_past_its_bound_keeps_what_it_said_by_then(self):
        cmd = python("import time; print('running 1 test', flush=True); time.sleep(30)")
        with self.assertRaises(subprocess.TimeoutExpired):
            subprocess.run(cmd, capture_output=True, text=True, timeout=2)
        self.assertEqual(baseline.heard[-1], [cmd, None, "running 1 test\n"])
        # Asked again after it was killed, as a script that bounds its own
        # commands does, its later word stands: one command, kept once.
        p = subprocess.Popen(cmd, stdout=subprocess.PIPE, text=True)
        with self.assertRaises(subprocess.TimeoutExpired):
            p.communicate(timeout=2)
        p.kill()
        p.communicate()
        self.assertEqual(baseline.heard[-1], [cmd, -9, "running 1 test\n"])
        self.assertEqual(len(baseline.heard), 2)

    def test_a_command_not_captured_keeps_how_it_ended(self):
        cmd = python("import sys; sys.exit(3)")
        subprocess.run(cmd)
        self.assertEqual(baseline.heard[-1], [cmd, 3, ""])
        self.assertIn("(exit 3):\n    (what it said was not captured)", explained())

    def test_explain_names_what_failed_as_it_failed(self):
        failing = python(f"import sys; sys.stdout.write({PANIC!r}); sys.exit(101)")
        passing = python("print('test result: ok. 4 passed; 0 failed')")
        subprocess.run(passing, capture_output=True, text=True)
        subprocess.run(failing, capture_output=True, text=True)
        said = explained()
        self.assertIn("(exit 101):\n    failed: the_committed_components_are_current\n", said)
        self.assertIn("docs/evidence/E10/store.page.add_to_cart.wasm is stale", said)
        # The control: the command that passed is not named.
        self.assertNotIn("4 passed", said)

    def test_explain_shows_a_failing_tests_records(self):
        baseline.heard.clear()
        code = (
            "import sys; print('pw-record (the reader told) {\"trail\":[]}'); "
            "print('  1) [chromium] › e2e/feed.spec.mjs:663:1 › a follow'); "
            "print('    Error: expect(locator).toHaveText(expected) failed'); sys.exit(1)"
        )
        subprocess.run(python(code), capture_output=True, text=True)
        said = explained()
        self.assertIn('    pw-record (the reader told) {"trail":[]}', said)
        # The control: a command that passed shows none of its records.
        baseline.heard.clear()
        # (Its command names the marker only in parts, so only a record
        # shown would show it.)
        subprocess.run(python("print('pw' + '-record passed {}')"), capture_output=True, text=True)
        self.assertNotIn("pw-record", explained())

    def test_explain_runs_nothing_again(self):
        subprocess.run(python("import sys; sys.exit(1)"), capture_output=True)
        before = list(baseline.heard)
        real = subprocess.Popen.__init__

        def refused(*args, **kwargs):
            raise AssertionError("explain ran a command")

        subprocess.Popen.__init__ = refused
        try:
            explained()
        finally:
            subprocess.Popen.__init__ = real
        self.assertEqual(list(baseline.heard), before)

    def test_commands_that_ended_well_say_what_each_counted(self):
        # A baseline is red with every command ending well where no test
        # passed: a filter that matched none.
        cmd = python("print('test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 9 filtered out')")
        subprocess.run(cmd, capture_output=True, text=True)
        said = explained()
        self.assertIn("each command ended well; what each counted:", said)
        self.assertIn("test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 9 filtered out", said)

    def test_only_the_last_commands_are_kept(self):
        for _ in range(baseline.KEPT + 3):
            subprocess.run(python("pass"))
        self.assertEqual(len(baseline.heard), baseline.KEPT)


class Bounded(unittest.TestCase):
    def test_imported_it_bounds_every_process_the_script_starts(self):
        watching = [t for t in threading.enumerate() if t.name == "mutation-bound" and t.is_alive()]
        self.assertEqual(len(watching), 1)
        # Its own: the bound `mutation_baseline` started, of this process.
        watch = watching[0].watch
        self.assertIsInstance(watch, baseline.mutation_bound.Watch)
        self.assertEqual(watch.root, os.getpid())
        self.assertEqual(watch.bound, baseline.mutation_bound.BOUND)


# A toy tree's mutation script, shaped as each real one is: its `MUTANTS`,
# then `main()`, which imports `mutation_baseline` first where it is guarded,
# and runs a suite in a Python of its own, before its mutant, under it and
# after its source is restored. Its one mutant is its anchor's size, and the
# source is restored within the second the mutant was written in, as a suite
# that ends within that second leaves it: the seconds are set, not waited
# for. The script's own process imports a source of the tree too (`own`).
TOY_SCRIPT = '''\
import os
import pathlib
import subprocess
import sys

ROOT = pathlib.Path(__file__).resolve().parent
TOY = ROOT / "toy.py"
MUTANTS = [("the value is two", TOY, "VALUE = 1\\n", "VALUE = 2\\n")]


def seen():
    """The toy's value, as a suite run now imports it."""
    return subprocess.run(
        [sys.executable, "-c", "import toy; print(toy.VALUE)"],
        cwd=ROOT, capture_output=True, text=True, check=True,
    ).stdout.strip()


def written(text, at):
    TOY.write_text(text)
    os.utime(TOY, (at, at))


def main():
    if os.environ.get("TOY_GUARDED"):
        sys.path.insert(0, os.environ["TOY_SCRIPTS"])
        import mutation_baseline  # noqa: F401
    import own  # noqa: F401
    second = int(sys.argv[1])
    what, path, anchor, replacement = MUTANTS[0]
    original = TOY.read_text()
    said = [seen()]
    written(original.replace(anchor, replacement), second + 0.1)
    said.append(seen())
    written(original, second + 0.9)
    said.append(seen())
    print("seen:", " ".join(said), flush=True)


main()
'''


class Bytecode(unittest.TestCase):
    """**No bytecode a mutant left decides what a later run runs** (track
    evidence-integrity, finding 1). Python takes a cached `.pyc` for its
    source where the two agree in the source's size and its modification
    time in whole seconds (`importlib._bootstrap_external`'s `get_code`:
    `int(st['mtime'])`, `_validate_timestamp_pyc`), and reads one whatever
    `dont_write_bytecode` says: that setting stops only the writing. A
    mutant of a Python source the same size as its anchor, restored within
    the second it was written in, leaves its bytecode taken for the
    restored source, by the next run, another mutant's or `just ci`'s.

    Each run here is a toy tree's script (`TOY_SCRIPT`) started as a
    script is, with no setting of the interpreter's that keeps it from
    writing bytecode; it prints what its suite saw before its mutant, under
    it, and after its source was restored."""

    def toy(self) -> pathlib.Path:
        root = pathlib.Path(tempfile.mkdtemp(prefix="pw-bytecode-"))
        self.addCleanup(shutil.rmtree, root, True)
        (root / "toy.py").write_text("VALUE = 1\n")
        (root / "own.py").write_text("OWN = 1\n")
        (root / "toy_mutations.py").write_text(TOY_SCRIPT)
        return root

    def run_toy(self, root: pathlib.Path, guarded: bool) -> str:
        env = {
            k: v
            for k, v in os.environ.items()
            if k not in ("PYTHONDONTWRITEBYTECODE", "PYTHONPYCACHEPREFIX")
        }
        if guarded:
            env.update(TOY_GUARDED="1", TOY_SCRIPTS=str(SCRIPTS))
        # A second well before the toy's sources were written, so the
        # unmutated source's own second is another.
        second = int(time.time()) - 100
        out = subprocess.run(
            [sys.executable, str(root / "toy_mutations.py"), str(second)],
            cwd=root,
            env=env,
            capture_output=True,
            text=True,
            timeout=60,
        )
        self.assertEqual(out.returncode, 0, out.stdout + out.stderr)
        return next(l for l in out.stdout.splitlines() if l.startswith("seen: "))[len("seen: "):]

    def cached(self, root: pathlib.Path, name: str) -> list[str]:
        return sorted(p.name for p in (root / "__pycache__").glob(f"{name}.*.pyc"))

    def test_unguarded_the_restored_source_runs_its_mutants_bytecode(self):
        # The hazard, on this Python: the control for the two tests below.
        root = self.toy()
        self.assertEqual(self.run_toy(root, guarded=False), "1 2 2")
        # Its script's own process writes bytecode too, unguarded.
        self.assertNotEqual(self.cached(root, "own"), [])

    def test_guarded_no_bytecode_is_written_so_the_restored_source_runs(self):
        root = self.toy()
        self.assertEqual(self.run_toy(root, guarded=True), "1 2 1")
        # Neither its suite's processes nor its own wrote any.
        self.assertEqual(self.cached(root, "toy"), [])
        self.assertEqual(self.cached(root, "own"), [])

    def test_guarded_bytecode_left_before_for_a_mutated_source_is_not_read(self):
        # An unguarded run leaves the mutant's bytecode, taken for the
        # restored source; a guarded run after it reads its source.
        root = self.toy()
        self.assertEqual(self.run_toy(root, guarded=False), "1 2 2")
        self.assertNotEqual(self.cached(root, "toy"), [])
        self.assertEqual(self.run_toy(root, guarded=True), "1 2 1")
        self.assertEqual(self.cached(root, "toy"), [])


class EveryScript(unittest.TestCase):
    RED = re.compile(
        r'^(\s*)print\("FAIL: the unmutated baseline is not green; no (mutant|control) can '
        r'mean anything"\)\n(?!\1mutation_baseline\.explain\(\)\n)',
        re.M,
    )

    def scripts(self):
        return [
            p
            for p in sorted(SCRIPTS.glob("*_mutations.py"))
            if "the unmutated baseline is not green" in p.read_text()
        ]

    def test_every_red_baseline_says_what_failed(self):
        silent = [p.name for p in self.scripts() if self.RED.search(p.read_text())]
        self.assertEqual(silent, [])

    def test_every_script_keeps_what_its_commands_say_from_its_start(self):
        # First in `main()`, before the baseline runs; not at the top, so a
        # script loaded to read its mutants needs nothing beside it.
        first = re.compile(r"^def main\(\):\n    import mutation_baseline  #", re.M)
        unheard = [
            p.name
            for p in self.scripts()
            if not first.search(p.read_text())
            or re.search(r"^(import|from) mutation_baseline", p.read_text(), re.M)
        ]
        self.assertEqual(unheard, [])
        self.assertGreater(len(self.scripts()), 200)


if __name__ == "__main__":
    unittest.main()

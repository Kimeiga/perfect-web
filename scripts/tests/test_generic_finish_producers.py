"""Query/discard controls credit native assertions, never invalid producers."""
import contextlib
import importlib.util
import io
from pathlib import Path
import sys
import tempfile
import types
import unittest
from unittest.mock import patch

SCRIPTS = Path(__file__).resolve().parent.parent
sys.path.insert(0, str(SCRIPTS))


def load(name):
    spec = importlib.util.spec_from_file_location(name, SCRIPTS / (name + ".py"))
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


QUERY = load("query_values_mutations")
DISCARD = load("let_discard_mutations")
OK = "test result: ok. 2 passed; 0 failed; 0 ignored\n"
FAIL = "test witness ... FAILED\ntest result: FAILED. 1 passed; 1 failed; 0 ignored\n"


class GenericFinishProducers(unittest.TestCase):
    def outcome(self, subject, status, output):
        result = types.SimpleNamespace(returncode=status, stdout=output, stderr="")
        with patch.object(subject.subprocess, "run", return_value=result):
            if subject is QUERY:
                return subject.run("typer")
            with patch.object(subject, "TESTS", [["cargo", "test"]]):
                return subject.run_tests()

    def test_native_status_and_result_must_agree(self):
        for subject in (QUERY, DISCARD):
            for status, output in [(-9, FAIL), (137, FAIL), (67, FAIL), (1, FAIL),
                                   (0, FAIL), (101, OK), (101, FAIL + "could not compile target"),
                                   (101, FAIL + "\nsignal: 9, SIGKILL: kill"),
                                   (0, "test result: ok. 0 passed; 0 failed; 0 ignored\n"),
                                   (0, "")]:
                with self.subTest(subject=subject.__name__, status=status, output=output):
                    self.assertEqual(self.outcome(subject, status, output), (False, 0, 0))
            self.assertEqual(self.outcome(subject, 0, OK), (True, 2, 0))
            self.assertEqual(self.outcome(subject, 101, FAIL), (True, 1, 1))

    def test_a_later_invalid_command_does_not_borrow_an_earlier_assertion(self):
        good = types.SimpleNamespace(returncode=101, stdout=FAIL, stderr="")
        bad = types.SimpleNamespace(returncode=137, stdout=FAIL, stderr="")
        with patch.object(DISCARD, "TESTS", [["cargo", "one"], ["cargo", "two"]]), \
             patch.object(DISCARD.subprocess, "run", side_effect=[good, bad]):
            self.assertEqual(DISCARD.run_tests(), (False, 0, 0))

    def campaign(self, subject, outcome):
        with tempfile.TemporaryDirectory(prefix="pw-generic-producer-") as directory:
            path = Path(directory) / "source.rs"
            original = "original anchor\n"
            path.write_text(original)
            if subject is QUERY:
                controls = [("private witness", "typer", path, "original", "changed")]
                method = "run"
                commands = patch.object(subject, "CARGO", {"typer": ["cargo", "test"]})
            else:
                controls = [("private witness", path, "original", "changed")]
                method = "run_tests"
                commands = patch.object(subject, "TESTS", [["cargo", "test"]])
            calls = []
            def produce(*args):
                calls.append(path.read_text())
                if len(calls) == 1:
                    return True, 2, 0
                self.assertEqual(path.read_text(), "changed anchor\n")
                if isinstance(outcome, Exception):
                    raise outcome
                return outcome
            output = io.StringIO()
            try:
                with patch.dict(sys.modules, {"mutation_baseline": types.SimpleNamespace(explain=lambda: None)}), \
                     patch.object(subject, "MUTANTS", controls * 2), commands, \
                     patch.object(subject, method, side_effect=produce), contextlib.redirect_stdout(output):
                    status = subject.main()
            finally:
                self.assertEqual(path.read_text(), original)
            return status, len(calls), output.getvalue()

    def test_invalid_mutant_restores_and_refuses_without_a_kill(self):
        for subject in (QUERY, DISCARD):
            with self.subTest(subject=subject.__name__):
                status, calls, output = self.campaign(subject, (False, 0, 0))
                self.assertEqual((status, calls), (1, 2))
                self.assertIn("REFUSED", output)
                self.assertNotIn("KILLED", output)

    def test_assertions_are_kills_and_survivors_remain_failures(self):
        for subject in (QUERY, DISCARD):
            for outcome, expected, verdict in [((True, 1, 1), 0, "KILLED"), ((True, 2, 0), 1, "SURVIVED")]:
                with self.subTest(subject=subject.__name__, outcome=outcome):
                    status, calls, output = self.campaign(subject, outcome)
                    self.assertEqual((status, calls), (expected, 3))
                    self.assertEqual(output.count(": " + verdict), 2)

    def test_producer_exception_preserves_original_source(self):
        for subject in (QUERY, DISCARD):
            with self.subTest(subject=subject.__name__), self.assertRaisesRegex(RuntimeError, "private error"):
                self.campaign(subject, RuntimeError("private error"))


if __name__ == "__main__":
    unittest.main()

"""A bounded mutation earns a kill only from actual Rust test failures."""
import contextlib
import importlib.util
import io
from pathlib import Path
import subprocess
import sys
import tempfile
import types
import unittest
from unittest.mock import patch

SCRIPTS = Path(__file__).resolve().parent.parent
spec = importlib.util.spec_from_file_location("bounded_result_subject", SCRIPTS / "bounded_checker_mutations.py")
bounded = importlib.util.module_from_spec(spec)
spec.loader.exec_module(bounded)


def result(output, status=0):
    return subprocess.CompletedProcess(["cargo", "test"], status, output, "")


class RustResults(unittest.TestCase):
    def run_results(self, *results):
        commands = [["cargo", "test"] for _ in results]
        with patch.object(bounded, "TESTS", commands), patch.object(bounded.subprocess, "run", side_effect=results):
            return bounded.run()

    def test_every_binary_in_one_command_contributes_its_result(self):
        self.assertEqual(self.run_results(result(
            "test result: ok. 32 passed; 0 failed; 0 ignored\n"
            "test result: FAILED. 4 passed; 1 failed; 0 ignored\n", 101)), (True, 36, 1))

    def test_every_command_contributes_its_result(self):
        self.assertEqual(self.run_results(
            result("test result: ok. 3 passed; 0 failed; 0 ignored\n"),
            result("test result: FAILED. 1 passed; 2 failed; 0 ignored\n", 101)), (True, 4, 2))

    def test_a_missing_result_is_no_assertion_failure(self):
        self.assertEqual(self.run_results(result("host disconnected\n", 1)), (False, 0, 0))

    def test_a_successful_summary_cannot_hide_a_failed_command(self):
        self.assertEqual(self.run_results(result(
            "test result: ok. 3 passed; 0 failed; 0 ignored\n", 101)), (False, 0, 0))

    def test_a_stopped_process_cannot_earn_a_kill(self):
        self.assertEqual(self.run_results(result(
            "test result: FAILED. 0 passed; 1 failed; 0 ignored\n", -9)), (False, 0, 0))

    def test_a_later_build_failure_cannot_hide_behind_a_test_result(self):
        self.assertEqual(self.run_results(result(
            "test result: ok. 3 passed; 0 failed; 0 ignored\n"
            "error[E0308]: mismatched types\ncould not compile second target\n", 101)), (False, 0, 0))

    def main_result(self, outcome):
        with tempfile.TemporaryDirectory(prefix="pw-bounded-results-") as temporary:
            path = Path(temporary) / "source.rs"
            path.write_text("original\n")
            controls = [("a real assertion control", path, "original\n", "mutated\n")]
            output = io.StringIO()
            with patch.object(bounded, "MUTANTS", controls), patch.object(
                bounded, "run", side_effect=[(True, 1, 0), outcome]
            ), patch.dict(sys.modules, {"mutation_baseline": types.SimpleNamespace()}), contextlib.redirect_stdout(output):
                status = bounded.main()
            self.assertEqual(path.read_text(), "original\n")
            return status, output.getvalue()

    def test_an_invalid_build_is_refused_and_fails_the_campaign(self):
        status, output = self.main_result((False, 0, 0))
        self.assertEqual(status, 1, output)
        self.assertIn("REFUSED", output)
        self.assertIn("0 of 1 mutants killed", output)
        self.assertNotIn("KILLED (does not build)", output)

    def test_an_actual_assertion_failure_earns_a_kill_and_restores_source(self):
        status, output = self.main_result((True, 0, 1))
        self.assertEqual(status, 0, output)
        self.assertIn("KILLED (1 of 1 tests fail)", output)
        self.assertIn("1 of 1 mutants killed", output)


if __name__ == "__main__":
    unittest.main()

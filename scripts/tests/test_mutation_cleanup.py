#!/usr/bin/env python3
"""Actual mutation entry points must refuse a failed restored browser run."""
import contextlib
import importlib.util
import io
import pathlib
import subprocess
import sys
import tempfile
import types
import unittest
from unittest import mock

ROOT = pathlib.Path(__file__).resolve().parents[2]


def load_script(name):
    path = ROOT / "scripts" / f"{name}_mutations.py"
    spec = importlib.util.spec_from_file_location(name, path)
    module = importlib.util.module_from_spec(spec)
    exec(compile(path.read_bytes(), str(path), "exec"), module.__dict__)
    return module


class RestoredBrowserGate(unittest.TestCase):
    def run_cleanup(self, script, outcome):
        module = load_script(script)
        with tempfile.TemporaryDirectory(prefix="pw-restored-browser-") as directory:
            source = pathlib.Path(directory) / "owned-source"
            original = "one original anchor\n"
            source.write_text(original)
            producer = mock.Mock(side_effect=[(True, 1, 0), (True, 0, 1)])
            baseline = types.SimpleNamespace(explain=mock.Mock(), mutation_bound=types.SimpleNamespace(start=lambda: None))
            with mock.patch.object(module, "MUTANTS", [("stand-in", "probe", source, "original", "mutated")]), \
                 mock.patch.object(module, "SUITES", {"probe": producer}), \
                 mock.patch.object(module, "browser_tests", return_value=outcome) as cleanup, \
                 mock.patch.object(module.signal, "signal"), \
                 mock.patch.dict(sys.modules, {"mutation_baseline": baseline}), \
                 contextlib.redirect_stdout(io.StringIO()):
                code = module.main()
            self.assertEqual(source.read_text(), original, "the real finally must restore source")
            self.assertEqual(producer.call_count, 2)
            cleanup.assert_called_once_with()
            return code

    def browser_command(self, script, output, status):
        module = load_script(script)
        completed = subprocess.CompletedProcess([], 0, "", "")
        selections = [f"[chromium] › {spec[0]}:1:1 › selected" for spec in module.SPECS]
        def produce(command, **_):
            if "--list" in command:
                if "--test-list" in command:
                    return "\n".join(selections), 0
                return f"[chromium] › {command[4]}:1:1 › selected", 0
            return output, status
        with mock.patch.object(module.subprocess, "run", return_value=completed), \
             mock.patch.object(module, "bounded", side_effect=produce) as command:
            result = module.browser_tests()
        return result, command.call_args_list

    def test_refusal_batch_refuses_changed_selected_multiset(self):
        module = load_script("refusal")
        completed = subprocess.CompletedProcess([], 0, "", "")
        def produce(command, **_):
            if "--test-list" in command:
                return "[chromium] › missing.spec.mjs:1:1 › changed", 0
            return f"[chromium] › {command[4]}:1:1 › selected", 0
        with mock.patch.object(module.subprocess, "run", return_value=completed), \
             mock.patch.object(module, "bounded", side_effect=produce) as command, \
             contextlib.redirect_stdout(io.StringIO()):
            self.assertFalse(module.browser_tests()[0])
        self.assertTrue(all("--list" in call.args[0] for call in command.call_args_list))

    def test_refusal_batch_refuses_failed_inventory_producer(self):
        module = load_script("refusal")
        completed = subprocess.CompletedProcess([], 0, "", "")
        with mock.patch.object(module.subprocess, "run", return_value=completed), \
             mock.patch.object(module, "bounded", return_value=("[chromium] › identity.spec.mjs:1:1 › selected", 67)) as command:
            self.assertFalse(module.browser_tests()[0])
        self.assertEqual(command.call_count, 1)

    def run_mutant(self, script, outcome, memory_stop=False, what="stand-in"):
        module = load_script(script)
        outcome = outcome(module) if callable(outcome) else outcome
        watch = types.SimpleNamespace(stopped=[])
        baseline = types.SimpleNamespace(explain=mock.Mock(), mutation_bound=types.SimpleNamespace(start=lambda: watch))
        with tempfile.TemporaryDirectory(prefix="pw-mutant-outcome-") as directory:
            source = pathlib.Path(directory) / "owned-source"
            original = "one original anchor\n"
            source.write_text(original)
            def producer(_):
                if source.read_text() == original:
                    return True, 1, 0
                if memory_stop:
                    watch.stopped.append(("stand-in", 1))
                return outcome
            output = io.StringIO()
            with mock.patch.object(module, "MUTANTS", [(what, "probe", source, "original", "mutated")]), \
                 mock.patch.object(module, "SUITES", {"probe": producer}), \
                 mock.patch.object(module, "browser_tests", return_value=(True, 1, 0)), \
                 mock.patch.object(module.signal, "signal"), \
                 mock.patch.dict(sys.modules, {"mutation_baseline": baseline}), \
                 contextlib.redirect_stdout(output):
                code = module.main()
            self.assertEqual(source.read_text(), original, "invalid outcomes must still restore source in finally")
            return code, output.getvalue()

    def test_refusal_named_PW5048_remains_an_explicit_checker_kill(self):
        code, output = self.run_mutant("refusal", lambda m: m.CompilerRefusal({"PW5048"}), what="the feed's composer is shown to a reader signed out")
        self.assertEqual(code, 0)
        self.assertIn("KILLED (named checker refusal: PW5048)", output)

    def test_refusal_named_checker_exception_refuses_additional_codes(self):
        code, output = self.run_mutant("refusal", lambda m: m.CompilerRefusal({"PW5048", "PW6601"}), what="the feed's composer is shown to a reader signed out")
        self.assertNotEqual(code, 0)
        self.assertNotIn("KILLED", output)

    def test_refusal_named_code_has_no_exception_for_an_unlisted_mutant(self):
        code, output = self.run_mutant("refusal", lambda m: m.CompilerRefusal({"PW5048"}))
        self.assertNotEqual(code, 0)
        self.assertNotIn("KILLED", output)

    def test_refusal_failed_feed_check_retains_actual_diagnostic_codes(self):
        module = load_script("refusal")
        success = subprocess.CompletedProcess([], 0, "", "")
        refusal = subprocess.CompletedProcess([], 1, "", "error: [PW5048] refused\n")
        with mock.patch.object(module.subprocess, "run", side_effect=[success, refusal]):
            result = module.browser_tests()
        self.assertIsInstance(result, module.CompilerRefusal)
        self.assertEqual(result.codes, {"PW5048"})

    def test_refusal_wrong_producer_status_cannot_mint_named_refusal(self):
        module = load_script("refusal")
        success = subprocess.CompletedProcess([], 0, "", "")
        invalid = subprocess.CompletedProcess([], 67, "", "error: [PW5048] refused\n")
        with mock.patch.object(module.subprocess, "run", side_effect=[success, invalid]):
            result = module.browser_tests()
        self.assertNotIsInstance(result, module.CompilerRefusal)
        self.assertFalse(result[0])


def add_tests(script):
    def cleanup_build_failure(self):
        self.assertNotEqual(self.run_cleanup(script, (False, 1, 0)), 0)
    def cleanup_assertion_failure(self):
        self.assertNotEqual(self.run_cleanup(script, (True, 0, 1)), 0)
    def cleanup_zero_tests(self):
        self.assertNotEqual(self.run_cleanup(script, (True, 0, 0)), 0)
    def cleanup_green(self):
        self.assertEqual(self.run_cleanup(script, (True, 1, 0)), 0)
    def actual_command_uses_shared_authority(self):
        result, calls = self.browser_command(script, "1 passed (1s)\n", 0)
        self.assertTrue(result[0])
        self.assertTrue(calls)
        prefix = ["bash", str(ROOT / "scripts/run_suite.sh"), "-C", ".", "--", "pnpm", "exec", "playwright", "test"]
        actual = [call for call in calls if "--list" not in call.args[0]]
        self.assertEqual(len(actual), 1)
        for call in actual:
            self.assertEqual(call.args[0][:len(prefix)], prefix)
            self.assertEqual(call.kwargs["cwd"], ROOT / "spikes/own-renderer")
    def producer_failure_cannot_be_passed_counts(self):
        result, _ = self.browser_command(script, "1 passed (1s)\n", 67)
        built, passed, failed = result
        self.assertFalse(built and passed and not failed, "producer exit67 was swallowed by passed counts")
    for name, test in locals().copy().items():
        if callable(test) and name != "add_tests":
            setattr(RestoredBrowserGate, f"test_{script}_{name}", test)


def add_outcome_tests(script):
    def main_rejects(outcome):
        def test(self):
            code, output = self.run_mutant(script, outcome)
            self.assertNotEqual(code, 0)
            self.assertIn("REFUSED", output)
            self.assertNotIn("KILLED", output, "invalid producer state earned a mutant kill")
        return test
    for name, outcome in {
        "build_failure": (False, 0, 0),
        "zero_tests": (True, 0, 0),
        "failed_producer_with_passed_counts": (False, 1, 0),
        "stopped_producer_with_failed_counts": (False, 0, 1),
    }.items():
        setattr(RestoredBrowserGate, f"test_{script}_main_refuses_{name}", main_rejects(outcome))
    def main_accepts_assertion_failure(self):
        code, output = self.run_mutant(script, (True, 1, 1))
        self.assertEqual(code, 0)
        self.assertIn("KILLED (1 of 2 tests fail)", output)
    def main_refuses_memory_stop(self):
        code, output = self.run_mutant(script, (True, 1, 1), memory_stop=True)
        self.assertNotEqual(code, 0)
        self.assertIn("REFUSED", output)
        self.assertNotIn("KILLED", output)
    setattr(RestoredBrowserGate, f"test_{script}_main_valid_assertion_failure", main_accepts_assertion_failure)
    setattr(RestoredBrowserGate, f"test_{script}_main_refuses_memory_stop", main_refuses_memory_stop)
    def browser_invalid(output, status):
        def test(self):
            result, _ = self.browser_command(script, output, status)
            self.assertFalse(result[0], "invalid browser producer was classified as an assertion result")
        return test
    for name, output, status in [
        ("zero_exit_with_failed_counts", "1 passed\n1 failed\n", 0),
        ("exit67_with_failed_counts", "1 passed\n1 failed\n", 67),
        ("stop_with_failed_counts", "1 failed\n", None),
    ]:
        setattr(RestoredBrowserGate, f"test_{script}_browser_refuses_{name}", browser_invalid(output, status))
    def cargo_outcome(output, status, expected):
        def test(self):
            module = load_script(script)
            with mock.patch.object(module, "bounded", return_value=(output, status)):
                result = module.cargo_tests("core")
            self.assertEqual(result[0], expected)
        return test
    for name, passed, failed, status, valid in [
        ("exit67_with_failed_counts", 1, 1, 67, False),
        ("zero_exit_with_failed_counts", 1, 1, 0, False),
        ("zero_tests", 0, 0, 0, False),
        ("assertion_failure", 1, 1, 101, True),
    ]:
        text = f"test result: FAILED. {passed} passed; {failed} failed"
        setattr(RestoredBrowserGate, f"test_{script}_cargo_{name}", cargo_outcome(text, status, valid))


for script in ["build_id", "refusal"]:
    add_tests(script)
    add_outcome_tests(script)

if __name__ == "__main__":
    unittest.main()

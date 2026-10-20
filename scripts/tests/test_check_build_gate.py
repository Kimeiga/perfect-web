#!/usr/bin/env python3
"""Exercise the real test-compile recipe with failing corpus producers."""
import os
import pathlib
import subprocess
import tempfile
import unittest

ROOT = pathlib.Path(__file__).resolve().parents[2]
class CorpusGate(unittest.TestCase):
    def assert_failure_propagates(self, target):
        with tempfile.TemporaryDirectory(prefix="pw-corpus-gate-") as directory:
            scratch = pathlib.Path(directory)
            cargo = scratch / "cargo"
            cargo.write_text("#!/usr/bin/env python3\nimport os,sys\nwith open(os.environ['TRACE'], 'a') as f: f.write(' '.join(sys.argv[1:]) + '\\n')\nsys.exit(67 if os.environ['FAIL_TARGET'] in ' '.join(sys.argv[1:]) else 0)\n")
            cargo.chmod(0o755)
            trace = scratch / "calls"
            env = dict(os.environ, TRACE=str(trace), FAIL_TARGET=target)
            result = subprocess.run(["just", "--justfile", str(ROOT / "justfile"), "--set", "toolchain_bin", str(scratch), "test-compile"], cwd=ROOT, env=env, capture_output=True, text=True, timeout=30)
            calls = trace.read_text() if trace.exists() else ""
            self.assertIn(target, calls, result.stdout + result.stderr)
            self.assertNotEqual(result.returncode, 0, "a failed corpus producer was swallowed")
            self.assertIn("exit code 67", result.stderr)
    def test_semantic_language_gate_runs_and_propagates_its_failure(self):
        self.assert_failure_propagates("--test checking_source no_accepted_corpus_file_reports_a_body_level_error")
    def test_complete_support_gate_runs_and_propagates_its_failure(self):
        self.assert_failure_propagates("--test check_build_corpus accepted_corpus_has_exactly_its_known_backend_refusals")

class MutationProducerContract(unittest.TestCase):
    """Invalid producers cannot become mutation evidence or reach a next input."""
    def load(self, name):
        import importlib.util
        import sys
        sys.path.insert(0, str(ROOT / "scripts"))
        path = ROOT / "scripts" / (name + "_mutations.py")
        spec = importlib.util.spec_from_file_location(name, path)
        module = importlib.util.module_from_spec(spec)
        exec(compile(path.read_bytes(), str(path), "exec"), module.__dict__)
        return module

    def cargo(self, name, status, output, timeout=False):
        from unittest import mock
        module = self.load(name)
        child = mock.Mock(returncode=status, pid=1234567)
        if timeout:
            child.communicate.side_effect = [subprocess.TimeoutExpired("private probe", 1), (output, None)]
        else:
            child.communicate.return_value = (output, None)
        if name == "check_is_build":
            module.COMMANDS = [["cargo", "test", "--test", "private_probe"]]
        with mock.patch.object(module.subprocess, "Popen", return_value=child), mock.patch.object(module.os, "killpg") as stopped:
            result = module.run_tests()
        self.assertEqual(stopped.call_count, int(timeout))
        return result

    def test_cargo_invalid_status_and_build_text_never_count_as_assertions(self):
        failure = "test probe ... FAILED\n---- probe stdout ----\nassertion failed\ntest result: FAILED. 0 passed; 1 failed; 0 ignored;\n"
        for name in ("check_is_build", "held_labels"):
            for status in (0, 67, -15, None):
                with self.subTest(script=name, status=status):
                    self.assertFalse(self.cargo(name, status, failure)[0], "invalid native Cargo status earned a kill")
            with self.subTest(script=name, build=True):
                self.assertFalse(self.cargo(name, 101, "error[E0001]: build failed\ncould not compile probe\n" + failure)[0])
            self.assertEqual(self.cargo(name, 101, failure)[:3], (True, 0, 1))
            self.assertEqual(self.cargo(name, 0, "test result: ok. 1 passed; 0 failed; 0 ignored;\n")[:3], (True, 1, 0))

    def test_stopped_child_cannot_earn_a_kill_even_with_a_green_summary(self):
        for name in ("check_is_build", "held_labels"):
            with self.subTest(script=name):
                self.assertFalse(self.cargo(name, 0, "test result: ok. 1 passed; 0 failed; 0 ignored;\n", timeout=True)[0])

    @staticmethod
    def report(kind="success", origin="body", identity="probe.C.test_body"):
        import json
        failed = kind != "success"
        return json.dumps({"schema": 1, "expected": [identity], "started": [identity], "finished": [identity],
            "bodies": [{"id": identity, "entered": 1, "completed": 1}],
            "events": [{"id": identity, "kind": kind, "origin": origin}],
            "failed": [identity] if failed else [], "passed": [] if failed else [identity],
            "refusals": [], "complete": True})

    def test_python_requires_exact_complete_body_assertion_evidence(self):
        module = self.load("check_is_build")
        ids = ["probe.C.test_body"]
        for status, report in [(67, self.report("failure")), (0, self.report("failure")),
                (1, self.report("failure", "fixture")), (1, self.report("error")),
                (0, self.report(identity="foreign.C.test_body")), (0, ""), (0, "{}")] :
            with self.subTest(status=status, report=report):
                self.assertEqual(module.python_result(status, report, ids), (False, 0, 0), "non-assertion or foreign Python evidence earned a kill")
        self.assertEqual(module.python_result(0, self.report(), ids), (True, 1, 0))
        self.assertEqual(module.python_result(1, self.report("failure"), ids), (True, 0, 1))

    def test_each_python_command_uses_a_fresh_report_and_frozen_ids(self):
        from unittest import mock
        import json
        module = self.load("check_is_build")
        ids = ["probe.C.test_body"]
        paths = []
        def produced(command):
            report = pathlib.Path(command[command.index("--report") + 1])
            expected = pathlib.Path(command[command.index("--expected") + 1])
            self.assertFalse(report.exists(), "report already existed before this invocation")
            self.assertEqual(json.loads(expected.read_text()), ids)
            self.assertEqual(command[-1], "scripts.tests.test_check_build_gate")
            paths.append(report)
            report.write_text(self.report())
            return "actual body output", 0, False
        with mock.patch.object(module, "bounded", side_effect=produced):
            for _ in range(2):
                self.assertEqual(module.python_tests(["python3", "scripts/tests/test_check_build_gate.py"], ids)[:3], (True, 1, 0))
        self.assertNotEqual(paths[0], paths[1], "separate invocations reused one result path")
        self.assertTrue(all(not p.exists() for p in paths))
        with mock.patch.object(module, "bounded", return_value=("no report", 0, False)):
            self.assertFalse(module.python_tests(["python3", "scripts/tests/test_check_build_gate.py"], ids)[0])

    def main_outcome(self, name, outcome=None, error=None):
        from contextlib import ExitStack, redirect_stdout
        from io import StringIO
        from unittest import mock
        import sys
        import types
        module = self.load(name)
        with tempfile.TemporaryDirectory(prefix="pw-mocked-mutation-") as directory, ExitStack() as stack:
            first, second = [pathlib.Path(directory) / n for n in ("first", "second")]
            first.write_text("first original")
            second.write_text("second original")
            module.MUTANTS = [("first", first, "first original", "first mutant"), ("second", second, "second original", "second mutant")]
            baseline = (True, 1, 0, False, []) if name == "check_is_build" else (True, 1, 0)
            remaining = iter([baseline, error if error is not None else outcome])
            def produce(*args):
                value = next(remaining, None)
                self.assertIsNotNone(value, "a next control ran after invalid evidence")
                if isinstance(value, BaseException):
                    raise value
                return value
            results = stack.enter_context(mock.patch.object(module, "run_tests", side_effect=produce))
            stack.enter_context(mock.patch.object(module.signal, "signal"))
            stack.enter_context(mock.patch.dict(sys.modules, {"mutation_baseline": types.SimpleNamespace(explain=lambda: None, mutation_bound=types.SimpleNamespace(start=lambda: types.SimpleNamespace(stopped=[])))}))
            if name == "check_is_build":
                stack.enter_context(mock.patch.object(module, "freeze_python", return_value={}))
            output = StringIO()
            with redirect_stdout(output):
                if error is not None:
                    with self.assertRaises(type(error)) as caught:
                        module.main()
                    self.assertIs(caught.exception, error)
                    status = None
                else:
                    status = module.main()
            self.assertEqual(first.read_text(), "first original")
            self.assertEqual(second.read_text(), "second original")
            self.assertEqual(results.call_count, 2, "a next control ran after invalid evidence")
            return status, output.getvalue()

    def test_invalid_result_restores_source_and_aborts_without_credit_or_next(self):
        for name in ("check_is_build", "held_labels"):
            with self.subTest(script=name):
                result = (False, 0, 0, False, ["invalid producer"]) if name == "check_is_build" else (False, 0, 0)
                status, output = self.main_outcome(name, result)
                self.assertNotEqual(status, 0)
                self.assertNotIn("KILLED", output)
        status, output = self.main_outcome("check_is_build", (False, 1, 0, True, ["mock stopped"]))
        self.assertNotEqual(status, 0)
        self.assertNotIn("KILLED", output)

    def test_all_green_is_a_survivor_and_never_assertion_credit(self):
        from contextlib import redirect_stdout
        from io import StringIO
        from unittest import mock
        import sys
        import types
        for name in ("check_is_build", "held_labels"):
            with self.subTest(script=name), tempfile.TemporaryDirectory(prefix="pw-mocked-green-") as directory:
                module = self.load(name)
                path = pathlib.Path(directory) / "source"
                path.write_text("original")
                module.MUTANTS = [("all green", path, "original", "mutant")]
                green = (True, 1, 0, False, []) if name == "check_is_build" else (True, 1, 0)
                output = StringIO()
                with mock.patch.object(module, "run_tests", return_value=green), mock.patch.object(module.signal, "signal"), mock.patch.dict(sys.modules, {"mutation_baseline": types.SimpleNamespace(explain=lambda: None, mutation_bound=types.SimpleNamespace(start=lambda: types.SimpleNamespace(stopped=[])))}), redirect_stdout(output):
                    if name == "check_is_build":
                        with mock.patch.object(module, "freeze_python", return_value={}):
                            status = module.main()
                    else:
                        status = module.main()
                self.assertNotEqual(status, 0)
                self.assertIn("SURVIVED", output.getvalue())
                self.assertNotIn("KILLED", output.getvalue())
                self.assertEqual(path.read_text(), "original")

    def test_private_stale_bytecode_cannot_replace_either_current_mutant_source(self):
        import importlib.util
        import py_compile
        from unittest import mock
        helper_path = ROOT / "scripts/tests/test_mutation_cleanup.py"
        spec = importlib.util.spec_from_file_location("private_cleanup_helper", helper_path)
        helper = importlib.util.module_from_spec(spec)
        exec(compile(helper_path.read_bytes(), str(helper_path), "exec"), helper.__dict__)
        for name in ("consumer", "cleanup"):
            with self.subTest(loader=name), tempfile.TemporaryDirectory(prefix="pw-private-stale-pyc-") as directory:
                private = pathlib.Path(directory)
                (private / "scripts").mkdir()
                source = private / "scripts/probe_mutations.py"
                source.write_text("VALUE = 1\n")
                stamp = source.stat().st_mtime_ns
                py_compile.compile(str(source), doraise=True)
                source.write_text("VALUE = 2\n")
                os.utime(source, ns=(stamp, stamp))
                if name == "consumer":
                    with mock.patch.dict(globals(), {"ROOT": private}):
                        loaded = self.load("probe")
                else:
                    with mock.patch.object(helper, "ROOT", private):
                        loaded = helper.load_script("probe")
                self.assertEqual(loaded.VALUE, 2, "cached old source replaced the current mutant")
                self.assertEqual(source.read_text(), "VALUE = 2\n")

    def test_existing_memory_stop_flag_refuses_baseline_and_mutant_without_next(self):
        from contextlib import redirect_stdout
        from io import StringIO
        from unittest import mock
        import sys
        import types
        for baseline_stop in (True, False):
            with self.subTest(baseline_stop=baseline_stop), tempfile.TemporaryDirectory(prefix="pw-mocked-watch-") as directory:
                module = self.load("held_labels")
                watch = types.SimpleNamespace(stopped=[("synthetic", 5000000000)] if baseline_stop else [])
                start = mock.Mock(return_value=watch)
                baseline = types.SimpleNamespace(explain=lambda: None, mutation_bound=types.SimpleNamespace(start=start))
                first, second = [pathlib.Path(directory) / name for name in ("first", "second")]
                first.write_text("original")
                second.write_text("untouched")
                module.MUTANTS = [("first", first, "original", "mutant"), ("next", second, "untouched", "incorrect")]
                calls = []
                def outcome():
                    calls.append(1)
                    self.assertLessEqual(len(calls), 1 if baseline_stop else 2, "next control ran after a known memory stop")
                    if len(calls) == 1:
                        return True, 1, 0
                    watch.stopped.append(("synthetic", 5000000000))
                    return True, 0, 1
                output = StringIO()
                with mock.patch.dict(sys.modules, {"mutation_baseline": baseline}), mock.patch.object(module, "run_tests", side_effect=outcome), mock.patch.object(module.signal, "signal"), redirect_stdout(output):
                    status = module.main()
                start.assert_called_once_with()
                self.assertNotEqual(status, 0, "known memory-stop flag was credited")
                self.assertNotIn("KILLED", output.getvalue())
                self.assertEqual(len(calls), 1 if baseline_stop else 2)
                self.assertEqual(first.read_text(), "original")
                self.assertEqual(second.read_text(), "untouched")

    def test_report_cleanup_keeps_cancellation_and_exposes_ordinary_failure(self):
        from unittest import mock
        module = self.load("check_is_build")
        for primary, secondary in ((SystemExit(37), OSError("cleanup")),
                (RuntimeError("ordinary primary"), KeyboardInterrupt()), (None, OSError("visible cleanup"))):
            with self.subTest(primary=type(primary).__name__, secondary=type(secondary).__name__), tempfile.TemporaryDirectory(prefix="pw-private-report-release-") as directory:
                manager = mock.MagicMock(name=directory)
                manager.name = directory
                manager.__enter__.return_value = directory
                manager.__exit__.side_effect = secondary
                manager.cleanup.side_effect = secondary
                def outcome(command):
                    if primary is not None:
                        raise primary
                    report = pathlib.Path(command[command.index("--report") + 1])
                    report.write_text(self.report())
                    return "body", 0, False
                caught = None
                with mock.patch.object(module.tempfile, "TemporaryDirectory", return_value=manager), mock.patch.object(module, "bounded", side_effect=outcome):
                    try:
                        module.python_tests(["python3", "scripts/tests/test_run_suite.py"], ["probe.C.test_body"])
                    except BaseException as error:
                        caught = error
                self.assertIs(caught, primary if isinstance(primary, (SystemExit, KeyboardInterrupt)) else secondary)

    def source_unwind(self, primary, secondary, diagnostic, wanted):
        from unittest import mock
        import mutation_source
        class Path:
            def write_text(self, value):
                if value == "original":
                    raise secondary
        caught = None
        with mock.patch.object(mutation_source, "print", side_effect=diagnostic, create=True):
            try:
                with mutation_source.changed(Path(), "original", "mutant"):
                    raise primary
            except BaseException as error:
                caught = error
        self.assertIs(caught, wanted, "the first cancellation lost its identity during release")

    def test_first_source_release_cancel_beats_ordinary_primary(self):
        primary, cancel = RuntimeError("ordinary"), SystemExit(37)
        self.source_unwind(primary, cancel, None, cancel)

    def test_first_source_diagnostic_cancel_beats_ordinary_primary(self):
        primary, cancel = RuntimeError("ordinary"), KeyboardInterrupt()
        self.source_unwind(primary, OSError("release"), cancel, cancel)

    def test_original_primary_exception_survives_main_restoration(self):
        for name in ("check_is_build", "held_labels"):
            with self.subTest(script=name):
                self.main_outcome(name, error=RuntimeError("original primary"))

    def test_secondary_restore_or_diagnostic_failure_cannot_replace_primary(self):
        from unittest import mock
        import mutation_source
        class Path:
            def __init__(self):
                self.value = "original"
            def write_text(self, value):
                self.value = value
                if value == "original":
                    raise OSError("secondary restore")
        path = Path()
        primary = SystemExit(37)
        with mock.patch.object(mutation_source, "print", side_effect=OSError("secondary diagnostic"), create=True):
            caught = None
            try:
                with mutation_source.changed(path, "original", "mutant"):
                    raise primary
            except BaseException as error:
                caught = error
        self.assertIs(caught, primary)
        self.assertEqual(path.value, "original")


if __name__ == "__main__":
    unittest.main()

#!/usr/bin/env python3
"""Guide producer/main regressions; real Python children, no compiler/browser."""
import contextlib
import importlib.util
import io
import json
import pathlib
import sys
import tempfile
import types
import unittest
from unittest import mock

ROOT = pathlib.Path(__file__).resolve().parents[2]


def load_guide():
    spec = importlib.util.spec_from_file_location("private_guide_mutations", ROOT / "scripts/guide_mutations.py")
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def cargo_output(failed=False, name="intended_private_witness"):
    if failed:
        return f"test {name} ... FAILED\ntest result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out\n"
    return "test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out\n"


def python_output(failed=False, name="intended_private_witness"):
    if failed:
        return f"FAIL: {name} (private.fixture)\n\nRan 1 test in 0.001s\n\nFAILED (failures=1)\n"
    return "Ran 7 tests in 0.001s\n\nOK\n"


def browser_output(failed=False):
    if failed:
        return "  1) [chromium] › e2e/guestbook.spec.mjs:1:1 › intended private witness ───\n  1 failed\n"
    return "4 passed (1s)\n"


class GuideProducers(unittest.TestCase):
    receipts = {}

    def real_receipt(self, kind):
        if kind not in self.receipts:
            module = load_guide()
            with tempfile.TemporaryDirectory(prefix="pw-guide-real-receipt-") as folder:
                directory = pathlib.Path(folder)
                source = "import unittest\nclass Body(unittest.TestCase):\n"
                if kind != "empty":
                    source += " def test_intended_private_witness(self):\n"
                    source += "  self.fail('real body assertion')\n" if kind == "assertion" else "  pass\n"
                    for number in range(6):
                        source += f" def test_extra_{number}(self): pass\n"
                else:
                    source += " pass\n"
                (directory / "private_receipt.py").write_text(source)
                expected = sorted([f"private_receipt.Body.test_extra_{number}" for number in range(6)] + ["private_receipt.Body.test_intended_private_witness"]) if kind != "empty" else []
                produced = module.guide_python.run("private_receipt", expected, directory, directory / "evidence")
                text = (produced.directory / "result.json").read_text() if (produced.directory / "result.json").exists() else ""
                self.assertEqual(produced.result.valid, kind != "empty")
                self.receipts[kind] = (text, expected, produced.output)
        return self.receipts[kind]

    def guide_result(self, cargo=(0, None), python=(0, "green")):
        module = load_guide()
        cargo = (cargo[0], cargo[1] if cargo[1] is not None else cargo_output())
        kind = python[1]
        text, expected, output = self.real_receipt("green" if kind == "missing" else kind)
        if kind == "missing":
            text = ""
        outcome = module.guide_python.unittest_protocol.validate_result(python[0], text, expected)
        produced = module.guide_python.PythonRun(outcome, output, pathlib.Path("private-fixture-evidence"), python[0])
        with mock.patch.object(module, "run", return_value=cargo), mock.patch.object(module, "python_tests", return_value=produced):
            return module.guide_tests()

    def browser_result(self, status, output):
        module = load_guide()
        with mock.patch.object(module, "run", side_effect=[(0, "built\n"), (status, output)]) as run:
            result = module.browser_tests()
        return result, run.call_args_list, module

    def test_cargo_refuses_invalid_failure_status(self):
        for status in (-9, 137, 67, 0):
            with self.subTest(status=status):
                self.assertIsNone(self.guide_result(cargo=(status, cargo_output(True)))[0])

    def test_cargo_refuses_invalid_green_status(self):
        for status in (-9, 137, 67, 101):
            with self.subTest(status=status):
                self.assertIsNone(self.guide_result(cargo=(status, cargo_output()))[0])

    def test_cargo_refuses_empty_or_missing_summary(self):
        for text in ("", "test result: ok. 0 passed; 0 failed\n"):
            with self.subTest(text=text):
                self.assertIsNone(self.guide_result(cargo=(0, text))[0])

    def test_cargo_accepts_actual_assertion_failure(self):
        ran, failed, names, _ = self.guide_result(cargo=(101, cargo_output(True)))
        self.assertEqual((ran, failed), (8, 1))
        self.assertIn("intended_private_witness", names)

    def test_cargo_all_binary_summaries_must_agree(self):
        mixed = cargo_output() + cargo_output(True)
        ran, failed, names, _ = self.guide_result(cargo=(101, mixed))
        self.assertEqual((ran, failed), (9, 1))
        self.assertIn("intended_private_witness", names)
        self.assertIsNone(self.guide_result(cargo=(0, mixed))[0])

    def test_cargo_refuses_build_or_stop_diagnostics_after_partial_results(self):
        for extra in ("error[E0001]: private build error\n", "error: could not compile private\n", "signal: 9, SIGKILL\n"):
            with self.subTest(extra=extra):
                self.assertIsNone(self.guide_result(cargo=(101, cargo_output(True) + extra))[0])

    def test_cargo_quiet_failure_names_remain_intended_witnesses(self):
        output = "---- intended_private_witness stdout ----\n" + cargo_output(True).split("\n", 1)[1]
        ran, failed, names, _ = self.guide_result(cargo=(101, output))
        self.assertEqual((ran, failed), (8, 1))
        self.assertIn("intended_private_witness", names)

    def test_cargo_refuses_empty_binary_among_nonempty_results(self):
        self.assertIsNone(self.guide_result(cargo=(0, cargo_output() + "test result: ok. 0 passed; 0 failed; 0 ignored;\n"))[0])

    def test_python_refuses_invalid_failure_status(self):
        for status in (-9, 137, 67, 0):
            with self.subTest(status=status):
                self.assertIsNone(self.guide_result(python=(status, "assertion"))[0])

    def test_python_refuses_invalid_green_status(self):
        for status in (-9, 137, 67, 1):
            with self.subTest(status=status):
                self.assertIsNone(self.guide_result(python=(status, "green"))[0])

    def test_python_refuses_empty_or_missing_summary(self):
        for kind in ("missing", "empty"):
            with self.subTest(kind=kind):
                self.assertIsNone(self.guide_result(python=(0, kind))[0])

    def test_python_accepts_actual_assertion_failure(self):
        ran, failed, names, _ = self.guide_result(python=(1, "assertion"))
        self.assertEqual((ran, failed), (8, 1))
        self.assertIn("test_intended_private_witness", names)

    def test_guide_accepts_green_producers(self):
        ran, failed, names, _ = self.guide_result()
        self.assertEqual((ran, failed, names), (8, 0, set()))

    def test_guide_retains_earlier_producer_output(self):
        out = self.guide_result(cargo=(101, "earlier Cargo detail\n" + cargo_output(True)))[-1]
        self.assertIn("earlier Cargo detail", out)
        self.assertIn("Ran 7 tests", out)

    def test_browser_refuses_invalid_failure_status(self):
        for status in (-9, 137, 67, 0):
            with self.subTest(status=status):
                self.assertFalse(self.browser_result(status, browser_output(True))[0][0])

    def test_browser_refuses_invalid_green_status(self):
        for status in (-9, 137, 67, 1):
            with self.subTest(status=status):
                self.assertFalse(self.browser_result(status, browser_output())[0][0])

    def test_browser_refuses_empty_or_missing_summary(self):
        for text in ("", "0 passed\n"):
            with self.subTest(text=text):
                self.assertFalse(self.browser_result(0, text)[0][0])

    def test_browser_accepts_actual_assertion_failure(self):
        result, _, _ = self.browser_result(1, browser_output(True))
        self.assertEqual(result[:3], (True, 1, 1))
        self.assertIn("intended private witness", result[3])

    def test_browser_accepts_green_producer(self):
        result, _, _ = self.browser_result(0, browser_output())
        self.assertEqual(result[:3], (True, 4, 0))

    def test_browser_uses_canonical_authority_and_exact_selection(self):
        result, calls, module = self.browser_result(0, browser_output())
        self.assertTrue(result[0])
        expected = ["bash", str(ROOT / "scripts/run_suite.sh"), "-C", ".", "--", *module.SPEC]
        self.assertEqual(calls[1].args[0], expected)
        self.assertEqual(calls[1].kwargs["cwd"], ROOT / "spikes/own-renderer")

    def run_main(self, phase, status=0, mode="invalid", codes=("PW5048",), named=False):
        module = load_guide()
        held = "browser" if phase in ("browser", "build") else "guide"
        witness = "intended private witness" if held == "browser" else ("test_intended_private_witness" if phase == "python" else "intended_private_witness")
        what = "private intended mutation"
        watch = types.SimpleNamespace(stopped=[])
        baseline = types.SimpleNamespace(explain=mock.Mock(), mutation_bound=types.SimpleNamespace(start=lambda: watch))
        output = io.StringIO()
        with tempfile.TemporaryDirectory(prefix="pw-guide-producer-") as directory:
            source = pathlib.Path(directory) / "owned-source"
            original = b"one original anchor\n"
            source.write_bytes(original)
            build_calls = 0
            cargo_calls = 0
            def produce(command, **_):
                nonlocal build_calls, cargo_calls
                mutated = source.read_bytes() != original
                if command == module.CARGO_TEST:
                    cargo_calls += 1
                    if mode == "bad_baseline" and cargo_calls == 1:
                        return 101, cargo_output()
                    if mutated and phase == "cargo":
                        if mode == "stopped":
                            watch.stopped.append(("private-owned-child", 1))
                        return (0, cargo_output()) if mode == "green" else (status, cargo_output(True, witness))
                    return 0, cargo_output()
                if command == module.PYTHON_TEST:
                    if mutated and phase == "python":
                        return (0, python_output()) if mode == "green" else (status, python_output(True, witness))
                    return 0, python_output()
                if command == module.APP_BUILD:
                    build_calls += 1
                    if mode == "cleanup_failure" and build_calls == 3:
                        return 67, "private cleanup build failure\n"
                    if mutated and phase == "build":
                        return status, "\n".join(f"error: [{code}] private named diagnostic" for code in codes)
                    return 0, "built\n"
                if command[-len(module.SPEC):] == module.SPEC:
                    if mutated and phase == "browser":
                        return (0, browser_output()) if mode == "green" else (status, browser_output(True))
                    return 0, browser_output()
                raise AssertionError(f"unexpected command: {command}")
            expected_python = sorted([f"private.Body.test_extra_{number}" for number in range(6)] + ["private.Body.test_intended_private_witness"])
            def produce_python():
                protocol = module.guide_python.unittest_protocol
                if source.read_bytes() != original and phase == "python" and mode != "green":
                    if status == 1:
                        failed_id = "private.Body.test_intended_private_witness"
                        result = protocol.Result(True, "mocked validated body assertion", tuple(identity for identity in expected_python if identity != failed_id), (failed_id,), (), 7)
                    else:
                        result = protocol.Result(False, "mocked invalid native child status")
                else:
                    result = protocol.Result(True, "mocked validated green bodies", tuple(expected_python), (), (), 7)
                return module.guide_python.PythonRun(result, "mocked structured Python domain outcome", pathlib.Path(directory), status)
            with contextlib.ExitStack() as stack:
                stack.enter_context(mock.patch.object(module, "MUTANTS", [(what, held, source, "original", "mutated")]))
                stack.enter_context(mock.patch.object(module, "MUST", {what: set() if named else {witness}}))
                stack.enter_context(mock.patch.object(module, "CHECKED", {what: {"PW5048"}} if named else {}))
                stack.enter_context(mock.patch.object(module, "run", side_effect=produce))
                stack.enter_context(mock.patch.object(module, "freeze_python", return_value=expected_python))
                stack.enter_context(mock.patch.object(module, "python_tests", side_effect=produce_python))
                stack.enter_context(mock.patch.dict(sys.modules, {"mutation_baseline": baseline}))
                stack.enter_context(contextlib.redirect_stdout(output))
                code = module.main()
            self.assertEqual(source.read_bytes(), original, "actual main finally must restore exact fixture bytes")
        return code, output.getvalue()

    def assert_refused_without_kill(self, result):
        code, output = result
        self.assertNotEqual(code, 0)
        self.assertIn("REFUSED", output)
        self.assertNotIn("KILLED", output)
        self.assertNotIn("1 of 1 mutants killed", output)

    def test_main_refuses_cargo_invalid_status_without_kill(self):
        for status in (-9, 137, 67, 0):
            with self.subTest(status=status):
                self.assert_refused_without_kill(self.run_main("cargo", status))

    def test_main_refuses_python_invalid_status_without_kill(self):
        for status in (-9, 137, 67, 0):
            with self.subTest(status=status):
                self.assert_refused_without_kill(self.run_main("python", status))

    def test_main_refuses_browser_invalid_status_without_kill(self):
        for status in (-9, 137, 67, 0):
            with self.subTest(status=status):
                self.assert_refused_without_kill(self.run_main("browser", status))

    def test_main_refuses_named_codes_after_invalid_build_status(self):
        for status in (-9, 137, 67):
            with self.subTest(status=status):
                self.assert_refused_without_kill(self.run_main("build", status, named=True))

    def test_main_refuses_additional_named_codes(self):
        self.assert_refused_without_kill(self.run_main("build", 1, codes=("PW5048", "PW6601"), named=True))

    def test_main_refuses_unlisted_compiler_refusal(self):
        self.assert_refused_without_kill(self.run_main("build", 1))

    def test_main_accepts_exact_named_compiler_refusal(self):
        code, output = self.run_main("build", 1, named=True)
        self.assertEqual(code, 0)
        self.assertIn("KILLED by the checker (PW5048)", output)

    def test_main_accepts_valid_intended_assertion_neighbours(self):
        for phase, status in (("cargo", 101), ("python", 1), ("browser", 1)):
            with self.subTest(phase=phase):
                code, output = self.run_main(phase, status)
                self.assertEqual(code, 0)
                self.assertIn("KILLED", output)
                self.assertIn("1 of 1 mutants killed", output)

    def test_main_green_neighbours_are_survivors(self):
        for phase in ("cargo", "python", "browser"):
            with self.subTest(phase=phase):
                code, output = self.run_main(phase, mode="green")
                self.assertNotEqual(code, 0)
                self.assertIn("SURVIVED", output)
                self.assertNotIn("KILLED", output)

    def test_main_refuses_bound_stop_with_valid_partial_summary(self):
        self.assert_refused_without_kill(self.run_main("cargo", 101, mode="stopped"))

    def test_main_refuses_invalid_green_baseline(self):
        code, output = self.run_main("cargo", 101, mode="bad_baseline")
        self.assertNotEqual(code, 0)
        self.assertNotIn("KILLED", output)

    def test_final_restored_build_failure_fails_campaign(self):
        code, output = self.run_main("browser", 1, mode="cleanup_failure")
        self.assertNotEqual(code, 0)
        self.assertIn("does not build again from the restored sources", output)


class GuideNativeProtocol(unittest.TestCase):
    """Actual native child/fixture provenance through the real guide main."""

    def run_native_main(self, mode):
        module = load_guide()
        watch = types.SimpleNamespace(stopped=[])
        baseline = types.SimpleNamespace(explain=mock.Mock(), mutation_bound=types.SimpleNamespace(start=lambda: watch))
        with tempfile.TemporaryDirectory(prefix="pw-guide-native-main-") as folder:
            directory = pathlib.Path(folder)
            source = directory / "private_native.py"
            marker = directory / "entered"
            original = '''import pathlib, unittest
MODE = "green"
MARKER = pathlib.Path(__file__).with_name("entered")
class Native(unittest.TestCase):
 def setUp(self):
  if MODE == "setup_error": raise OSError("actual setUp error")
  if MODE == "setup_assertion": raise AssertionError("actual setUp assertion")
 def tearDown(self):
  if MODE == "teardown_error": raise OSError("actual tearDown error")
  if MODE == "teardown_assertion": raise AssertionError("actual tearDown assertion")
 if MODE == "async":
  async def test_required(self):
   MARKER.write_text("entered")
   self.fail("deferred body must not earn a kill")
 elif MODE == "generator":
  def test_required(self):
   MARKER.write_text("entered")
   yield "deferred"
 elif MODE == "expected_failure":
  @unittest.expectedFailure
  def test_required(self):
   MARKER.write_text("entered")
   self.fail("expected failure")
 else:
  def test_required(self):
   MARKER.write_text("entered")
   if MODE == "assertion": self.fail("actual body assertion")
   if MODE == "body_error": raise OSError("actual body error")
   if MODE == "skip": self.skipTest("actual skip")
   if MODE == "returned_coroutine":
    async def later(): pass
    return later()
'''.encode()
            source.write_bytes(original)
            produced = []
            real_run = module.guide_python.run
            mutated_marker_cleared = False
            def cargo(command, **kwargs):
                nonlocal mutated_marker_cleared
                self.assertEqual(command, module.CARGO_TEST)
                if source.read_bytes() != original and not mutated_marker_cleared:
                    marker.unlink(missing_ok=True)
                    mutated_marker_cleared = True
                return 0, cargo_output()
            def python_run(*args, **kwargs):
                result = real_run(*args, **kwargs)
                produced.append(result)
                return result
            output = io.StringIO()
            what = "native intended mutation"
            with contextlib.ExitStack() as stack:
                for name, value in {
                    "MUTANTS": [(what, "guide", source, 'MODE = "green"', f'MODE = "{mode}"')],
                    "MUST": {what: {"test_required"}}, "CHECKED": {},
                    "PYTHON_SELECTOR": "private_native", "PYTHON_SEARCH_DIR": directory,
                    "PYTHON_PATTERN": "private_native.py", "PYTHON_EXPECTED_COUNT": 1,
                }.items():
                    stack.enter_context(mock.patch.object(module, name, value))
                stack.enter_context(mock.patch.object(module, "run", side_effect=cargo))
                stack.enter_context(mock.patch.object(module.guide_python, "run", side_effect=python_run))
                stack.enter_context(mock.patch.dict(sys.modules, {"mutation_baseline": baseline}))
                stack.enter_context(contextlib.redirect_stdout(output))
                code = module.main()
            self.assertEqual(source.read_bytes(), original, "real guide main must restore exact private source bytes")
            self.assertEqual(len(produced), 2, "baseline and mutant each launch a real canonical child")
            self.assertTrue(produced[0].result.valid)
            self.assertEqual(produced[0].result.executed, 1)
            return code, output.getvalue(), produced[-1], marker.exists()

    def test_native_setup_failures_earn_no_kill_and_never_enter_body(self):
        for mode in ("setup_error", "setup_assertion"):
            with self.subTest(mode=mode):
                code, output, produced, entered = self.run_native_main(mode)
                self.assertNotEqual(code, 0)
                self.assertIn("REFUSED", output)
                self.assertNotIn("KILLED", output)
                self.assertFalse(produced.result.valid)
                self.assertEqual(produced.result.executed, 0)
                self.assertFalse(entered)

    def test_native_teardown_failures_earn_no_kill_after_body_entry(self):
        for mode in ("teardown_error", "teardown_assertion"):
            with self.subTest(mode=mode):
                code, output, produced, entered = self.run_native_main(mode)
                self.assertNotEqual(code, 0)
                self.assertIn("REFUSED", output)
                self.assertNotIn("KILLED", output)
                self.assertFalse(produced.result.valid)
                self.assertEqual(produced.result.executed, 0)
                self.assertTrue(entered)

    def test_native_body_assertion_is_a_named_assertion_kill(self):
        code, output, produced, entered = self.run_native_main("assertion")
        self.assertEqual(code, 0)
        self.assertIn("KILLED by test_required", output)
        self.assertIn("Python body assertion", output)
        self.assertNotIn("Python body error", output)
        self.assertEqual(produced.result.assertion_failed, ("private_native.Native.test_required",))
        self.assertEqual(produced.result.body_errors, ())
        self.assertTrue(entered)

    def test_native_body_error_preserves_the_distinct_named_error_policy(self):
        code, output, produced, entered = self.run_native_main("body_error")
        self.assertEqual(code, 0)
        self.assertIn("KILLED by test_required", output)
        self.assertIn("Python body error", output)
        self.assertNotIn("Python body assertion", output)
        self.assertEqual(produced.result.body_errors, ("private_native.Native.test_required",))
        self.assertEqual(produced.result.assertion_failed, ())
        self.assertTrue(entered)

    def test_native_skip_expected_and_deferred_bodies_earn_no_kill(self):
        for mode in ("skip", "expected_failure", "async", "generator", "returned_coroutine"):
            with self.subTest(mode=mode):
                code, output, produced, _ = self.run_native_main(mode)
                self.assertNotEqual(code, 0)
                self.assertIn("REFUSED", output)
                self.assertNotIn("KILLED", output)
                self.assertFalse(produced.result.valid)
                self.assertEqual(produced.result.executed, 0)

    def test_native_guide_discovery_preserves_exact_original_seven_ids(self):
        module = load_guide()
        expected = [
            "test_guide_diagnostics.Current.test_every_registered_code_has_a_row",
            "test_guide_diagnostics.Current.test_the_committed_page_is_what_the_registry_writes",
            "test_guide_diagnostics.Reading.test_a_line_that_is_no_entry_is_refused_not_skipped",
            "test_guide_diagnostics.Reading.test_a_pipe_in_a_cell_is_escaped",
            "test_guide_diagnostics.Reading.test_a_retired_code_with_its_reason_continued_across_lines",
            "test_guide_diagnostics.Reading.test_an_owner_without_a_title_is_written_from_its_name",
            "test_guide_diagnostics.Reading.test_each_entry_with_its_invariant_and_the_adrs_its_comment_cites",
        ]
        actual = module.freeze_python()
        self.assertEqual(actual, expected)
        produced = module.guide_python.run(module.PYTHON_SELECTOR, actual, module.PYTHON_SEARCH_DIR)
        self.assertTrue(produced.result.valid, produced.output)
        self.assertEqual(produced.result.passed, tuple(expected))
        self.assertEqual(produced.result.executed, 7)

    def test_native_timeout_refuses_and_reaps_its_owned_child(self):
        module = load_guide()
        with tempfile.TemporaryDirectory(prefix="pw-guide-timeout-") as folder:
            directory = pathlib.Path(folder)
            (directory / "private_timeout.py").write_text("import time, unittest\nclass Native(unittest.TestCase):\n def test_required(self): time.sleep(30)\n")
            produced = module.guide_python.run("private_timeout", ["private_timeout.Native.test_required"], directory, directory / "evidence", timeout=0.3)
            self.assertFalse(produced.result.valid)
            self.assertEqual(produced.result.executed, 0)
            self.assertIsNone(produced.status)
            metadata = json.loads((produced.directory / "process.json").read_text())
            self.assertTrue(metadata["timed_out"])
            self.assertTrue(metadata["child_reaped"])
            self.assertTrue(metadata["deployment_env_absent"])

    def test_native_fresh_cache_does_not_read_same_timestamp_prior_mutant(self):
        import os
        import py_compile
        module = load_guide()
        with tempfile.TemporaryDirectory(prefix="pw-guide-bytecode-") as folder:
            directory = pathlib.Path(folder)
            source = directory / "private_cached.py"
            old = 'import unittest\nclass Native(unittest.TestCase):\n def test_required(self): self.assertEqual("old", "old")\n'
            new = old.replace('("old", "old")', '("new", "old")')
            self.assertEqual(len(old), len(new))
            source.write_text(old)
            stamp = source.stat()
            cached = pathlib.Path(py_compile.compile(str(source), doraise=True))
            cached_bytes = cached.read_bytes()
            source.write_text(new)
            os.utime(source, ns=(stamp.st_atime_ns, stamp.st_mtime_ns))
            produced = module.guide_python.run("private_cached", ["private_cached.Native.test_required"], directory, directory / "evidence")
            self.assertTrue(produced.result.valid, produced.output)
            self.assertEqual(produced.result.assertion_failed, ("private_cached.Native.test_required",))
            self.assertEqual(cached.read_bytes(), cached_bytes, "existing cache must remain untouched")

if __name__ == "__main__":
    unittest.main()
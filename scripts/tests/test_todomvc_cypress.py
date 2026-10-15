"""TodoMVC's own Cypress suite: unchanged (`todomvc_cypress_vendored.py`),
judged by what it prints (`todomvc_cypress_result.py`), and run by `run.sh`.

Track `todomvc` runs the official spec unchanged as its acceptance. That is a
claim about bytes (the vendored files are upstream's) and a claim about a run
(the suite ran, and each test came out as the app's design gives it). These
tests hold the checks to both: the fixtures are real outputs of Cypress 15.14.2 and
15.21.1 against this app (`fixtures/todomvc-cypress/`), each rule of the judgement is
broken in a copy of one of them, and `run.sh` is run whole with a stand-in
Cypress that prints what a test says, so a suite that cannot run is seen to
fail and not to be skipped.
"""

import contextlib
import importlib.util
import io
import os
import pathlib
import re
import socket
import subprocess
import tempfile
import unittest

HERE = pathlib.Path(__file__).resolve().parent
SCRIPTS = HERE.parent
ROOT = SCRIPTS.parent
FIXTURES = HERE / "fixtures" / "todomvc-cypress"
RUN_SH = ROOT / "spikes/todomvc-cypress/run.sh"


def load(name: str):
    spec = importlib.util.spec_from_file_location(name, SCRIPTS / f"{name}.py")
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


vendored = load("todomvc_cypress_vendored")
result = load("todomvc_cypress_result")


def fixture(name):
    return (FIXTURES / name).read_text(encoding="utf-8")


# Cypress 15.14.2 (the first three) and 15.21.1 (the last two), as they printed.
ACCEPTANCE = fixture("acceptance.txt")  # the hook fails at test 7; 4 failing
ACCEPTANCE_6 = fixture("acceptance-hook-at-6.txt")  # the hook fails at test 6; 3 failing
DIAGNOSTIC = fixture("diagnostic.txt")  # 28 passing, 1 pending
RACY = fixture("diagnostic-racy.txt")  # as it is when test 11 loses its race
ACCEPTANCE_NEW = fixture("acceptance-15.21.1.txt")  # the hook fails at test 6; 3 failing
DIAGNOSTIC_NEW = fixture("diagnostic-15.21.1.txt")  # 28 passing, 1 pending
TESTS = result.spec_tests(result.SPEC.read_text(encoding="utf-8"))


def judged(mode, text, status=None, tests=TESTS):
    return result.judge(mode, tests, result.parse_run(text), status)


def has(problems, part):
    return any(part in p for p in problems)


def edit(text, pattern, new):
    """`text` with the one line matching `pattern` replaced by `new`."""
    found = re.findall(pattern, text, re.M)
    assert len(found) == 1, f"{pattern!r} matches {len(found)} lines"
    return re.sub(pattern, lambda _: new, text, count=1, flags=re.M)


def without(text, part):
    return "\n".join(line for line in text.split("\n") if part not in line)


class Vendored(unittest.TestCase):
    def test_a_blob_is_gits(self):
        # `printf 'hello\n' | git hash-object --stdin`, and the empty blob's
        # well-known hash.
        self.assertEqual(
            vendored.blob(b"hello\n"), "ce013625030ba8dba906f756967f9e9ca394464a"
        )
        self.assertEqual(vendored.blob(b""), "e69de29bb2d1d6434b8b29ae775ad8c2e48c5391")

    def test_the_suite_in_the_tree_is_upstreams(self):
        self.assertEqual(vendored.problems(), [])

    def test_a_file_changed_by_a_byte_is_not(self):
        real = vendored.DIR
        with tempfile.TemporaryDirectory() as tmp:
            copy = pathlib.Path(tmp)
            for path in list(vendored.BLOBS) + ["package.json"]:
                target = copy / path
                target.parent.mkdir(parents=True, exist_ok=True)
                target.write_bytes((real / path).read_bytes())
            vendored.DIR = copy
            try:
                self.assertEqual(vendored.problems(), [])
                with contextlib.redirect_stdout(io.StringIO()):
                    self.assertEqual(vendored.main(), 0)
                spec = copy / "cypress/e2e/spec.cy.js"
                spec.write_bytes(spec.read_bytes() + b" ")
                found = vendored.problems()
                self.assertEqual(len(found), 1, found)
                self.assertIn("spec.cy.js", found[0])
                with contextlib.redirect_stdout(io.StringIO()):
                    self.assertEqual(vendored.main(), 1)
                # And a file gone is a problem too, and so is another Cypress.
                spec.unlink()
                self.assertTrue(any("missing" in p for p in vendored.problems()))
                (copy / "package.json").write_text('{"devDependencies": {"cypress": "16.2.0"}}')
                self.assertTrue(any("16.2.0" in p for p in vendored.problems()))
            finally:
                vendored.DIR = real


class Spec(unittest.TestCase):
    """The tests, and what each asks of localStorage, are read from the spec."""

    def test_the_spec_has_29_tests_in_order(self):
        self.assertEqual([t.n for t in TESTS], list(range(1, 30)))
        self.assertEqual(TESTS[0].title, "should focus on the todo input field")
        self.assertEqual(TESTS[0].context, "When page is initially opened")
        self.assertEqual(TESTS[10].title, "complete all checkbox should update state when items are completed / cleared")
        self.assertEqual(TESTS[-1].title, "should highlight the currently applied filter")
        self.assertEqual(TESTS[-1].context, "Routing")
        self.assertEqual(TESTS[3].line, 561)

    def test_six_tests_ask_nothing_of_localstorage(self):
        self.assertEqual({t.n for t in TESTS if not result.asks(t)}, {1, 2, 3, 21, 22, 23})

    def test_a_test_asks_through_its_own_body_and_its_contexts_hook(self):
        direct = lambda n: set(TESTS[n - 1].direct)
        hooked = lambda n: set(TESTS[n - 1].hooked)
        self.assertEqual(direct(4), {"checkItemSaved", "checkNumberOfTodosInLocalStorage"})
        self.assertEqual(hooked(4), set())
        # Mark all as completed: its beforeEach asks for 3 todos and a save.
        self.assertEqual(hooked(9), {"checkNumberOfTodosInLocalStorage", "checkItemSaved"})
        # Routing's beforeEach is the only thing the last test asks it through.
        self.assertEqual(direct(29), set())
        self.assertEqual(hooked(29), {"checkTodosInLocalStorage"})
        self.assertTrue(result.asks(TESTS[28]))
        # Clear completed button's beforeEach asks nothing.
        self.assertEqual(hooked(21), set())
        self.assertEqual(
            direct(24),
            {"checkCompletedKeywordInLocalStorage", "checkNumberOfCompletedTodosInLocalStorage", "checkTodosInLocalStorage"},
        )


class Parse(unittest.TestCase):
    def test_the_acceptance_is_read(self):
        run = result.parse_run(ACCEPTANCE)
        self.assertEqual(run["cypress"], "15.14.2")
        self.assertIn("Electron", run["browser"])
        self.assertEqual(run["counts"], {"Tests": 29, "Passing": 3, "Failing": 4, "Pending": 0, "Skipped": 22})
        self.assertEqual(run["passed"], ["should focus on the todo input field", "starts with nothing", "should hide #main and #footer"])
        self.assertEqual(
            [title for _, title in run["failed"]],
            [
                "should allow me to add todo items",
                "should clear text input field when an item is added",
                "should append new items to the bottom of the list",
                '"before each" hook for "should trim text input"',
            ],
        )
        self.assertTrue(run["errors"][1].startswith("AssertionError: Timed out retrying after 4000ms: expected setItem"))
        self.assertTrue(run["errors"][2].startswith("CypressError: `cy.then()` timed out"))
        self.assertEqual(run["errors"][4], "Error: Cannot determine what kind of selectors this app uses.")
        self.assertTrue(run["storage_denied"])

    def test_the_newer_cypress_prints_the_same_lines(self):
        run = result.parse_run(ACCEPTANCE_NEW)
        self.assertEqual(run["cypress"], "15.21.1")
        self.assertEqual(run["counts"]["Tests"], 29)
        self.assertEqual(len(run["passed"]), 3)
        self.assertTrue(run["storage_denied"])
        self.assertEqual(result.parse_run(DIAGNOSTIC_NEW)["pending"], ["should persist its data"])

    def test_the_diagnostic_is_read(self):
        run = result.parse_run(DIAGNOSTIC)
        self.assertEqual(len(run["passed"]), 28)
        self.assertEqual(run["pending"], ["should persist its data"])
        self.assertEqual(run["failed"], [])
        self.assertFalse(run["storage_denied"])
        racy = result.parse_run(RACY)
        self.assertEqual([t for _, t in racy["failed"]], ["complete all checkbox should update state when items are completed / cleared"])
        self.assertIn("not to be 'checked'", racy["errors"][1])

    def test_colour_codes_are_no_part_of_a_line(self):
        coloured = DIAGNOSTIC.replace("✓", "\x1b[32m✓\x1b[0m").replace("Passing:", "\x1b[1mPassing:\x1b[0m")
        self.assertEqual(result.parse_run(coloured)["passed"], result.parse_run(DIAGNOSTIC)["passed"])
        self.assertEqual(result.parse_run(coloured)["counts"], result.parse_run(DIAGNOSTIC)["counts"])

    def test_nothing_is_read_where_nothing_ran(self):
        run = result.parse_run("")
        self.assertEqual((run["passed"], run["failed"], run["pending"], run["counts"], run["cypress"]), ([], [], [], {}, None))


class JudgeAcceptance(unittest.TestCase):
    def test_the_acceptances_the_app_gives_are_judged_right(self):
        for text, status in ((ACCEPTANCE, 4), (ACCEPTANCE_6, 3), (ACCEPTANCE_NEW, 3)):
            st, problems = judged("acceptance", text, status)
            self.assertEqual(problems, [])
            self.assertEqual([st[n].state for n in (1, 2, 3)], ["pass"] * 3)
        st, _ = judged("acceptance", ACCEPTANCE, 4)
        self.assertEqual([st[n].state for n in (4, 5, 6)], ["fail"] * 3)
        self.assertEqual(st[7].state, "hook")
        self.assertEqual({st[n].state for n in range(8, 30)}, {"skip"})
        # Where the hook fails depends on timing: one test sooner here.
        st, _ = judged("acceptance", ACCEPTANCE_6, 3)
        self.assertEqual([st[4].state, st[5].state, st[6].state, st[7].state], ["fail", "fail", "hook", "skip"])

    def test_a_suite_that_did_not_run_is_not_a_pass(self):
        st, problems = judged("acceptance", "")
        self.assertIsNone(st)
        self.assertTrue(has(problems, "did not run the suite"))
        st, problems = judged("acceptance", "The cypress npm package is installed, but the Cypress binary is missing.\n", 1)
        self.assertIsNone(st)
        self.assertTrue(has(problems, "did not run the suite"))

    def test_a_test_that_asks_localstorage_and_passes_is_refused(self):
        _, problems = judged("acceptance", DIAGNOSTIC, 0)
        self.assertTrue(has(problems, "passed, though it asks localStorage"))
        edited = edit(ACCEPTANCE, r"^ +1\) should allow me to add todo items$", "      ✓ should allow me to add todo items (10ms)")
        _, problems = judged("acceptance", edited, 4)
        self.assertTrue(has(problems, "test 4 ('should allow me to add todo items') passed, though it asks localStorage"))

    def test_a_test_that_asks_nothing_and_fails_is_refused(self):
        edited = edit(ACCEPTANCE, r"^ +✓ should hide #main and #footer .*$", "      9) should hide #main and #footer")
        _, problems = judged("acceptance", edited, 4)
        self.assertTrue(has(problems, "failed, though it asks nothing of localStorage"))

    def test_the_first_tests_must_pass(self):
        edited = edit(ACCEPTANCE, r"^ +✓ should focus on the todo input field .*$", '      9) "before each" hook for "should focus on the todo input field"')
        _, problems = judged("acceptance", edited, 4)
        self.assertTrue(has(problems, "comes first, so it must pass"))

    def test_a_test_not_run_needs_a_failed_hook_before_it(self):
        edited = edit(ACCEPTANCE, r'^ +4\) "before each" hook for "should trim text input"$', "")
        _, problems = judged("acceptance", edited, 3)
        self.assertTrue(has(problems, "was not run, and no failed hook before it says why"))

    def test_a_failed_hook_needs_its_cause_on_record(self):
        _, problems = judged("acceptance", without(ACCEPTANCE, "SecurityError"), 4)
        self.assertTrue(has(problems, "does not record why"))

    def test_a_pending_test_is_refused(self):
        edited = edit(ACCEPTANCE, r"^ +✓ starts with nothing .*$", "      - starts with nothing")
        _, problems = judged("acceptance", edited, 4)
        self.assertTrue(has(problems, "is pending"))

    def test_the_table_must_say_what_the_lines_say(self):
        edited = edit(ACCEPTANCE, r"^( +│ Passing: +)3( +│)$", "  │ Passing:      4                                                                                │")
        _, problems = judged("acceptance", edited, 4)
        self.assertTrue(has(problems, "Cypress's table says Passing: 4"))

    def test_the_table_must_count_the_specs_tests(self):
        _, problems = judged("acceptance", ACCEPTANCE, 4, tests=TESTS[:-1])
        self.assertTrue(has(problems, "the spec has 28 tests"))

    def test_the_exit_status_must_be_the_failing_count(self):
        _, problems = judged("acceptance", ACCEPTANCE, 99)
        self.assertTrue(has(problems, "exited with status 99"))
        _, problems = judged("acceptance", ACCEPTANCE, 4)
        self.assertEqual(problems, [])

    def test_a_test_the_spec_does_not_have_is_named(self):
        edited = edit(ACCEPTANCE, r"^ +✓ starts with nothing .*$", "      ✓ starts with nothing (1ms)\n      ✓ a test the spec lacks (5ms)")
        _, problems = judged("acceptance", edited, 4)
        self.assertTrue(has(problems, "names a test the spec does not have: 'a test the spec lacks'"))


class JudgeDiagnostic(unittest.TestCase):
    def test_the_diagnostics_the_app_gives_are_judged_right(self):
        for text in (DIAGNOSTIC, DIAGNOSTIC_NEW):
            _, problems = judged("diagnostic", text, 0)
            self.assertEqual(problems, [])
        st, problems = judged("diagnostic", DIAGNOSTIC, 0)
        self.assertEqual(problems, [])
        self.assertEqual(st[24].state, "pending")
        self.assertEqual(st[11].state, "pass")
        self.assertEqual({st[n].state for n in range(1, 30) if n != 24}, {"pass"})
        st, problems = judged("diagnostic", RACY, 1)
        self.assertEqual(problems, [])
        self.assertEqual(st[11].state, "fail")

    def test_the_persistence_test_must_be_skipped_by_the_suite(self):
        edited = edit(DIAGNOSTIC, r"^ +- should persist its data$", "      ✓ should persist its data (5ms)")
        _, problems = judged("diagnostic", edited, 0)
        self.assertTrue(has(problems, "should be skipped by the suite for this name"))

    def test_every_other_test_must_pass(self):
        edited = edit(DIAGNOSTIC, r"^ +✓ should allow me to mark items as complete .*$", "      9) should allow me to mark items as complete")
        _, problems = judged("diagnostic", edited, 1)
        self.assertTrue(has(problems, "every test but two should pass"))

    def test_the_racy_test_may_lose_only_its_race(self):
        edited = edit(RACY, r"^( +AssertionError: Timed out retrying after 4000ms: expected )'<input#toggle-all.toggle-all>' not to be 'checked'$",
                      "     AssertionError: Timed out retrying after 4000ms: expected something else")
        _, problems = judged("diagnostic", edited, 1)
        self.assertTrue(has(problems, "another reason than its race"))

    def test_the_racy_test_may_not_be_skipped(self):
        edited = edit(
            DIAGNOSTIC,
            r"^ +✓ complete all checkbox should update state when items are completed / cleared .*$",
            "      - complete all checkbox should update state when items are completed / cleared",
        )
        _, problems = judged("diagnostic", edited, 0)
        self.assertTrue(has(problems, "passes or loses its race"))

    def test_a_diagnostic_that_did_not_run_is_not_a_pass(self):
        st, problems = judged("diagnostic", "")
        self.assertIsNone(st)
        self.assertTrue(has(problems, "did not run the suite"))


class Cli(unittest.TestCase):
    def main(self, *args):
        out = io.StringIO()
        with contextlib.redirect_stdout(out):
            code = result.main(["todomvc_cypress_result.py", *args])
        return code, out.getvalue()

    def test_the_report_says_what_each_test_came_to_and_why(self):
        code, text = self.main("acceptance", str(FIXTURES / "acceptance.txt"), "4")
        self.assertEqual(code, 0)
        self.assertIn("acceptance: 3 of 29 passed; 4 failed (3 tests, 1 hook); 22 not run", text)
        self.assertIn("cypress 15.14.2 on Electron", text)
        self.assertIn("[New Todo] should allow me to add todo items  (spec.cy.js:561)", text)
        self.assertIn("by design: it asks localStorage (checkItemSaved, checkNumberOfTodosInLocalStorage), which this app never writes", text)
        self.assertIn("not run (a failed hook skipped it); it asks nothing of localStorage: the diagnostic runs it", text)
        self.assertIn("recorded by the browser: Uncaught SecurityError", text)
        self.assertIn("acceptance: the outcome is the one the app's design gives", text)

    def test_the_report_of_the_diagnostic_says_it_is_not_the_acceptance(self):
        code, text = self.main("diagnostic", str(FIXTURES / "diagnostic.txt"), "0")
        self.assertEqual(code, 0)
        self.assertIn("The diagnostic, NOT the acceptance", text)
        self.assertIn("diagnostic: 28 of 29 passed; 1 pending", text)
        self.assertIn("skipped by the suite itself for this name", text)

    def test_the_report_names_a_race_a_race(self):
        code, text = self.main("diagnostic", str(FIXTURES / "diagnostic-racy.txt"), "1")
        self.assertEqual(code, 0)
        self.assertIn("diagnostic: 27 of 29 passed; 1 failed; 1 pending", text)
        self.assertIn("by timing, not by design", text)

    def test_a_suite_that_did_not_run_exits_1(self):
        with tempfile.TemporaryDirectory() as tmp:
            empty = pathlib.Path(tmp) / "empty.txt"
            empty.write_text("")
            code, text = self.main("acceptance", str(empty), "1")
            self.assertEqual(code, 1)
            self.assertIn("a suite that did not run is not a pass", text)
            code, text = self.main("diagnostic", str(pathlib.Path(tmp) / "nowhere.txt"), "1")
            self.assertEqual(code, 1)
            self.assertIn("is not there", text)

    def test_an_outcome_the_design_does_not_give_exits_1(self):
        code, text = self.main("acceptance", str(FIXTURES / "diagnostic.txt"), "0")
        self.assertEqual(code, 1)
        self.assertIn("PROBLEM:", text)
        self.assertIn("acceptance: FAILED:", text)
        code, _ = self.main("acceptance", str(FIXTURES / "acceptance.txt"), "7")
        self.assertEqual(code, 1)

    def test_a_mode_that_is_neither_is_a_usage_error(self):
        code, _ = self.main("both", str(FIXTURES / "acceptance.txt"))
        self.assertEqual(code, 2)
        code, _ = self.main()
        self.assertEqual(code, 2)


FAKE_PNPM = """#!/bin/sh
# `pnpm exec cypress ...` in a test of run.sh: verify succeeds or fails as the
# test says, --version answers, and run prints the output the test names (the
# browser's console is in it only where logging is on) with the status the
# test names. What it was asked, and the environment it was given, go to
# $FAKE_CALLS.
echo "$*" >> "$FAKE_CALLS"
case "$3" in
  verify) exit "$FAKE_VERIFY" ;;
  --version) echo "Cypress package version: 15.21.1"; exit 0 ;;
  run)
    echo "env DISPLAY=${DISPLAY-unset} WAYLAND_DISPLAY=${WAYLAND_DISPLAY-unset} ELECTRON_ENABLE_LOGGING=${ELECTRON_ENABLE_LOGGING-unset}" >> "$FAKE_CALLS"
    if [ "${ELECTRON_ENABLE_LOGGING-}" = 1 ]; then cat "$FAKE_OUTPUT"; else cat "$FAKE_OUTPUT_WITHOUT_LOG"; fi
    exit "$FAKE_STATUS" ;;
esac
exit 99
"""

# A host: whatever answers on the port it is given.
FAKE_HOST = """#!/bin/sh
exec python3 -m http.server "$PORT" --bind 127.0.0.1
"""


class RunSh(unittest.TestCase):
    """`run.sh` run whole, with a Cypress that prints what the test says."""

    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.dir = pathlib.Path(self.tmp.name)
        self.bin = self.dir / "bin"
        self.bin.mkdir()
        (self.dir / "dist" / "build").mkdir(parents=True)
        self.calls = self.dir / "calls.txt"
        self.write(self.bin / "pnpm", FAKE_PNPM)
        self.write(self.dir / "host", FAKE_HOST)
        with socket.socket() as s:
            s.bind(("127.0.0.1", 0))
            self.port = s.getsockname()[1]

    def tearDown(self):
        self.tmp.cleanup()

    @staticmethod
    def write(path, text):
        path.write_text(text)
        path.chmod(0o755)

    def run_sh(self, output=ACCEPTANCE, status=4, diagnostic=False, verify=0, **env_over):
        (self.dir / "output.txt").write_text(output)
        (self.dir / "output-without-log.txt").write_text(without(output, "SecurityError"))
        env = dict(os.environ)
        env.update(
            {
                "PATH": f"{self.bin}{os.pathsep}{env['PATH']}",
                "PORT": str(self.port - 143),
                "TODOMVC_DIST": str(self.dir / "dist"),
                "PW_DEV_SERVER": str(self.dir / "host"),
                "XVFB_BIN": "sh",
                "OUT": str(self.dir / "cypress-out.txt"),
                "DISPLAY": ":99",
                "WAYLAND_DISPLAY": "wayland-0",
                "FAKE_CALLS": str(self.calls),
                "FAKE_VERIFY": str(verify),
                "FAKE_STATUS": str(status),
                "FAKE_OUTPUT": str(self.dir / "output.txt"),
                "FAKE_OUTPUT_WITHOUT_LOG": str(self.dir / "output-without-log.txt"),
            }
        )
        env.pop("DIAGNOSTIC", None)
        if diagnostic:
            env["DIAGNOSTIC"] = "1"
        env.update(env_over)
        return subprocess.run(["bash", str(RUN_SH)], capture_output=True, text=True, env=env, cwd=ROOT, timeout=120)

    def calls_text(self):
        return self.calls.read_text() if self.calls.exists() else ""

    def test_the_acceptance_is_run_as_the_suite_ships_and_judged(self):
        r = self.run_sh()
        self.assertEqual(r.returncode, 0, r.stdout + r.stderr)
        self.assertIn("acceptance: 3 of 29 passed; 4 failed (3 tests, 1 hook); 22 not run", r.stdout)
        self.assertIn("the outcome is the one the app's design gives", r.stdout)
        calls = self.calls_text()
        self.assertIn("run --env framework=page/todomvc.app.Todos --config baseUrl=http://127.0.0.1:%d --reporter spec" % self.port, calls)
        # The display of the machine is left out, and the browser's console is on.
        self.assertIn("env DISPLAY=unset WAYLAND_DISPLAY=unset ELECTRON_ENABLE_LOGGING=1", calls)

    def test_the_diagnostic_is_run_under_another_name_and_says_so(self):
        r = self.run_sh(output=DIAGNOSTIC, status=0, diagnostic=True)
        self.assertEqual(r.returncode, 0, r.stdout + r.stderr)
        self.assertIn("the DIAGNOSTIC run (framework puremvc", r.stdout)
        self.assertIn("diagnostic: 28 of 29 passed; 1 pending", r.stdout)
        self.assertIn(
            "run --env framework=puremvc --config baseUrl=http://127.0.0.1:%d/page/todomvc.app.Todos# --reporter spec" % self.port,
            self.calls_text(),
        )

    def test_a_run_that_printed_no_results_fails(self):
        r = self.run_sh(output="Cypress crashed before it ran anything\n", status=1)
        self.assertEqual(r.returncode, 1, r.stdout)
        self.assertIn("did not run the suite", r.stdout)

    def test_an_outcome_the_design_does_not_give_fails(self):
        # The diagnostic's output, taken for the acceptance's: tests that ask
        # localStorage passed.
        r = self.run_sh(output=DIAGNOSTIC, status=0)
        self.assertEqual(r.returncode, 1, r.stdout)
        self.assertIn("passed, though it asks localStorage", r.stdout)

    def test_an_exit_status_that_is_not_the_failing_count_fails(self):
        r = self.run_sh(status=7)
        self.assertEqual(r.returncode, 1, r.stdout)
        self.assertIn("exited with status 7", r.stdout)

    def test_a_missing_xvfb_fails_and_runs_nothing(self):
        r = self.run_sh(XVFB_BIN="no-such-xvfb-for-this-test")
        self.assertEqual(r.returncode, 1, r.stdout)
        self.assertIn("Cypress cannot run: Xvfb is not installed", r.stdout)
        self.assertNotIn(" run ", self.calls_text())

    def test_a_missing_binary_fails_and_runs_nothing(self):
        r = self.run_sh(verify=1)
        self.assertEqual(r.returncode, 1, r.stdout)
        self.assertIn("Cypress cannot run: its binary is not installed", r.stdout)
        self.assertNotIn(" run ", self.calls_text())

    def test_a_build_that_is_missing_fails(self):
        (self.dir / "empty").mkdir()
        r = self.run_sh(TODOMVC_DIST=str(self.dir / "empty"))
        self.assertEqual(r.returncode, 1, r.stdout)
        self.assertIn("is not built", r.stdout)
        self.assertNotIn(" run ", self.calls_text())

    def test_a_host_that_is_missing_fails(self):
        r = self.run_sh(PW_DEV_SERVER=str(self.dir / "no-such-host"))
        self.assertEqual(r.returncode, 1, r.stdout)
        # Said at once, not after the host failed to answer for twenty seconds.
        self.assertIn("the host %s is not built" % (self.dir / "no-such-host"), r.stdout)
        self.assertNotIn("did not start", r.stdout)
        self.assertNotIn(" run ", self.calls_text())


if __name__ == "__main__":
    unittest.main()

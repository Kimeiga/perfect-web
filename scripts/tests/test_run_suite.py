"""`scripts/run_suite.sh`: a browser suite run for a recipe's evidence
records what the suite said and exits with its status (track
evidence-integrity-2, the integrator's ruling 6 of 2026-10-15). Run on a
stand-in Playwright that says what it is told, with Playwright's colours and
line erasures, and exits as it is told.
"""

from __future__ import annotations

import os
import pathlib
import subprocess
import tempfile
import unittest

ROOT = pathlib.Path(__file__).resolve().parents[2]
HELPER = ROOT / "scripts/run_suite.sh"

# A Playwright run's output, as the line reporter writes it to a pipe: a
# progress line erased, a failure, its error, the counts.
FAILED_RUN = (
    "\x1b[1A\x1b[2K[1/3] [webkit] › e2e/feed.spec.mjs:419:1 › Load more\n"
    "  1) [webkit] › e2e/feed.spec.mjs:419:1 › Load more shows the next page ──────\n"
    "\n"
    "    Error: \x1b[2mexpect(\x1b[22m\x1b[31mreceived\x1b[39m\x1b[2m).\x1b[22mtoBeGreaterThan()\n"
    "    Expected: > 3\n"
    "    Received:   3\n"
    "      at e2e/feed.spec.mjs:440:5\n"
    "\x1b[31m  1 failed\x1b[39m\n"
    "    [webkit] › e2e/feed.spec.mjs:419:1 › Load more shows the next page\n"
    "  10 did not run\n"
    "\x1b[32m  2 passed\x1b[39m\x1b[2m (31.2s)\x1b[22m\n"
)
PASSED_RUN = (
    "Running 3 tests using 3 workers\n"
    "  ✓  1 [chromium] › e2e/a.spec.mjs:3:1 › one (1.1s)\n"
    "  -  2 [chromium] › e2e/a.spec.mjs:9:1 › skipped here\n"
    "EVIDENCE activation-ms=12.50\n"
    "[WebServer] listening on 127.0.0.1:3141\n"
    "  1 skipped\n"
    "  2 passed (4.0s)\n"
)
SKIPPED_RUN = "Running 2 tests using 1 worker\n  2 skipped\n"

# The stand-in: says the file `$SAYS_<run>` names (or `$SAYS`), exits
# `$EXIT_<run>` (or `$EXIT`), counting its runs in `$COUNT`, and records
# where it ran.
STAND_IN = r"""#!/usr/bin/env bash
n=$(( $(cat "$COUNT" 2>/dev/null || echo 0) + 1 ))
echo "$n" > "$COUNT"
pwd > "$COUNT.where"
says="SAYS_$n"; exits="EXIT_$n"
cat "${!says:-$SAYS}"
echo "a line on stderr" >&2
exit "${!exits:-${EXIT:-0}}"
"""


class RunSuite(unittest.TestCase):
    def run_helper(self, args: list[str], says: str, exit: int = 0, **per_run: str) -> tuple[int, str, pathlib.Path]:
        d = pathlib.Path(tempfile.mkdtemp(prefix="pw-run-suite-"))
        (d / "spikes/own-renderer").mkdir(parents=True)
        (d / "benchmarks/harness").mkdir(parents=True)
        stand_in = d / "playwright"
        stand_in.write_text(STAND_IN)
        stand_in.chmod(0o755)
        (d / "says.txt").write_text(says)
        env = dict(os.environ, SAYS=str(d / "says.txt"), EXIT=str(exit), COUNT=str(d / "count"))
        for key, value in per_run.items():
            if key.startswith("SAYS_"):
                (d / f"{key}.txt").write_text(value)
                env[key] = str(d / f"{key}.txt")
            else:
                env[key] = value
        out = subprocess.run(
            ["bash", str(HELPER), *args, "--", str(stand_in), "test", "e2e/a.spec.mjs"],
            cwd=d,
            env=env,
            capture_output=True,
            text=True,
            timeout=60,
        )
        return out.returncode, out.stdout, d

    def test_a_failed_suite_fails_and_its_failure_is_recorded(self):
        status, out, _ = self.run_helper([], FAILED_RUN, exit=1)
        self.assertEqual(status, 1)
        self.assertEqual(
            out.splitlines(),
            [
                "  1) [webkit] › e2e/feed.spec.mjs:419:1 › Load more shows the next page ──────",
                "    Error: expect(received).toBeGreaterThan()",
                "    Expected: > 3",
                "    Received:   3",
                "  1 failed",
                "  10 did not run",
                "  2 passed (31.2s)",
                "  run_suite: exit 1",
            ],
        )

    def test_a_passed_suite_passes_and_keeps_its_tests_and_evidence_lines(self):
        # The control.
        status, out, d = self.run_helper([], PASSED_RUN)
        self.assertEqual(status, 0, out)
        self.assertEqual(
            out.splitlines(),
            [
                "  ✓  1 [chromium] › e2e/a.spec.mjs:3:1 › one (1.1s)",
                "  -  2 [chromium] › e2e/a.spec.mjs:9:1 › skipped here",
                "EVIDENCE activation-ms=12.50",
                "  1 skipped",
                "  2 passed (4.0s)",
            ],
        )
        self.assertEqual((d / "count.where").read_text().strip(), str(d / "spikes/own-renderer"))

    def test_its_status_is_any_exit_but_zero(self):
        status, out, _ = self.run_helper([], FAILED_RUN, exit=130)
        self.assertEqual(status, 130)
        self.assertIn("  run_suite: exit 130", out)

    def test_a_suite_that_passed_no_test_fails(self):
        status, out, _ = self.run_helper([], SKIPPED_RUN)
        self.assertEqual(status, 1)
        self.assertEqual(out.splitlines(), ["  2 skipped", "  run_suite: the suite passed no test", "  run_suite: exit 1"])

    def test_a_run_that_says_nothing_kept_shows_its_last_lines(self):
        crash = "node:internal/modules/cjs/loader:1228\n  throw err;\n  ^\nCannot find module 'x'\n"
        status, out, _ = self.run_helper([], crash, exit=1)
        self.assertEqual(status, 1)
        self.assertIn("  run_suite: the suite's last lines:", out)
        self.assertIn("    Cannot find module 'x'", out)
        self.assertTrue(out.endswith("  run_suite: exit 1\n"), out)

    def test_every_run_runs_and_the_first_failure_is_its_status(self):
        status, out, d = self.run_helper(["--runs", "3"], PASSED_RUN, SAYS_2=FAILED_RUN, EXIT_2="4")
        self.assertEqual(status, 4)
        self.assertEqual((d / "count").read_text().strip(), "3")
        lines = out.splitlines()
        self.assertEqual([l for l in lines if l.startswith("== run")], ["== run 1 of 3", "== run 2 of 3", "== run 3 of 3"])
        self.assertEqual(lines.count("  2 passed (4.0s)"), 2)
        self.assertIn("  1 failed", lines)
        self.assertEqual(lines.count("  run_suite: exit 4"), 1)
        # And every run passed, it passes, each run headed.
        status, out, _ = self.run_helper(["--runs", "2"], PASSED_RUN)
        self.assertEqual(status, 0, out)

    def test_it_runs_from_the_directory_it_is_given(self):
        status, _, d = self.run_helper(["-C", "benchmarks/harness"], PASSED_RUN)
        self.assertEqual(status, 0)
        self.assertEqual((d / "count.where").read_text().strip(), str(d / "benchmarks/harness"))

    def test_a_call_it_cannot_read_is_refused(self):
        for args in (["--runs", "0"], ["--runs", "00"], ["--runs", "000"], ["--runs", "x"], ["--bogus"]):
            with self.subTest(args=args):
                status, _, _ = self.run_helper(args, PASSED_RUN)
                self.assertEqual(status, 2)
        out = subprocess.run(["bash", str(HELPER), "--"], capture_output=True, text=True)
        self.assertEqual(out.returncode, 2)
        self.assertIn("no command", out.stderr)


if __name__ == "__main__":
    unittest.main()

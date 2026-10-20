"""`scripts/run_suite.sh`: a browser suite run for a recipe's evidence
records what the suite said and exits with its status (track
evidence-integrity-2, the integrator's ruling 6 of 2026-10-15). Run on a
stand-in Playwright that says what it is told, with Playwright's colours and
line erasures, and exits as it is told.
"""

from __future__ import annotations

import os
import pathlib
import shutil
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
# where it ran. As Playwright does, it clears `test-results/` as a run starts,
# and leaves in it, for a run, the failed test `$LEAVES_<run>` names, with a
# trace of `$LEAVES_BYTES` bytes (8) that says which run left it.
STAND_IN = r"""#!/usr/bin/env bash
n=$(( $(cat "$COUNT" 2>/dev/null || echo 0) + 1 ))
echo "$n" > "$COUNT"
pwd > "$COUNT.where"
rm -rf test-results
leaves="LEAVES_$n"
if [ -n "${!leaves:-}" ]; then
  mkdir -p "test-results/${!leaves}"
  { echo "run $n"; head -c "${LEAVES_BYTES:-8}" /dev/zero; } > "test-results/${!leaves}/trace.zip"
fi
says="SAYS_$n"; exits="EXIT_$n"
cat "${!says:-$SAYS}"
echo "a line on stderr" >&2
exit "${!exits:-${EXIT:-0}}"
"""


class RunSuite(unittest.TestCase):
    def run_helper(
        self,
        args: list[str],
        says: str,
        exit: int = 0,
        before=None,
        into: pathlib.Path | None = None,
        **per_run: str,
    ) -> tuple[int, str, pathlib.Path]:
        """Run the helper on the stand-in, in a tree of its own (or `into`,
        a tree an earlier call used, its run count started again); `before`
        is called with the tree first."""
        d = into or pathlib.Path(tempfile.mkdtemp(prefix="pw-run-suite-"))
        if into is None:
            self.addCleanup(shutil.rmtree, d, True)
        (d / "spikes/own-renderer").mkdir(parents=True, exist_ok=True)
        (d / "benchmarks/harness").mkdir(parents=True, exist_ok=True)
        (d / "count").unlink(missing_ok=True)
        stand_in = d / "playwright"
        stand_in.write_text(STAND_IN)
        stand_in.chmod(0o755)
        (d / "says.txt").write_text(says)
        if before is not None:
            before(d)
        env = dict(os.environ, SAYS=str(d / "says.txt"), EXIT=str(exit), COUNT=str(d / "count"))
        # Nothing of the environment this runs in (a recipe's name, a budget).
        env.pop("PW_RECIPE", None)
        env.pop("PW_KEPT_TRACES_KIB", None)
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

    # A failed run's output is kept, as the next run clears it (track
    # ci-diagnostics, 2026-10-19: train-10's Chromium failure in `e10-browser`'s
    # second run, whose trace its third run and a mutation control after it
    # cleared).

    def kept(self, d: pathlib.Path, suite: str = "spikes/own-renderer") -> dict[tuple[str, str, str], bytes]:
        """Everything kept in a suite's `kept-traces/`: (recipe, run, the
        file's path under the run) to what it holds; the call's tag is the
        directory between."""
        root = d / suite / "kept-traces"
        out = {}
        for path in sorted(root.rglob("*")) if root.is_dir() else []:
            if path.is_file():
                recipe, _tag, run, *rest = path.relative_to(root).parts
                out[(recipe, run, "/".join(rest))] = path.read_bytes()
        return out

    def test_a_run_that_fails_keeps_its_output_though_the_next_run_clears_it(self):
        # A run that fails, then one that passes: the first's output survives.
        status, out, d = self.run_helper(
            ["--runs", "2"], PASSED_RUN, SAYS_1=FAILED_RUN, EXIT_1="1", LEAVES_1="feed-load-more-webkit", PW_RECIPE="e10-browser"
        )
        self.assertEqual(status, 1)
        # The second run cleared `test-results/`, as Playwright does: without
        # the keeping, this is where the failure's trace was lost.
        self.assertFalse((d / "spikes/own-renderer/test-results").exists())
        kept = self.kept(d)
        self.assertEqual(list(kept), [("e10-browser", "run-1", "feed-load-more-webkit/trace.zip")])
        self.assertTrue(next(iter(kept.values())).startswith(b"run 1\n"))
        # And the evidence says where, before it says the run's exit.
        lines = out.splitlines()
        said = next(i for i, l in enumerate(lines) if l.startswith("  run_suite: kept 1 of 1 entries of its output"))
        self.assertEqual(lines[said + 1], "  run_suite: exit 1")
        self.assertIn("spikes/own-renderer/kept-traces/e10-browser/", lines[said])
        self.assertIn("/run-1", lines[said])

    def test_a_run_that_passes_keeps_nothing(self):
        # The control: what a passing run leaves in `test-results/` is not kept.
        status, out, d = self.run_helper(["--runs", "2"], PASSED_RUN, LEAVES_1="left-by-a-pass", PW_RECIPE="e10-browser")
        self.assertEqual(status, 0, out)
        self.assertFalse((d / "spikes/own-renderer/kept-traces").exists())
        self.assertNotIn("kept", out)

    def test_each_run_that_failed_is_kept_apart_from_the_others(self):
        status, _, d = self.run_helper(
            ["--runs", "3"],
            PASSED_RUN,
            SAYS_1=FAILED_RUN,
            SAYS_3=FAILED_RUN,
            EXIT_1="1",
            EXIT_3="1",
            LEAVES_1="first-chromium",
            LEAVES_2="second-passes",
            LEAVES_3="third-firefox",
            PW_RECIPE="e10-browser",
        )
        self.assertEqual(status, 1)
        kept = self.kept(d)
        self.assertEqual(
            sorted(kept),
            [("e10-browser", "run-1", "first-chromium/trace.zip"), ("e10-browser", "run-3", "third-firefox/trace.zip")],
        )
        self.assertTrue(kept[("e10-browser", "run-1", "first-chromium/trace.zip")].startswith(b"run 1\n"))
        self.assertTrue(kept[("e10-browser", "run-3", "third-firefox/trace.zip")].startswith(b"run 3\n"))

    def test_two_calls_in_a_recipe_keep_apart(self):
        # A recipe that runs a suite for each engine: the second call's `run-1`
        # does not replace the first's.
        _, _, d = self.run_helper([], FAILED_RUN, exit=1, LEAVES_1="chromium-test", PW_RECIPE="e14-x")
        self.run_helper([], FAILED_RUN, exit=1, into=d, LEAVES_1="firefox-test", PW_RECIPE="e14-x")
        root = d / "spikes/own-renderer/kept-traces/e14-x"
        calls = sorted(p for p in root.iterdir())
        self.assertEqual(len(calls), 2, calls)
        self.assertEqual(sorted(p.name for c in calls for p in (c / "run-1").iterdir()), ["chromium-test", "firefox-test"])

    def test_it_is_kept_under_the_recipe_that_ran_it_or_local(self):
        _, _, d = self.run_helper([], FAILED_RUN, exit=1, LEAVES_1="t")
        self.assertEqual([k[0] for k in self.kept(d)], ["local"])
        # A name that is no safe directory's is made one; it never leaves the directory.
        _, _, d = self.run_helper([], FAILED_RUN, exit=1, LEAVES_1="t", PW_RECIPE="../../x y")
        self.assertEqual([k[0] for k in self.kept(d)], ["_.._x_y"])
        self.assertEqual(list(d.glob("*/x*")), [])

    def test_the_benchmark_harness_keeps_under_its_own_directory(self):
        _, _, d = self.run_helper(["-C", "benchmarks/harness"], FAILED_RUN, exit=1, LEAVES_1="grade-chromium", PW_RECIPE="e14-contract")
        self.assertEqual(list(self.kept(d, "benchmarks/harness")), [("e14-contract", "run-1", "grade-chromium/trace.zip")])
        self.assertEqual(self.kept(d), {})

    def test_what_is_kept_is_bounded_and_the_rest_is_named(self):
        # A budget of 1 KiB, and a trace of 4 KB: not kept, and said.
        status, out, d = self.run_helper(
            [], FAILED_RUN, exit=1, LEAVES_1="big-webkit", LEAVES_BYTES="4000", PW_RECIPE="e10-browser", PW_KEPT_TRACES_KIB="1"
        )
        self.assertEqual(status, 1)
        self.assertEqual(list(self.kept(d)), [("e10-browser", "run-1", "NOT-KEPT.txt")])
        self.assertIn("big-webkit", next(iter(self.kept(d).values())).decode())
        self.assertIn("were over the 1.0 KiB a kept-traces holds", out)

    def test_a_failure_to_keep_does_not_change_the_status(self):
        # `kept-traces` is a file, so nothing can be kept under it: the suite's
        # status, 4, is still the helper's, and it says what failed.
        status, out, _ = self.run_helper(
            [],
            FAILED_RUN,
            exit=4,
            before=lambda d: (d / "spikes/own-renderer/kept-traces").write_text("a file"),
            LEAVES_1="t",
            PW_RECIPE="e10-browser",
        )
        self.assertEqual(status, 4, out)
        self.assertIn("could not keep the run's output", out)
        self.assertTrue(out.endswith("  run_suite: exit 4\n"), out)

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

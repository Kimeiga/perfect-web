"""`scripts/nightly_skip.py`: the nightly `verify` run is skipped where a
push's or an earlier nightly's run has succeeded at `master`'s exact head,
never on Sunday's schedule, and never where the GitHub API cannot say (track
ci-diagnostics, 2026-10-20).

In the seven days to 2026-10-20 the nightly used 691 runner-hours, about 100 a
night and 27% of all the CI there was, and ran every recipe again at a head
`master` had held since 2026-10-17. Its decision is one query of the API
(`runs?head_sha=...&status=success`), tested here on the answers the API
gives; the plan step of `verify.yml` that acts on it is run, as the runner
runs it, on stand-ins for the API and the planner.
"""

from __future__ import annotations

import importlib.util
import json
import os
import pathlib
import re
import shutil
import subprocess
import sys
import tempfile
import unittest

SCRIPTS = pathlib.Path(__file__).resolve().parent.parent
ROOT = SCRIPTS.parent
WORKFLOW = ROOT / ".github/workflows/verify.yml"
sys.path.insert(0, str(SCRIPTS))


def load(name: str):
    spec = importlib.util.spec_from_file_location(name, SCRIPTS / f"{name}.py")
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


skip = load("nightly_skip")

HEAD = "32757966" + "a" * 32
DAILY = "17 6 * * 1-6"


def run(event: str, conclusion: str = "success", id: int = 1) -> dict:
    return {"id": id, "event": event, "conclusion": conclusion, "html_url": f"https://github.com/o/r/actions/runs/{id}"}


class Decides(unittest.TestCase):
    def decide(self, runs, schedule=DAILY):
        return skip.decide(HEAD, schedule, lambda sha: runs)

    def test_a_pushs_run_that_succeeded_at_the_head_skips_the_nightly(self):
        # The landing train's own run, or master's push of it.
        skipped, why = self.decide([run("push", id=7)])
        self.assertTrue(skipped, why)
        self.assertTrue(why.startswith("skip: 32757966 was verified by the push's run 7 "), why)
        self.assertIn("https://github.com/o/r/actions/runs/7", why)

    def test_an_earlier_nightlys_run_skips_it_too(self):
        skipped, why = self.decide([run("schedule", id=9)])
        self.assertTrue(skipped, why)
        self.assertIn("the schedule's run 9", why)

    def test_the_first_covering_run_among_others_is_the_one_named(self):
        skipped, why = self.decide([run("workflow_dispatch", id=1), run("push", id=2), run("schedule", id=3)])
        self.assertTrue(skipped)
        self.assertIn("run 2", why)

    def test_a_dispatchs_run_does_not_verify_the_head(self):
        # A dispatch runs the recipes it names, which may be a few.
        skipped, why = self.decide([run("workflow_dispatch"), run("pull_request")])
        self.assertFalse(skipped, why)
        self.assertTrue(why.startswith("run: no push's or nightly's verify run has succeeded at 32757966"), why)

    def test_a_run_that_did_not_succeed_does_not_verify_it(self):
        # `status=success` leaves them out of the API's answer; a reply that
        # has one anyway is not trusted.
        for conclusion in ("failure", "cancelled", "skipped", "timed_out", None, ""):
            skipped, why = self.decide([run("push", conclusion=conclusion), run("schedule", conclusion=conclusion)])
            self.assertFalse(skipped, (conclusion, why))

    def test_no_run_at_the_head_runs_the_nightly(self):
        skipped, why = self.decide([])
        self.assertFalse(skipped)
        self.assertIn("no push's or nightly's verify run has succeeded", why)

    def test_the_weekly_run_is_never_skipped(self):
        for runs in ([run("push")], [run("schedule")], []):
            skipped, why = self.decide(runs, schedule=skip.WEEKLY)
            self.assertFalse(skipped, why)
            self.assertIn("the weekly full run", why)

    def test_any_other_schedule_is_asked(self):
        for schedule in (DAILY, "", "0 0 * * *", "17 6 * * *"):
            self.assertTrue(self.decide([run("push")], schedule=schedule)[0], schedule)

    def test_an_error_runs_the_nightly_never_skips_it(self):
        def broken(sha):
            raise subprocess.CalledProcessError(1, ["gh", "api"], stderr="HTTP 502")

        skipped, why = skip.decide(HEAD, DAILY, broken)
        self.assertFalse(skipped)
        self.assertIn("could not ask which runs passed at 32757966", why)
        self.assertIn("verifying in full", why)
        for bad in (KeyError("GITHUB_REPOSITORY"), json.JSONDecodeError("no", "", 0), FileNotFoundError("gh")):
            def raises(sha, bad=bad):
                raise bad

            self.assertFalse(skip.decide(HEAD, DAILY, raises)[0], bad)

    def test_the_events_that_verify_a_head_are_the_ones_a_push_to_master_trusts(self):
        # One rule for both: verified_base.py plans a push to master from the
        # newest commit these events' runs passed at.
        verified_base = load("verified_base")
        self.assertEqual(skip.COVERING, verified_base.COVERING)
        self.assertEqual(skip.COVERING, ("push", "schedule"))


class AsksTheApi(unittest.TestCase):
    """The command line, with `gh` on the path a stand-in that says what the
    API would, and records what it was asked."""

    def setUp(self):
        self.tmp = pathlib.Path(tempfile.mkdtemp(prefix="pw-nightly-skip-"))
        self.addCleanup(shutil.rmtree, self.tmp, True)
        bin_dir = self.tmp / "bin"
        bin_dir.mkdir()
        gh = bin_dir / "gh"
        gh.write_text(
            "#!/usr/bin/env bash\n"
            'echo "$*" >> "$GH_LOG"\n'
            '[ -n "${GH_FAIL:-}" ] && { echo "HTTP 502" >&2; exit 1; }\n'
            'printf "%s" "$GH_SAYS"\n'
        )
        gh.chmod(0o755)
        self.env = {
            **{k: v for k, v in os.environ.items() if k not in ("GH_FAIL", "GH_SAYS")},
            "PATH": f"{bin_dir}:{os.environ['PATH']}",
            "GH_LOG": str(self.tmp / "gh.log"),
            "GITHUB_REPOSITORY": "o/r",
        }

    def cli(self, *args, says="[]", fail=False):
        env = dict(self.env, GH_SAYS=says)
        if fail:
            env["GH_FAIL"] = "1"
        return subprocess.run(
            [sys.executable, str(SCRIPTS / "nightly_skip.py"), *args], capture_output=True, text=True, env=env, timeout=60
        )

    def test_it_asks_for_the_heads_successful_runs_of_verify(self):
        done = self.cli(HEAD, DAILY, says=json.dumps([run("push", id=5)]))
        self.assertEqual((done.returncode, done.stdout.startswith("skip: ")), (0, True), done.stdout + done.stderr)
        asked = (self.tmp / "gh.log").read_text().strip()
        self.assertIn("api repos/o/r/actions/workflows/verify.yml/runs", asked)
        self.assertIn(f"head_sha={HEAD}", asked)
        self.assertIn("status=success", asked)

    def test_its_exit_status_says_skip_run_or_misuse(self):
        self.assertEqual(self.cli(HEAD, DAILY, says="[]").returncode, 1)
        self.assertEqual(self.cli(HEAD, skip.WEEKLY, says=json.dumps([run("push")])).returncode, 1)
        self.assertEqual(self.cli().returncode, 2)
        self.assertEqual(self.cli(HEAD, DAILY, "extra").returncode, 2)

    def test_an_api_that_fails_or_answers_nonsense_runs_it(self):
        failed = self.cli(HEAD, DAILY, fail=True)
        self.assertEqual(failed.returncode, 1)
        self.assertIn("verifying in full", failed.stdout)
        nonsense = self.cli(HEAD, DAILY, says="<html>rate limited</html>")
        self.assertEqual(nonsense.returncode, 1)
        self.assertIn("verifying in full", nonsense.stdout)


class Workflow(unittest.TestCase):
    """`verify.yml`: two schedules, the plan step's branch for the nightly, and
    the override (read as text: a runner's Python need not have PyYAML)."""

    text = WORKFLOW.read_text()

    def test_the_schedules_are_weekdays_and_sunday_and_sunday_is_the_scripts_weekly(self):
        crons = re.findall(r'^\s*- cron: "([^"]+)"\s*$', self.text, re.M)
        self.assertEqual(crons, [DAILY, skip.WEEKLY])
        self.assertEqual(skip.WEEKLY, "17 6 * * 0")

    def test_the_plan_step_hands_the_schedule_that_fired_to_the_script(self):
        self.assertIn("          SCHEDULE: ${{ github.event.schedule }}", self.text)
        self.assertIn('python3 scripts/nightly_skip.py "$GITHUB_SHA" "$SCHEDULE"', self.text)

    def test_only_the_nightly_asks(self):
        script = plan_script()
        call = "python3 scripts/nightly_skip.py"
        self.assertEqual(script.count(call), 1)
        nightly = script[script.index("  schedule)") : script.index("  *)")]
        self.assertIn(call, nightly)
        # Not a push's plan, not a dispatch's: the override never asks.
        self.assertNotIn(call, script[: script.index("  schedule)")])
        self.assertNotIn(call, script[script.index("  *)") :])


def plan_script() -> str:
    """The plan step's `run:` block, dedented, as the runner runs it."""
    lines = WORKFLOW.read_text().splitlines()
    start = lines.index("      - id: plan")
    run_at = next(i for i in range(start, len(lines)) if lines[i] == "        run: |")
    out = []
    for line in lines[run_at + 1 :]:
        if line.strip() and not line.startswith("          "):
            break
        out.append(line[10:])
    return "\n".join(out) + "\n"


class PlanStep(unittest.TestCase):
    """The plan step run for each event, on stand-ins for the API (`gh`) and
    the planner, in a tree of its own: what it writes to `$GITHUB_OUTPUT`."""

    def setUp(self):
        self.tmp = pathlib.Path(tempfile.mkdtemp(prefix="pw-plan-step-"))
        self.addCleanup(shutil.rmtree, self.tmp, True)
        scripts = self.tmp / "scripts"
        scripts.mkdir()
        for name in ("nightly_skip.py", "verified_base.py"):
            shutil.copy(SCRIPTS / name, scripts / name)
        # The planner: says what it was asked, as the outputs it writes.
        (scripts / "ci_plan.py").write_text(
            "import sys\nprint('shards=PLANNED', ' '.join(sys.argv[1:]))\nprint('browsers=true')\n"
        )
        bin_dir = self.tmp / "bin"
        bin_dir.mkdir()
        gh = bin_dir / "gh"
        gh.write_text(
            '#!/usr/bin/env bash\n[ -n "${GH_FAIL:-}" ] && exit 1\nprintf "%s" "$GH_SAYS"\n'
        )
        gh.chmod(0o755)
        self.bin = bin_dir
        (self.tmp / "plan.sh").write_text(plan_script())

    def plan(self, event, schedule="", says="[]", fail=False, recipes=""):
        output = self.tmp / "output"
        summary = self.tmp / "summary"
        output.write_text("")
        summary.write_text("")
        env = {
            "PATH": f"{self.bin}:{os.environ['PATH']}",
            "HOME": str(self.tmp),
            "EVENT": event,
            "SCHEDULE": schedule,
            "BEFORE": "",
            "RECIPES": recipes,
            "SHARDS": "17",
            "GH_TOKEN": "x",
            "GITHUB_SHA": HEAD,
            "GITHUB_REF_NAME": "master",
            "GITHUB_REPOSITORY": "o/r",
            "GITHUB_OUTPUT": str(output),
            "GITHUB_STEP_SUMMARY": str(summary),
            "GH_SAYS": says,
        }
        if fail:
            env["GH_FAIL"] = "1"
        done = subprocess.run(["bash", str(self.tmp / "plan.sh")], cwd=self.tmp, env=env, capture_output=True, text=True, timeout=60)
        self.assertEqual(done.returncode, 0, done.stdout + done.stderr)
        return output.read_text().splitlines(), summary.read_text(), done.stdout

    PASSED = json.dumps([run("push", id=11)])

    def test_a_nightly_at_a_head_a_run_succeeded_at_plans_nothing_and_says_why(self):
        output, summary, log = self.plan("schedule", DAILY, says=self.PASSED)
        self.assertEqual(output, ["shards=[]", "browsers=false"])
        self.assertIn("skip: 32757966 was verified by the push's run 11", summary)
        self.assertIn("skip: 32757966", log)

    def test_a_nightly_at_a_head_none_has_is_planned_in_full(self):
        output, summary, log = self.plan("schedule", DAILY, says="[]")
        self.assertEqual(output, ["shards=PLANNED --shards 17 --all", "browsers=true"])
        self.assertEqual(summary, "")
        self.assertIn("run: no push's or nightly's verify run", log)

    def test_sundays_nightly_is_planned_in_full_though_the_head_was_verified(self):
        output, _, log = self.plan("schedule", skip.WEEKLY, says=self.PASSED)
        self.assertEqual(output, ["shards=PLANNED --shards 17 --all", "browsers=true"])
        self.assertIn("the weekly full run", log)

    def test_a_nightly_whose_api_fails_is_planned_in_full(self):
        output, _, log = self.plan("schedule", DAILY, fail=True)
        self.assertEqual(output, ["shards=PLANNED --shards 17 --all", "browsers=true"])
        self.assertIn("verifying in full", log)

    def test_a_dispatch_is_planned_whatever_has_passed(self):
        # The override: every recipe with none named, or the recipes named.
        output, _, log = self.plan("workflow_dispatch", says=self.PASSED)
        self.assertEqual(output, ["shards=PLANNED --shards 17 --all", "browsers=true"])
        self.assertNotIn("skip", log)
        output, _, _ = self.plan("workflow_dispatch", says=self.PASSED, recipes="e14-a e14-b")
        self.assertEqual(output, ["shards=PLANNED --shards 17 e14-a e14-b", "browsers=true"])


if __name__ == "__main__":
    unittest.main()

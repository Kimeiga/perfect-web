"""`scripts/suite_status.py`: every browser-suite invocation in the
justfile's recipes is run by `scripts/run_suite.sh`, its status its
recipe's (track evidence-integrity-2, the integrator's ruling 6 of
2026-10-15). Held on the repository, with controls: justfiles of its own
under the real one's settings, each a way a status was or could be
swallowed.
"""

from __future__ import annotations

import pathlib
import sys
import tempfile
import unittest

ROOT = pathlib.Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / "scripts"))

import suite_status  # noqa: E402

REAL = (ROOT / "justfile").read_text()
SETTINGS = REAL[: REAL.index("# The parallel tracks' recipes")]

SUITE = "bash scripts/run_suite.sh -- pnpm exec playwright test e2e/a.spec.mjs --reporter=line"


def recipe(lines: list[str], name: str = "e1-x", settings: str = SETTINGS) -> str:
    """A justfile of one recipe, its lines continued as one evidence group,
    under `settings`."""
    body = "".join(f"       {line} \\\n" for line in lines)
    return f"{settings}\n{name}:\n    @{{ echo x; \\\n{body}     }} > docs/evidence/E14/x.txt\n"


def judged(text: str) -> tuple[list[str], str]:
    with tempfile.TemporaryDirectory(prefix="pw-suite-status-") as d:
        root = pathlib.Path(d)
        (root / "justfile").write_text(text)
        return suite_status.report(root)


class SuiteStatus(unittest.TestCase):
    def assertPasses(self, lines: list[str], **kw) -> None:
        refusals, summary = judged(recipe(lines, **kw))
        self.assertEqual(refusals, [], summary)

    def assertRefused(self, lines: list[str], why: str, **kw) -> None:
        refusals, summary = judged(recipe(lines, **kw))
        self.assertTrue(any(why in r for r in refusals), f"{why!r} not among {refusals}")

    def test_the_repository_runs_every_suite_by_the_helper_its_status_its_recipes(self):
        refusals, summary = suite_status.report()
        print(f"\n{summary}", file=sys.stderr)
        self.assertEqual(refusals, [], "\n".join(refusals))
        found, occurrences = suite_status.invocations()
        # Non-vacuity: every occurrence recognised, and more than the 80 of
        # 2026-10-15; the benchmark's controls too.
        self.assertGreater(occurrences, 80)
        self.assertEqual(sum(1 for i in found if i.kind == "playwright" and not i.problems), occurrences)
        self.assertGreaterEqual(sum(1 for i in found if i.kind == "bench"), 2)

    def test_the_helper_with_its_status_raised_passes(self):
        # The control: each form the recipes use.
        self.assertPasses(["echo; echo '== the browser'; echo;", f"{SUITE};", "echo done;"])
        self.assertPasses([f"PW_PERFORMANCE=1 PORT=3141 {SUITE.replace('-- ', '--runs {{RUNS}} -- ')};"])
        self.assertPasses(["for e in webkit chromium; do", f"  {SUITE} --project=$e;", "done;"])
        self.assertPasses(
            [
                "bash scripts/run_suite.sh --runs 3 -- pnpm exec playwright test --reporter=line",
                "  $(for e in chromium firefox; do printf -- '--project=%s ' \"$e\"; done);",
            ]
        )
        self.assertPasses(["bash scripts/run_suite.sh -C benchmarks/harness -- pnpm exec playwright test --repeat-each=3;"])
        self.assertPasses(["cargo build -p pw-dev-server &&", f"{SUITE};"])
        self.assertPasses([f"{SUITE};", "(cd benchmarks/harness && node bench.mjs controls --task T08 2>&1);"])

    def test_a_suite_not_run_by_the_helper_is_refused(self):
        self.assertRefused(
            ["(cd spikes/own-renderer && pnpm exec playwright test e2e/a.spec.mjs --reporter=line 2>&1);"],
            "is not run by `scripts/run_suite.sh`",
        )
        self.assertRefused(
            ["bash scripts/run_suite.sh pnpm exec playwright test e2e/a.spec.mjs;"],
            "is not the command `scripts/run_suite.sh` runs, after its `--`",
        )
        # Hidden in a string: not recognised, so the count refuses it.
        self.assertRefused(['bash -c "pnpm exec playwright test e2e/a.spec.mjs";'], "1 of 1 `playwright test` not run")

    def test_each_way_of_swallowing_a_status_is_refused(self):
        for lines, why in (
            # The pattern of 2026-10-15.
            (
                [
                    "(cd spikes/own-renderer && pnpm exec playwright test e2e/a.spec.mjs --reporter=line 2>&1)",
                    "  | sed 's/x//' | grep -E 'passed|failed' || true;",
                ],
                "the subshell holding it is followed by `|`",
            ),
            ([f"{SUITE} || true;"], "it is followed by `||`"),
            ([f"{SUITE} || echo failed;"], "it is followed by `||`"),
            ([f"{SUITE} && echo ok;"], "it is followed by `&&`"),
            ([f"{SUITE} | tee out.txt;"], "it is followed by `|`"),
            ([f"{SUITE} &"], "it is followed by `&`"),
            ([f"{SUITE}; true;"], "it is followed by `; true`"),
            ([f"{SUITE}; :;"], "it is followed by `; :`"),
            ([f"! {SUITE};"], "it is negated by `!`"),
            ([f"if {SUITE}; then echo ok; fi;"], "it is a condition of `if`"),
            ([f"while {SUITE}; do break; done;"], "it is a condition of `while`"),
            ([f"if true; then echo; elif {SUITE}; then echo; fi;"], "it is a condition of `elif`"),
            ([f"( {SUITE} ) || true;"], "the subshell holding it is followed by `||`"),
            ([f"{{ {SUITE}; echo; }} || true;"], "the group holding it is followed by `||`"),
            (["for e in a b; do", f"  {SUITE};", "done || true;"], "the loop holding it is followed by `||`"),
            # Within quotes it is not read as a command: the count refuses it.
            ([f'echo "$({SUITE})";'], "1 of 1 `playwright test` not run"),
            ([f"out=$({SUITE});"], "it runs inside a command substitution"),
            (["set +e;", f"{SUITE};", "set -e;"], "turns errexit off"),
            (["(cd benchmarks/harness && node bench.mjs controls --task T08 2>&1) || true;"], "followed by `||`"),
        ):
            with self.subTest(lines=lines):
                self.assertRefused(lines, why)

    def test_a_recipe_or_a_shell_that_would_not_stop_is_refused(self):
        # `just`'s `-` prefix runs a line ignoring its status.
        text = recipe([f"{SUITE};"]).replace("    @{ echo x;", "    -@{ echo x;")
        refusals, _ = judged(text)
        self.assertTrue(any("`just` ignores its status" in r for r in refusals), refusals)
        # A shebang recipe is a script, not the justfile's shell.
        text = f"{SETTINGS}\ne1-x:\n    #!/usr/bin/env bash\n    {SUITE}\n"
        refusals, _ = judged(text)
        self.assertTrue(any("a script of its own" in r for r in refusals), refusals)
        # A justfile whose shell does not stop at a failure.
        for shell, why in (
            ('set shell := ["bash", "-uo", "pipefail", "-c"]', "without `-e`"),
            ('set shell := ["bash", "-eu", "-c"]', "without `pipefail`"),
            ('set shell := ["sh", "-eu", "-c"]', "not bash"),
        ):
            with self.subTest(shell=shell):
                settings = SETTINGS.replace('set shell := ["bash", "-euo", "pipefail", "-c"]', shell)
                self.assertNotEqual(settings, SETTINGS)
                self.assertRefused([f"{SUITE};"], why, settings=settings)

    def test_a_justfile_with_no_suite_is_refused_as_read_nothing(self):
        refusals, _ = judged(recipe(["echo nothing;"]))
        self.assertEqual(refusals, ["no `playwright test` in the recipes' lines: the gate read nothing"])


if __name__ == "__main__":
    unittest.main()

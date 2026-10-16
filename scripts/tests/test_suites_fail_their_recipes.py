"""**A failed browser suite fails its recipe, and its evidence says so**
(track evidence-integrity-2, the integrator's ruling 6 of 2026-10-15).

Until then 83 of the 85 recipes that ran a browser suite ended it `|| true`:
a failed test failed neither its recipe nor the run, and its evidence said
"1 failed" in a green run. Every recipe that runs a browser suite, by
`scripts/run_suite.sh` (Playwright) or by the benchmark's `bench.mjs
controls`, is run here as `test_builds_before_suites.py` runs it: by `just`
under the justfile's own settings, in a tree of its own where each program
it calls is a stand-in that logs its call. Each suite call, failed in turn
(the stand-in says "1 failed" and exits 9), fails the recipe, and an
evidence file the recipe wrote records it. `scripts/suite_status.py` reads
the same from the recipes' text (`test_suite_status.py`).
"""

from __future__ import annotations

import concurrent.futures
import pathlib
import re
import sys
import tempfile
import unittest

HERE = pathlib.Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
sys.path.insert(0, str(HERE.parent))

import ci_plan  # noqa: E402
from test_builds_before_suites import ROOT, Call, run_full, shown  # noqa: E402

# What a failed suite's stand-in says: a count the helper keeps.
FAILED = "  1 failed (the stand-in's)"


def is_suite_call(call: Call) -> bool:
    """A browser suite's run: Playwright's, wherever it runs from, or the
    benchmark's controls."""
    name, _, args = call
    return (
        (name == "pnpm" and args.startswith("exec playwright test"))
        or (name == "npx" and args.startswith("playwright test"))
        or (name == "node" and args.startswith("bench.mjs controls"))
    )


def recipes(source: pathlib.Path = ROOT) -> list[str]:
    """Every recipe that runs a browser suite, in every part of the
    justfile."""
    return sorted(
        n for n, b in ci_plan.bodies(source).items() if re.search(r"playwright\s+test|bench\.mjs\s+controls", b)
    )


def problems(recipe: str, source: pathlib.Path = ROOT, tried: list | None = None) -> list[str]:
    """What is wrong with `recipe`'s suites' status: each suite call failed
    in turn is added to `tried`."""
    status, calls, said, _ = run_full(recipe, source=source)
    at = [k for k, c in enumerate(calls, 1) if is_suite_call(c)]
    if status != 0 or not at:
        return [f"{recipe}: its control run reached no suite (exit {status}): {said.strip()[-300:]}"]
    out = []
    for k in at:
        status, _, _, evidence = run_full(recipe, fail_at=k, source=source, fail_says=FAILED)
        if tried is not None:
            tried.append((recipe, k))
        suite = shown(calls[k - 1])
        if status == 0:
            out.append(f"{recipe}: its suite `{suite}` failed, and the recipe passed")
        if not any(FAILED.strip() in text for text in evidence.values()):
            out.append(f"{recipe}: its suite `{suite}` failed, and no evidence it wrote says so")
    return out


class SuitesFailTheirRecipes(unittest.TestCase):
    def test_the_stand_ins_tell_a_recipe_its_suite_fails_from_one_that_swallows_it(self):
        # The control for the test below, on recipes of its own under the
        # justfile's own settings: the helper's status stops its recipe and
        # its evidence keeps the count; `|| true`, a subshell piped to a
        # filter and swallowed, and a status read as a condition do not stop
        # it; a filter that drops the count keeps no evidence of the failure.
        real = (ROOT / "justfile").read_text()
        settings = real[: real.index("# The parallel tracks' recipes")]
        build = (
            "    @BUILD_ONLY=1 bash spikes/own-renderer/run.sh > /dev/null\n"
            "    @cargo build --quiet --locked -p pw-dev-server\n"
        )
        recipes_text = (
            "e1-helper:\n" + build + "    @{ echo x; \\\n"
            "       bash scripts/run_suite.sh -- pnpm exec playwright test a.spec.mjs --reporter=line; \\\n"
            "     } > docs/evidence/E14/helper.txt\n"
            "e1-swallowed:\n" + build + "    @{ echo x; \\\n"
            "       bash scripts/run_suite.sh -- pnpm exec playwright test a.spec.mjs --reporter=line || true; \\\n"
            "     } > docs/evidence/E14/swallowed.txt\n"
            "e1-old:\n" + build + "    @{ echo x; \\\n"
            "       (cd spikes/own-renderer && pnpm exec playwright test a.spec.mjs --reporter=line 2>&1) \\\n"
            "         | grep -E \"passed|failed\" || true; \\\n"
            "     } > docs/evidence/E14/old.txt\n"
            "e1-condition:\n" + build + "    @{ echo x; \\\n"
            "       if bash scripts/run_suite.sh -- pnpm exec playwright test a.spec.mjs; then echo ok; fi; \\\n"
            "     } > docs/evidence/E14/condition.txt\n"
            "e1-unrecorded:\n" + build + "    @{ echo x; \\\n"
            "       (cd spikes/own-renderer && pnpm exec playwright test a.spec.mjs 2>&1) | grep -c passed; \\\n"
            "     } > docs/evidence/E14/unrecorded.txt\n"
        )
        with tempfile.TemporaryDirectory(prefix="pw-suites-control-") as d:
            source = pathlib.Path(d)
            (source / "justfile").write_text(settings + recipes_text)
            self.assertEqual(
                recipes(source), ["e1-condition", "e1-helper", "e1-old", "e1-swallowed", "e1-unrecorded"]
            )
            self.assertEqual(problems("e1-helper", source), [])
            self.assertEqual(
                problems("e1-swallowed", source),
                ["e1-swallowed: its suite `pnpm exec playwright test a.spec.mjs --reporter=line` failed, and the recipe passed"],
            )
            self.assertEqual(
                problems("e1-old", source),
                ["e1-old: its suite `pnpm exec playwright test a.spec.mjs --reporter=line` failed, and the recipe passed"],
            )
            self.assertEqual(
                problems("e1-condition", source),
                ["e1-condition: its suite `pnpm exec playwright test a.spec.mjs` failed, and the recipe passed"],
            )
            self.assertEqual(
                problems("e1-unrecorded", source),
                ["e1-unrecorded: its suite `pnpm exec playwright test a.spec.mjs` failed, and no evidence it wrote says so"],
            )

    def test_every_browser_suite_fails_its_recipe_and_its_evidence_says_so(self):
        names = recipes()
        self.assertGreater(len(names), 80)
        tried: list = []
        with concurrent.futures.ThreadPoolExecutor(max_workers=8) as pool:
            found = [p for ps in pool.map(lambda n: problems(n, tried=tried), names) for p in ps]
        print(
            f"\nsuites fail their recipes: {len(names)} recipes run a browser suite; "
            f"{len(tried)} suite runs, each failed in turn",
            file=sys.stderr,
        )
        self.assertEqual(found, [], "\n".join(found))
        # Each recipe had at least one suite failed.
        self.assertEqual(sorted({r for r, _ in tried}), names)


if __name__ == "__main__":
    unittest.main()

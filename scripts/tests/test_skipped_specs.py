"""**Every spec an optional build can leave out runs where that build is
made** (the integrator's ruling of 2026-10-19, train-11;
`scripts/skipped_specs.py`): a spec left out of a run for want of its build
is said, not refused, so at least one recipe must make the build and run
the spec, or it goes unrun everywhere."""

import json
import pathlib
import sys
import unittest

ROOT = pathlib.Path(__file__).resolve().parent.parent.parent
sys.path.insert(0, str(ROOT / "scripts"))
import ci_plan  # noqa: E402
import skipped_specs  # noqa: E402


def registry():
    return json.loads(skipped_specs.REGISTRY.read_text())


class TheRepository(unittest.TestCase):
    def test_every_spec_an_optional_build_can_leave_out_runs_where_that_build_is_made(self):
        self.assertEqual(skipped_specs.unrun(registry(), ci_plan.bodies()), [])

    def test_the_specs_it_holds_are_the_registrys(self):
        # Not nothing: the guestbook's and kiokun's, which CI's browser jobs
        # do not build, a feed spec, and the spec whose tests annotate their
        # hosts, by what they annotate.
        found = skipped_specs.skippable(registry())
        self.assertEqual(found["guestbook.spec.mjs"], ["guestbook"])
        self.assertEqual(found["kiokun-site.spec.mjs"], ["kiokun"])
        self.assertEqual(found["feed.spec.mjs"], ["feed"])
        self.assertIn("page-policy.spec.mjs", found)
        self.assertIn("kiokun", found["page-policy.spec.mjs"])
        # And none the required build serves alone, nor a build with none.
        self.assertNotIn("cart.spec.mjs", found)
        self.assertNotIn("kiokun.spec.mjs", found)


# A registry of one optional build, the feed's, and one required, the
# store's; one spec of each.
REGISTRY = {
    "roles": {
        "base": {"build": "store", "offsets": {"shared": 0}},
        "feed": {"build": "feed", "offsets": {"chromium": 60}},
    },
    "builds": {
        "store": {"dist": "dist", "prerequisite": "run.sh"},
        "feed": {"dist": "dist-feed", "prerequisite": "feed.sh"},
    },
    "specs": {
        "feed.spec.mjs": {"roles": ["feed"]},
        "cart.spec.mjs": {"roles": ["base"]},
    },
}
MAKES = "@bash spikes/own-renderer/feed.sh > /dev/null\n"
RUNS = "bash scripts/run_suite.sh -- pnpm exec playwright test e2e/feed.spec.mjs --reporter=line\n"
EVERY = "bash scripts/run_suite.sh --runs 3 -- pnpm exec playwright test --reporter=line\n"
OTHER = "bash scripts/run_suite.sh -- pnpm exec playwright test e2e/cart.spec.mjs --reporter=line\n"
NAMED = ["e2e/feed.spec.mjs needs dist-feed: no recipe runs feed.sh and the spec"]


class TheCheck(unittest.TestCase):
    def test_a_spec_run_where_its_build_is_not_made_is_named(self):
        self.assertEqual(skipped_specs.unrun(REGISTRY, {"a": RUNS}), NAMED)

    def test_a_build_made_where_the_spec_is_not_run_is_named(self):
        self.assertEqual(skipped_specs.unrun(REGISTRY, {"a": MAKES}), NAMED)
        self.assertEqual(skipped_specs.unrun(REGISTRY, {"a": MAKES + OTHER}), NAMED)

    def test_the_build_and_the_run_in_two_recipes_are_named(self):
        self.assertEqual(skipped_specs.unrun(REGISTRY, {"a": RUNS, "b": MAKES}), NAMED)

    def test_one_recipe_that_makes_the_build_and_runs_the_spec_holds_it(self):
        self.assertEqual(skipped_specs.unrun(REGISTRY, {"a": MAKES + RUNS}), [])
        self.assertEqual(skipped_specs.unrun(REGISTRY, {"a": MAKES + EVERY}), [])


if __name__ == "__main__":
    unittest.main()

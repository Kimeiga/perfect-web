"""A failed or missing benchmark trial must never count as a speedup."""
import unittest
from linker_choice import medians, VARIANTS, ROUNDS


class Controls(unittest.TestCase):
    def test_every_linker_gets_three_trials(self):
        for name in VARIANTS:
            self.assertEqual(sum(name in batch for batch in ROUNDS), 3)

    def test_incomplete_candidate_is_not_ranked(self):
        rows = [{"name": f"mold-build-{n}", "kind": "build", "variant": "mold",
                 "seconds": 1.0, "exit_code": 0} for n in (1,2)]
        self.assertIsNone(medians(rows)["mold"])

    def test_failed_trial_is_not_ranked(self):
        rows = [{"name": f"mold-build-{n}", "kind": "build", "variant": "mold",
                 "seconds": 0.01, "exit_code": 1 if n == 2 else 0} for n in (1,2,3)]
        self.assertIsNone(medians(rows)["mold"])

    def test_median_uses_three_successful_values(self):
        rows = [{"name": f"mold-build-{n}", "kind": "build", "variant": "mold",
                 "seconds": t, "exit_code": 0} for n,t in enumerate((3,8,4), 1)]
        self.assertEqual(medians(rows)["mold"], 4)

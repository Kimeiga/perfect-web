"""Historical regression metadata cannot silently turn a proposal into proof."""

import unittest

from scripts.quality_regressions import validate


def case(identifier, replay=None):
    return {
        "id": identifier,
        "kind": "format",
        "source": "https://github.com/example/project/pull/1",
        "bad": "a" * 40,
        "fixed": "b" * 40,
        "replay": replay,
    }


class HistoricalCases(unittest.TestCase):
    def test_valid_data(self):
        self.assertEqual(1, len(validate({"schema": 1, "cases": [case("a")]})))

    def test_duplicate_case_is_rejected(self):
        with self.assertRaises(ValueError):
            validate({"schema": 1, "cases": [case("same"), case("same")]})

    def test_cannot_replay_shell_script(self):
        with self.assertRaises(ValueError):
            validate({"schema": 1, "cases": [case("a", ["bash", "-c", "true"])]})

    def test_partial_commit_hash_is_not_replay_proof(self):
        broken = case("a")
        broken["fixed"] = "c" * 9
        with self.assertRaises(ValueError):
            validate({"schema": 1, "cases": [broken]})

    def test_no_fixed_sha_can_be_documented(self):
        documented = case("a")
        documented["fixed"] = None
        self.assertEqual(1, len(validate({"schema": 1, "cases": [documented]})))

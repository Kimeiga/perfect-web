"""Proof controls for the Rust accelerator benchmark evidence parser."""
import json
import unittest

from rust_speed import inventory, parse_time, median


class TestBench(unittest.TestCase):
    def test_time_and_rss(self):
        self.assertEqual((1.25, 120000), parse_time("BENCH_TIME:1.25:120000"))
        self.assertEqual((None, None), parse_time("no timing"))

    def test_inventory_matches_multisets(self):
        fixture = {"rust-suites": {
            "one": {"status": "listed", "testcases": {
                "x": {"filter-match": {"status": "matches"}}}},
            "two": {"status": "listed", "testcases": {
                "x": {"filter-match": {"status": "matches"}},
                "y": {"filter-match": {"status": "matches"}}}},
        }}
        self.assertTrue(inventory("x: test\nx: test\ny: test", json.dumps(fixture))["equal"])

    def test_test_omission_is_a_failure(self):
        fixture = {"rust-suites": {
            "one": {"status": "listed", "testcases": {
                "x": {"filter-match": {"status": "matches"}}}},
        }}
        self.assertFalse(inventory("x: test\ny: test", json.dumps(fixture))["equal"])

    def test_ignored_case_is_listed_but_not_runnable(self):
        fixture = {"rust-suites": {
            "one": {"status": "listed", "testcases": {
                "normal": {"ignored": False, "filter-match": {"status": "matches"}},
                "heavy": {"ignored": True, "filter-match": {
                    "status": "mismatch", "reason": "ignored"}},
            }},
        }}
        result = inventory("heavy: test\\nnormal: test".replace("\\n", "\n"), json.dumps(fixture))
        self.assertTrue(result["equal"])
        self.assertEqual(result["nextest_ignored"], 1)
        self.assertEqual(result["nextest_runnable"], 1)

    def test_zero_tests_refused(self):
        self.assertFalse(inventory("", json.dumps({"rust-suites": {}}))["equal"])

    def test_failed_trials_do_not_improve_median(self):
        rows = [{"name": "cargo_run_1", "exit_code": 0, "seconds": 4},
                {"name": "cargo_run_2", "exit_code": 1, "seconds": 0.1},
                {"name": "cargo_run_3", "exit_code": 0, "seconds": 6}]
        self.assertEqual(5, median(rows, "cargo_run_"))

"""Cargo result validation shared by bounded, generic and guide producers."""
import sys
from pathlib import Path
import unittest

sys.path.insert(0, str(Path(__file__).resolve().parent.parent))
from cargo_test_results import parse

OK = "test result: ok. 2 passed; 0 failed; 0 ignored\n"
FAIL = "test witness ... FAILED\ntest result: FAILED. 1 passed; 1 failed; 0 ignored\n"


class CargoResults(unittest.TestCase):
    def test_only_real_cargo_exits_match_the_result(self):
        for status in [-9, 137, 67, 1, 0, True]:
            with self.subTest(status=status):
                self.assertFalse(parse(status, FAIL).valid)
        self.assertEqual(parse(101, FAIL)[:4], (True, 1, 1, ("witness",)))
        self.assertEqual(parse(0, OK)[:3], (True, 2, 0))

    def test_every_binary_contributes(self):
        self.assertEqual(parse(101, OK + FAIL)[:3], (True, 3, 1))

    def test_a_build_or_stopped_binary_is_not_a_kill(self):
        for error in ["error[E0308]: mismatch", "could not compile target", "signal: 9, SIGKILL: kill", "memory bound: process stopped"]:
            with self.subTest(error=error):
                self.assertFalse(parse(101, FAIL + error).valid)

    def test_empty_missing_and_contradictory_results_refuse(self):
        for output in ["", "test result: ok. 0 passed; 0 failed;", "test result: ok. 1 passed; 1 failed;", "test result: FAILED. 1 passed; 0 failed;", "test witness ... FAILED\n" + OK]:
            with self.subTest(output=output):
                self.assertFalse(parse(0, output).valid)

    def test_color_and_quiet_failure_names_are_diagnostics(self):
        result = parse(101, "\x1b[31m---- witness stdout ----\x1b[0m\n" + FAIL)
        self.assertEqual(result.failure_names, ("witness",))


    def test_empty_successful_siblings_preserve_a_nonempty_command(self):
        empty = "test result: ok. 0 passed; 0 failed; 0 ignored\n"
        for output in [empty + OK, OK + empty, empty + OK + empty]:
            with self.subTest(output=output):
                self.assertEqual(parse(0, output)[:3], (True, 2, 0))
        self.assertEqual(parse(101, empty + FAIL + empty)[:4], (True, 1, 1, ("witness",)))

    def test_empty_aggregate_refuses_even_with_multiple_siblings(self):
        empty = "test result: ok. 0 passed; 0 failed; 9 ignored\n"
        for status in [0, 101]:
            for output in [empty, empty + empty]:
                with self.subTest(status=status, output=output):
                    self.assertFalse(parse(status, output).valid)

    def test_empty_siblings_do_not_hide_contradictions_build_errors_or_exits(self):
        empty = "test result: ok. 0 passed; 0 failed; 0 ignored\n"
        contradictory = "test result: FAILED. 0 passed; 0 failed; 0 ignored\n"
        cases = [(0, OK + contradictory), (101, empty + OK),
                 (0, empty + FAIL), (137, empty + FAIL),
                 (101, empty + FAIL + "could not compile target"),
                 (101, empty + FAIL + "\nsignal: 9, SIGKILL: kill")]
        for status, output in cases:
            with self.subTest(status=status, output=output):
                self.assertFalse(parse(status, output).valid)



if __name__ == "__main__":
    unittest.main()

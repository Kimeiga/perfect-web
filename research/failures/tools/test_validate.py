"""Mutation tests for inventory structure, not for Pleris application semantics."""
import unittest
from pathlib import Path
from validate import ROOT, load_census, parse_census, validate, validate_obligations


class CensusValidationTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.text = load_census()
        cls.obligations = (ROOT / "OBLIGATIONS.md").read_text()
        cls.data = parse_census(cls.text)
        cls.ids = {row["id"] for row in cls.data["failures"]}

    def test_repository_inventory_is_consistent(self):
        self.assertGreater(len(validate()["data"]["failures"]), 0)

    def test_duplicate_leaf_is_rejected(self):
        row = next(line for line in self.text.splitlines() if line.startswith("| A01 |"))
        with self.assertRaisesRegex(ValueError, "duplicate ID"):
            parse_census(self.text.replace(row, row + "\n" + row, 1))

    def test_missing_audited_revision_is_rejected(self):
        with self.assertRaisesRegex(ValueError, "missing audited"):
            parse_census(self.text.replace(self.data["audited_revision"], "unknown"))

    def test_malformed_id_cannot_disappear_from_inventory(self):
        with self.assertRaisesRegex(ValueError, "outside its group"):
            parse_census(self.text.replace("| A01 |", "| A1 |", 1))

    def test_unknown_status_is_rejected(self):
        with self.assertRaisesRegex(ValueError, "unknown coverage"):
            parse_census(self.text.replace("| partially covered |", "| perfect |", 1))

    def test_invalid_enforcement_layer_is_rejected(self):
        with self.assertRaisesRegex(ValueError, "invalid enforcement"):
            parse_census(self.text.replace("| B |", "| Z |", 1))

    def test_missing_evidence_reference_is_rejected(self):
        with self.assertRaisesRegex(ValueError, "unresolved evidence"):
            parse_census(self.text.replace("[R2] [R3] [S58]", "[S9999]", 1))

    def test_missing_cell_is_rejected(self):
        with self.assertRaisesRegex(ValueError, "expected 10 cells"):
            parse_census(self.text.replace("| A01 |", "| A01 | extra |", 1))

    def test_leaf_outside_group_is_rejected(self):
        with self.assertRaisesRegex(ValueError, "outside its group"):
            parse_census(self.text.replace("| A01 |", "| Z01 |", 1))

    def test_obligation_unknown_leaf_is_rejected(self):
        with self.assertRaisesRegex(ValueError, "unresolved census leaf"):
            validate_obligations(self.obligations.replace("| A01 |", "| Z99 |", 1), self.ids)

    def test_duplicate_obligation_is_rejected(self):
        row = next(line for line in self.obligations.splitlines() if line.startswith("| CF01 |"))
        with self.assertRaisesRegex(ValueError, "duplicate CF01"):
            validate_obligations(self.obligations + "\n" + row, self.ids)

    def test_source_and_plan_lanes_are_explicit(self):
        with self.assertRaisesRegex(ValueError, "source/plan lane"):
            validate_obligations(self.obligations.replace("| CF01 | S |", "| CF01 | magic |"), self.ids)

    def test_empty_corpus_is_not_a_pass(self):
        with self.assertRaisesRegex(ValueError, "empty failure"):
            parse_census("**Audited repository revision:** `" + self.data["audited_revision"] + "`")


if __name__ == "__main__":
    unittest.main()

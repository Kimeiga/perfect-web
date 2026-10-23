"""Changes in existing Python files must not increase lint debt."""

import unittest

from scripts.quality_delta import added_findings


def issue(code, message):
    return {"code": code, "message": message}


class RuffDelta(unittest.TestCase):
    def test_unmoved_existing_finding_does_not_block(self):
        before = [issue("F401", "unused")]
        after = [issue("F401", "unused")]
        self.assertEqual([], added_findings(before, after))

    def test_new_issue_is_reported(self):
        after = [issue("E701", "colon"), issue("E701", "colon")]
        self.assertEqual([("E701", "colon", 2)], added_findings([], after))

    def test_duplicate_matters_even_with_existing_debt(self):
        self.assertEqual(
            [("F401", "unused", 1)],
            added_findings([issue("F401", "unused")], [issue("F401", "unused")] * 2),
        )

    def test_fix_can_only_reduce_debt(self):
        self.assertEqual([], added_findings([issue("F401", "unused")], []))

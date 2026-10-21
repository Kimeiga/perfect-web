"""Actual producer verdicts retain their canonical mutation-count owner."""
from pathlib import Path
import sys
import unittest

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / 'scripts'))
import evidence_counts
import producer_ownership_mutations

# The nine verdict lines from verify39526960864 at f15117de. Their native
# receipts were valid; eight were misread because bare method identifiers
# were longer than their descriptions. Keep this output independent of the
# current tuple representation and do not reinterpret the historical run.
VERDICTS = """launch cancellation is delivered before ownership: KILLED by test_producer_ownership.ProducerOwnership.test_launch_cancellation_reaps_before_all_four_restore (1 assertion methods fail)
cancellation omits owned cleanup: KILLED by test_producer_ownership.ProducerOwnership.test_running_cancellation_reaps_before_all_four_restore (1 assertion methods fail)
cleanup does not finish native reaping: KILLED by test_producer_ownership.ProducerOwnership.test_launch_cancellation_reaps_before_all_four_restore (1 assertion methods fail)
cleanup cancellation escapes before reaping: KILLED by test_producer_ownership.ProducerOwnership.test_repeated_cancellation_during_cleanup_cannot_skip_restoration (1 assertion methods fail)
default cancellation kills the parent without restoring source: KILLED by test_producer_ownership.ProducerOwnership.test_default_cancellation_raises_through_source_restoration (1 assertion methods fail)
ignored cancellation is captured: KILLED by test_producer_ownership.ProducerOwnership.test_ignored_parent_cancellation_stays_ignored (1 assertion methods fail)
native failed status is changed to success: KILLED by test_producer_ownership.ProducerOwnership.test_native_output_and_nonzero_status_are_unchanged (1 assertion methods fail)
timeout is reported as native completion: KILLED by test_producer_ownership.ProducerOwnership.test_timeout_reaps_and_preserves_native_partial_output (1 assertion methods fail)
original handlers are not restored: KILLED by test_producer_ownership.ProducerOwnership.test_launch_error_restores_original_handlers (1 assertion methods fail)
9 of 9 mutants killed
"""


class Metadata(unittest.TestCase):
    def test_actual_native_verdicts_have_exact_owner_and_catalog(self):
        blocks = evidence_counts.blocks(VERDICTS)
        self.assertEqual(len(blocks), 1)
        _, killed, total, verdicts = blocks[0]
        self.assertEqual((killed, total, len(verdicts)), (9, 9, 9))
        catalog = producer_ownership_mutations.MUTANTS
        self.assertEqual(
            evidence_counts.owner({'producer_ownership_mutations.py': catalog}, verdicts, total),
            'producer_ownership_mutations.py',
        )
        self.assertEqual(evidence_counts.disagree(catalog, killed, total, verdicts), [])


if __name__ == '__main__':
    unittest.main()

"""Actual guide-owner verdicts retain their canonical mutation-count owner."""
from pathlib import Path
import sys
import unittest

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / 'scripts'))
import evidence_counts
import guide_ownership_mutations

# Exact verdict/count lines from verify39526960864 at f15117de. Keep this
# fixture independent of the current catalog representation: bare long test
# identifiers were mistaken for twelve descriptions. These valid historical
# receipts do not make the overall failed bulk run or later source green.
VERDICTS = """only the immediate child is stopped: KILLED by test_guide_process_ownership.GuideOwnership.test_timeout_stops_same_group_grandchild_and_cannot_validate (1 assertion methods fail)
reaped cancellation has no retained native receipt: KILLED by test_guide_process_ownership.GuideOwnership.test_launch_cancellation_reaps_before_restoration_and_stops_its_group (1 assertion methods fail)
timeout status is eligible for validation: KILLED by test_guide_process_ownership.GuideOwnership.test_timeout_stops_same_group_grandchild_and_cannot_validate (1 assertion methods fail)
native stderr is merged into stdout: KILLED by test_guide_process_ownership.GuideOwnership.test_native_files_status_and_clean_import_environment_are_preserved (1 assertion methods fail)
guide cancellation cannot unwind the nested direct owner: KILLED by test_guide_process_ownership.GuideOwnership.test_nested_timeout_restores_both_owners_even_when_leaf_ignores_term (1 assertion methods fail)
equal grace deadlines let the outer owner skip inner restoration: KILLED by test_guide_process_ownership.GuideOwnership.test_recursive_cancellation_has_descending_cleanup_deadlines (1 assertion methods fail)
exited leader spares its surviving group: KILLED by test_guide_process_ownership.GuideOwnership.test_exited_leader_does_not_spare_a_term_ignoring_grandchild (1 assertion methods fail)
grace reaps the leader before the final group signal: KILLED by test_guide_process_ownership.GuideOwnership.test_abort_retains_the_leader_until_the_final_group_signal (1 assertion methods fail)
nested cleanup request ignores inherited cap: KILLED by test_guide_process_ownership.GuideOwnership.test_budget_is_capped_and_reserved_without_mutating_environments (1 assertion methods fail)
exhausted authority can still launch a native child: KILLED by test_guide_process_ownership.GuideOwnership.test_exhausted_or_malformed_budget_refuses_before_native_launch (1 assertion methods fail)
caller environment resets inherited cleanup authority: KILLED by test_guide_process_ownership.GuideOwnership.test_budget_is_capped_and_reserved_without_mutating_environments (1 assertion methods fail)
receipt failure masks pending cancellation: KILLED by test_guide_process_ownership.GuideOwnership.test_callback_storage_error_does_not_mask_cancellation_or_restoration (1 assertion methods fail)
late cancellation signals a reaped native group: KILLED by test_guide_process_ownership.GuideOwnership.test_cancellation_after_native_completion_never_signals_a_reaped_group (1 assertion methods fail)
13 of 13 mutants killed
"""


class Metadata(unittest.TestCase):
    def test_actual_native_verdicts_have_exact_owner_and_catalog(self):
        blocks = evidence_counts.blocks(VERDICTS)
        self.assertEqual(len(blocks), 1)
        _, killed, total, verdicts = blocks[0]
        self.assertEqual((killed, total, len(verdicts)), (13, 13, 13))
        catalog = guide_ownership_mutations.MUTANTS
        self.assertEqual(
            evidence_counts.owner({'guide_ownership_mutations.py': catalog}, verdicts, total),
            'guide_ownership_mutations.py',
        )
        self.assertEqual(evidence_counts.disagree(catalog, killed, total, verdicts), [])


if __name__ == '__main__':
    unittest.main()

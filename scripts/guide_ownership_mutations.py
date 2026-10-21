#!/usr/bin/env python3
"""Native witnesses for canonical Python launch ownership and retained receipts."""
from pathlib import Path
import sys
import tempfile

ROOT = Path(__file__).resolve().parent.parent
OWNER = ROOT / 'scripts/producer_process.py'
GUIDE = ROOT / 'scripts/guide_python.py'
MUTANTS = [
    ('only the immediate child is stopped', 'test_timeout_stops_same_group_grandchild_and_cannot_validate', OWNER,
     '        os.killpg(child.pid, number)\n',
     '        os.kill(child.pid, number)\n'),
    ('reaped cancellation has no retained native receipt', 'test_launch_cancellation_reaps_before_restoration_and_stops_its_group', OWNER,
     '            if child is not None and child.returncode is not None and on_reaped is not None:\n',
     '            if False:\n'),
    ('timeout status is eligible for validation', 'test_timeout_stops_same_group_grandchild_and_cannot_validate', GUIDE,
     '                        validated_status=None if timed_out or cancelled else actual_status,\n',
     '                        validated_status=actual_status,\n'),
    ('native stderr is merged into stdout', 'test_native_files_status_and_clean_import_environment_are_preserved', GUIDE,
     'stdout=stdout, stderr=stderr,\n', 'stdout=stdout, stderr=subprocess.STDOUT,\n'),
    ('guide cancellation cannot unwind the nested direct owner', 'test_nested_timeout_restores_both_owners_even_when_leaf_ignores_term', GUIDE,
     'text=True, timeout=timeout, on_reaped=reaped, grace=2)',
     'text=True, timeout=timeout, on_reaped=reaped, grace=0)'),
    ('equal grace deadlines let the outer owner skip inner restoration', 'test_recursive_cancellation_has_descending_cleanup_deadlines', OWNER,
     'child_environment[CLEANUP_ENV] = str(max(0, effective - PARENT_MARGIN_MS))',
     'child_environment[CLEANUP_ENV] = str(effective)'),
    ('exited leader spares its surviving group', 'test_exited_leader_does_not_spare_a_term_ignoring_grandchild', OWNER,
     '                stop_group(child, signal.SIGKILL)\n',
     '                if child.poll() is None: stop_group(child, signal.SIGKILL)\n'),
    ('grace reaps the leader before the final group signal', 'test_abort_retains_the_leader_until_the_final_group_signal', OWNER,
     'os.WEXITED | os.WNOHANG | os.WNOWAIT', 'os.WEXITED | os.WNOHANG'),
    ('nested cleanup request ignores inherited cap', 'test_budget_is_capped_and_reserved_without_mutating_environments', OWNER,
     '        effective = min(effective, allowance)\n', '        effective = effective\n'),
    ('exhausted authority can still launch a native child', 'test_exhausted_or_malformed_budget_refuses_before_native_launch', OWNER,
     '        if not 0 < allowance <= MAX_CLEANUP_MS:\n', '        if not 0 <= allowance <= MAX_CLEANUP_MS:\n'),
    ('caller environment resets inherited cleanup authority', 'test_budget_is_capped_and_reserved_without_mutating_environments', OWNER,
     'child_environment[CLEANUP_ENV] = str(max(0, effective - PARENT_MARGIN_MS))',
     'child_environment.setdefault(CLEANUP_ENV, str(max(0, effective - PARENT_MARGIN_MS)))'),
    ('receipt failure masks pending cancellation', 'test_callback_storage_error_does_not_mask_cancellation_or_restoration', OWNER,
     '                    if not pending:\n', '                    if True:\n'),

    ('late cancellation signals a reaped native group', 'test_cancellation_after_native_completion_never_signals_a_reaped_group', OWNER,
     '            if child is not None and not complete:\n',
     '            if child is not None and (not complete or pending):\n'),

]
MUTANTS = [(name, 'test_guide_process_ownership.GuideOwnership.' + witness, path, anchor, replacement)
           for name, witness, path, anchor, replacement in MUTANTS]


def main():
    import guide_python
    import mutation_baseline
    watch = mutation_baseline.mutation_bound.start()
    selector = 'test_guide_process_ownership'
    search = ROOT / 'scripts/tests'
    evidence = Path(tempfile.mkdtemp(prefix='pw-guide-ownership-'))
    expected = guide_python.freeze(selector, search, 'test_guide_process_ownership.py', 15, evidence)
    baseline = guide_python.run(selector, expected, search, evidence, timeout=60)
    print(baseline.output)
    if not baseline.result.valid or baseline.result.failed or watch.stopped:
        print('REFUSED: incomplete or failed baseline')
        return 1
    print(f'baseline: {len(expected)} passed, 0 failed')
    killed = 0
    for name, witness, path, anchor, replacement in MUTANTS:
        original = path.read_text()
        if original.count(anchor) != 1:
            print(f'REFUSED: {name}: anchor is not unique')
            return 1
        if witness not in expected:
            print(f'REFUSED: {name}: witness not in frozen inventory')
            return 1
        try:
            path.write_text(original.replace(anchor, replacement, 1))
            receipt = guide_python.run(witness, [witness], search, evidence, timeout=30)
        finally:
            path.write_text(original)
        print(receipt.output)
        result = receipt.result
        if not result.valid or result.body_errors or watch.stopped:
            print(f'REFUSED: {name}: invalid, erroneous or stopped producer; no kill')
            return 1
        if witness in result.assertion_failed:
            killed += 1
            print(f'{name}: KILLED by {witness} ({len(result.assertion_failed)} assertion methods fail)')
        else:
            print(f'{name}: SURVIVED')
    print(f'{killed} of {len(MUTANTS)} mutants killed')
    return int(killed != len(MUTANTS))


if __name__ == '__main__':
    sys.exit(main())

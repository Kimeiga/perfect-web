#!/usr/bin/env python3
"""Controls for graph producer validity and its explicit stack property (ADR-0205)."""
from pathlib import Path
import signal
import subprocess
import sys
import tempfile
import cargo_assertion_results
import guide_python
import mutation_source
import producer_process

ROOT = Path(__file__).resolve().parent.parent
PRODUCER = ROOT / 'scripts/graphs_on_the_wire_mutations.py'
STACK = ROOT / 'runtime/pw-render/tests/graphs/stack_probe.rs'
RUST = ROOT / 'runtime/pw-render/tests/graph_stack_probe.rs'
# All Rust protocol controls exercise this one classifier contract.
RUST_WITNESS = 'only_the_complete_property_or_identified_native_overflow_is_observed'
MUTANTS = [
    ('invalid Cargo results are accepted', 'python', 'Producers.test_bad_native_status_build_panic_or_abort_is_invalid', PRODUCER,
     '    return (True, result.passed, result.failed) if result.valid else (False, 0, 0)\n',
     '    return (True, result.passed, result.failed)\n'),
    ('the complete Cargo inventory is optional', 'python', 'Producers.test_exact_inventory_is_required', PRODUCER,
     'sources=[SOURCES[suite]], expected=NAMES[suite])', 'sources=[SOURCES[suite]], expected=None)'),
    ('an invalid command contributes a kill', 'python', 'Producers.test_partial_command_set_is_invalid', PRODUCER,
     '        if not valid:\n            return False, 0, 0\n', '        if not valid:\n            return True, passed, failed + 1\n'),
    ('a timeout supplies a failed status', 'python', 'Producers.test_timeout_keeps_invalid_native_status', PRODUCER,
     '        return expired.output, None\n', '        return expired.output, 101\n'),
    ('an invalid producer earns a kill', 'python', 'Producers.test_invalid_or_stopped_results_earn_no_kill', PRODUCER,
     '        if not valid or watch.stopped:\n', '        if watch.stopped:\n'),
    ('a stopped producer earns a kill', 'python', 'Producers.test_invalid_or_stopped_results_earn_no_kill', PRODUCER,
     '        if not valid or watch.stopped:\n', '        if not valid:\n'),
    ('an invalid baseline runs controls', 'python', 'Producers.test_baseline_must_be_complete_and_green', PRODUCER,
     '    if not valid or failed or not passed or watch.stopped:\n        print("FAIL:',
     '    if False:\n        print("FAIL:'),
    ('failed restored tests are ignored', 'python', 'Producers.test_restored_source_must_pass', PRODUCER,
     '    if not valid or failed or not passed or watch.stopped:\n        print("REFUSED:',
     '    if False:\n        print("REFUSED:'),
    ('cancelled mutation source is retained', 'python', 'Producers.test_cancellation_restores_source_and_identity', PRODUCER,
     '        with mutation_source.changed(path, original, original.replace(anchor, replacement, 1)):\n',
     '        with mutation_source.changed(path, original.replace(anchor, replacement, 1), original.replace(anchor, replacement, 1)):\n'),
    ('unrelated native abort is a stack observation', 'rust', None, STACK,
     'let overflow = drop_protocol && aborted(status) && native_overflow;',
     'let overflow = drop_protocol && !status.success();'),
    ('the property need not reach the declared drop phase', 'rust', None, STACK,
     'lines.get(2) != Some(&drop_phase.as_str())', 'false'),
    ('the child need not identify its intended workload', 'rust', None, STACK,
     'lines.first() != Some(&ready.as_str())', 'false'),
    ('completion may omit its terminal receipt', 'rust', None, STACK,
     'let complete_protocol = lines.len() == 4 && lines[3] == complete;', 'let complete_protocol = true;'),
    ('a malformed observation shape is semantic evidence', 'rust', None, STACK,
     'values.len() != fields', 'false'),
    ('a malformed boolean is semantic evidence', 'rust', None, STACK,
     'values.iter().skip(1).any(|value| *value > 1)', 'false'),
]
SELECTOR = 'test_graphs_producers'
SEARCH = ROOT / 'scripts/tests'
RUST_NAMES = (
    'only_the_complete_property_or_identified_native_overflow_is_observed',
    'a_native_valid_workload_completes_with_its_observations',
    'a_native_setup_panic_is_invalid',
    'a_native_exit_without_the_workload_receipt_is_invalid',
    'a_native_unrelated_abort_is_invalid',
    'a_native_timeout_is_reaped_and_invalid',
)


def rust():
    try:
        result = producer_process.run(['cargo', 'test', '--quiet', '--locked', '-p', 'pw-render',
            '--test', 'graph_stack_probe', '--', '--format=pretty'], timeout=90, cwd=ROOT)
        parsed = cargo_assertion_results.validate(result.returncode, result.stdout, root=ROOT,
            sources=[RUST], expected=RUST_NAMES)
        return parsed.valid, set(parsed.failure_names), parsed.passed + parsed.failed, result.stdout
    except subprocess.TimeoutExpired as expired:
        return False, set(), 0, expired.output


def main():
    import mutation_baseline  # keep native output before running any producer
    signal.signal(signal.SIGTERM, lambda *_: sys.exit(1))
    watch = mutation_baseline.mutation_bound.start()
    evidence = Path(tempfile.mkdtemp(prefix='pw-graphs-controls-'))
    expected = guide_python.freeze(SELECTOR, SEARCH, SELECTOR + '.py', 10, evidence)
    def python():
        run = guide_python.run(SELECTOR, expected, SEARCH, evidence, timeout=60)
        result = run.result
        return result.valid and not result.body_errors, set(result.assertion_failed), result.executed, run.output
    suites = {'python': python, 'rust': rust}
    for suite in suites.values():
        valid, failures, count, output = suite()
        if not valid or failures or not count or watch.stopped:
            print(output, flush=True)
            print('FAIL: the unmutated baseline is not green; no control can mean anything')
            mutation_baseline.explain()
            return 1
    killed = refused = survived = 0
    for what, suite, witness, path, anchor, replacement in MUTANTS:
        original = path.read_text()
        if original.count(anchor) != 1:
            print(f'{what}: REFUSED (anchor not unique)', flush=True)
            refused += 1
            continue
        with mutation_source.changed(path, original, original.replace(anchor, replacement, 1)):
            if path.suffix == '.py': compile(path.read_text(), str(path), 'exec')
            valid, failures, count, output = suites[suite]()
        intended = SELECTOR + '.' + witness if suite == 'python' else RUST_WITNESS
        if not valid or watch.stopped:
            verdict = 'REFUSED (invalid or stopped producer; no kill)'
            refused += 1
        elif intended in failures:
            verdict = f'KILLED by {intended} ({len(failures)} of {count} tests fail)'
            killed += 1
        else:
            verdict = 'SURVIVED'
            survived += 1
        print(f'{what} [{suite}]: {verdict}', flush=True)
        if verdict.startswith(('REFUSED', 'SURVIVED')): print(output, flush=True)
        if watch.stopped: return 1
    valid, failures, count, output = rust()
    if not valid or failures or not count or watch.stopped:
        print(output, flush=True)
        print('REFUSED: restored property producer is not green')
        return 1
    print(f'{killed} of {len(MUTANTS)} mutants killed; {refused} refused; {survived} survived', flush=True)
    return int(bool(refused or survived))


if __name__ == '__main__':
    sys.exit(main())

#!/usr/bin/env python3
"""Strict controls for the representation campaign's native evidence wiring."""
from pathlib import Path
import sys
import tempfile

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / "scripts"))
import guide_python
import mutation_source

PRODUCER = ROOT / "scripts/template_representations_mutations.py"
MUTANTS = [
    ("invalid Cargo evidence is accepted", "Producers.test_bad_native_status_build_or_panic_is_invalid", PRODUCER,
     'return (True, result.passed, result.failed) if result.valid else (False, 0, 0)',
     'return (True, result.passed, result.failed)'),
    ("the Cargo inventory is not required", "Producers.test_exact_inventory_is_required", PRODUCER,
     'sources=[SOURCES[suite]], expected=NAMES[suite])', 'sources=[SOURCES[suite]], expected=None)'),
    ("an invalid command is omitted from the campaign", "Producers.test_partial_command_set_is_invalid", PRODUCER,
     '        if not valid:\n', '        if False:\n'),
    ("a timeout supplies a failure status", "Producers.test_timeout_keeps_invalid_native_status", PRODUCER,
     '        return expired.output, None\n', '        return expired.output, 101\n'),
    ("a restoration command may fail", "Producers.test_restoration_build_errors_are_refused", PRODUCER,
     '        if code != 0:\n', '        if False:\n'),
    ("an invalid producer earns a kill", "Producers.test_invalid_or_stopped_results_earn_no_kill", PRODUCER,
     '        if not valid or watch.stopped:\n', '        if watch.stopped:\n'),
    ("a stopped producer earns a kill", "Producers.test_invalid_or_stopped_results_earn_no_kill", PRODUCER,
     '        if not valid or watch.stopped:\n', '        if not valid:\n'),
    ("an invalid baseline runs controls", "Producers.test_baseline_must_be_complete_and_green", PRODUCER,
     '        if not valid or failed or not passed or watch.stopped:\n', '        if False:\n'),
    ("failed restoration is ignored", "Producers.test_failed_restored_build_fails_campaign", PRODUCER,
     '    if not build() or watch.stopped:\n', '    if False:\n'),
    ("cancelled mutation source is retained", "Producers.test_cancellation_restores_source_and_identity", PRODUCER,
     '        with mutation_source.changed(path, original, original.replace(anchor, replacement, 1)):\n',
     '        with mutation_source.changed(path, original.replace(anchor, replacement, 1), original.replace(anchor, replacement, 1)):\n'),
]
SELECTOR = "test_representations_producers"
SEARCH = ROOT / "scripts/tests"


def main():
    import mutation_baseline  # keep native output before running any producer
    watch = mutation_baseline.mutation_bound.start()
    evidence = Path(tempfile.mkdtemp(prefix="pw-representations-controls-"))
    expected = guide_python.freeze(SELECTOR, SEARCH, SELECTOR + ".py", 11, evidence)
    baseline = guide_python.run(SELECTOR, expected, SEARCH, evidence, timeout=60)
    if not baseline.result.valid or baseline.result.failed or watch.stopped:
        print(baseline.output, flush=True)
        print("FAIL: the unmutated baseline is not green; no control can mean anything")
        mutation_baseline.explain()
        return 1
    killed = refused = survived = 0
    for what, witness, path, anchor, replacement in MUTANTS:
        original = path.read_text()
        if original.count(anchor) != 1:
            print(f"{what}: REFUSED (anchor not unique)", flush=True)
            refused += 1
            continue
        with mutation_source.changed(path, original, original.replace(anchor, replacement, 1)):
            compile(path.read_text(), str(path), "exec")
            run = guide_python.run(SELECTOR, expected, SEARCH, evidence, timeout=60)
        result = run.result
        intended = SELECTOR + "." + witness
        if not result.valid or result.body_errors or watch.stopped:
            verdict = "REFUSED (invalid, stopped or erroneous producer; no kill)"
            refused += 1
        elif intended in result.assertion_failed:
            verdict = f"KILLED by {intended} ({len(result.failed)} of {result.executed} tests fail)"
            killed += 1
        else:
            verdict = "SURVIVED"
            survived += 1
        print(f"{what}: {verdict}", flush=True)
        if verdict.startswith(("REFUSED", "SURVIVED")):
            print(run.output, flush=True)
        if watch.stopped:
            return 1
    print(f"{killed} of {len(MUTANTS)} mutants killed; {refused} refused; {survived} survived", flush=True)
    return int(bool(refused or survived))


if __name__ == '__main__':
    sys.exit(main())
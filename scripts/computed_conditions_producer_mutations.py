#!/usr/bin/env python3
"""Assertion controls for the computed-condition campaign's existing native authorities."""
from pathlib import Path
import sys
import tempfile

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / "scripts"))
import guide_python
import mutation_source
import producer_process

PRODUCER = ROOT / "scripts/computed_conditions_mutations.py"
MUTANTS = [
    ("invalid cargo results credited", "Producers.test_cargo_refuses_status_build_and_timeout", PRODUCER,
     'return (True, result.passed, result.failed) if result.valid else (False, 0, 0)',
     'return (True, result.passed, result.failed)'),
    ("cargo inventory is not frozen", "Producers.test_cargo_requires_every_named_result", PRODUCER,
     'sources=[SOURCES[suite]], expected=NAMES[suite])', 'sources=[SOURCES[suite]], expected=None)'),
    ("build failure is accepted", "Producers.test_build_error_or_timeout_is_invalid", PRODUCER,
     '        if code != 0:\n', '        if False:\n'),
    ("browser inventory is not frozen", "Producers.test_browser_requires_complete_frozen_inventory", PRODUCER,
     'nonce, INVENTORIES.get("browser"))', 'nonce, None)'),
    ("invalid browser receipt is credited", "Producers.test_browser_uses_fresh_native_receipt_not_prose", PRODUCER,
     '        if not result.valid:\n', '        if False:\n'),
    ("invalid producer earns a kill", "Producers.test_invalid_or_stopped_producers_earn_no_kill", PRODUCER,
     '        if not valid or watch.stopped:\n', '        if watch.stopped:\n'),
    ("restored build failure is ignored", "Producers.test_restored_build_must_succeed", PRODUCER,
     '    if not build() or watch.stopped:\n', '    if False:\n'),
    ("cancellation restores mutant source", "Producers.test_cancelled_campaign_restores_source", PRODUCER,
     '        with mutation_source.changed(path, original, original.replace(anchor, replacement, 1)):\n',
     '        with mutation_source.changed(path, original.replace(anchor, replacement, 1), original.replace(anchor, replacement, 1)):\n'),
    ("timeout is a failed test", "Producers.test_timeout_retains_invalid_status_and_shared_owner", PRODUCER,
     '        return expired.output, None\n', '        return expired.output, 101\n'),

]
SELECTOR = "test_computed_conditions_producers"
SEARCH = ROOT / "scripts/tests"
EXPECTED = 12


def main():
    import mutation_baseline
    watch = mutation_baseline.mutation_bound.start()
    evidence = Path(tempfile.mkdtemp(prefix="pw-computed-conditions-controls-"))
    expected = guide_python.freeze(SELECTOR, SEARCH, SELECTOR + ".py", EXPECTED, evidence)
    baseline = guide_python.run(SELECTOR, expected, SEARCH, evidence, timeout=60)
    if not baseline.result.valid or baseline.result.failed or watch.stopped:
        print(baseline.output, flush=True)
        return 1
    killed = refused = survived = 0
    for what, witness, path, anchor, replacement in MUTANTS:
        original = path.read_text()
        if original.count(anchor) != 1:
            print(f"REFUSED {what}: anchor not unique", flush=True)
            refused += 1
            continue
        with mutation_source.changed(path, original, original.replace(anchor, replacement, 1)):
            if path.suffix == ".py":
                compile(path.read_text(), str(path), "exec")
            elif path.suffix == ".mjs":
                syntax = producer_process.run(["node", "--check", str(path)], timeout=10, cwd=ROOT)
                if syntax.returncode != 0:
                    print(f"REFUSED {what}: JavaScript syntax", flush=True)
                    refused += 1
                    continue
            run = guide_python.run(SELECTOR, expected, SEARCH, evidence, timeout=60)
        result = run.result
        intended = SELECTOR + "." + witness
        if not result.valid or result.body_errors or watch.stopped:
            verdict = "REFUSED (invalid, stopped or erroneous producer; no kill)"
            refused += 1
        elif intended in result.assertion_failed:
            verdict = f"KILLED by {intended} ({len(result.assertion_failed)} of {len(expected)} tests fail)"
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

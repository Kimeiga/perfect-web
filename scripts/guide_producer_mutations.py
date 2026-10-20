#!/usr/bin/env python3
"""Whole guide producer controls, held by exact canonical body assertions.

The guide's 74 program controls stay separate and unchanged. This instrument
runs every listed control. Invalid producers, stops, body errors and missing
intended assertions earn no kill. All edited source bytes restore in finally.
"""
from pathlib import Path
import sys
import tempfile

ROOT = Path(__file__).resolve().parent.parent
if str(ROOT / "scripts") not in sys.path:
    sys.path.insert(0, str(ROOT / "scripts"))
import guide_python

GUIDE = ROOT / "scripts/guide_mutations.py"
MUTANTS = [
    ("Cargo result validity is ignored", "GuideProducers.test_cargo_refuses_invalid_failure_status", GUIDE,
     "    if not cargo.valid:\n", "    if False and not cargo.valid:\n"),
    ("Python canonical result validity is ignored", "GuideNativeProtocol.test_native_setup_failures_earn_no_kill_and_never_enter_body", GUIDE,
     "    if not outcome.valid:\n", "    if False and not outcome.valid:\n"),
    ("browser command bypasses canonical suite authority", "GuideProducers.test_browser_uses_canonical_authority_and_exact_selection", GUIDE,
     '["bash", str(ROOT / "scripts/run_suite.sh"), "-C", ".", "--", *SPEC]',
     '["bash", "-c", "exit 0", "--", *SPEC]'),
    ("browser status and counts need not agree", "GuideProducers.test_browser_refuses_invalid_failure_status", GUIDE,
     "    valid = _test_status(code, passed, failed, 1)\n", "    valid = True\n"),
    ("partial codes after an invalid compiler exit earn a checker kill", "GuideProducers.test_main_refuses_named_codes_after_invalid_build_status", GUIDE,
     "        if code == 1 and codes:\n", "        if codes:\n"),
    ("invalid guide producer is credited as killed", "GuideProducers.test_main_refuses_cargo_invalid_status_without_kill", GUIDE,
     '                    verdict = "REFUSED (invalid or stopped guide producer; no kill)"\n                    refused += 1\n',
     '                    verdict = "KILLED (invalid producer)"\n                    killed += 1\n'),
    ("bound stop with partial valid output is ignored", "GuideProducers.test_main_refuses_bound_stop_with_valid_partial_summary", GUIDE,
     "    return watch is not None and bool(watch.stopped)\n", "    return False\n"),
    ("final restored build failure is ignored", "GuideProducers.test_final_restored_build_failure_fails_campaign", GUIDE,
     "        if code != 0 or _stopped(watch):\n", "        if False:\n"),
]
SELECTOR = "test_guide_producer_results"
SEARCH = ROOT / "scripts/tests"
PATTERN = "test_guide_producer_results.py"
EXPECTED_COUNT = 40
TIMEOUT = 120


def main():
    import mutation_baseline
    watch = mutation_baseline.mutation_bound.start()
    evidence = Path(tempfile.mkdtemp(prefix="pw-guide-producer-controls-"))
    print(f"structured instrument receipts: {evidence}", flush=True)
    try:
        expected = guide_python.freeze(SELECTOR, SEARCH, PATTERN, EXPECTED_COUNT, evidence)
    except guide_python.DiscoveryRefused as error:
        print(f"REFUSED: {error}")
        return 1
    witnesses = {"test_guide_producer_results." + mutant[1] for mutant in MUTANTS}
    if not witnesses <= set(expected):
        print("REFUSED: intended full IDs are not in the frozen inventory")
        return 1
    baseline = guide_python.run(SELECTOR, expected, SEARCH, evidence, TIMEOUT)
    result = baseline.result
    print(f"baseline: {result.executed} executed; {len(result.failed)} failed; valid={result.valid}", flush=True)
    if not result.valid or result.failed or result.executed != EXPECTED_COUNT or watch.stopped:
        print(baseline.output)
        mutation_baseline.explain()
        return 1
    killed = refused = survived = 0
    originals = {path: path.read_bytes() for *_, path, anchor, replacement in MUTANTS}
    try:
        for what, witness, path, anchor, replacement in MUTANTS:
            original = originals[path]
            assert path.read_bytes() == original, "source changed between owned controls"
            text = original.decode()
            if text.count(anchor) != 1:
                print(f"REFUSED {what}: anchor did not match exactly once", flush=True)
                refused += 1
                continue
            try:
                path.write_bytes(text.replace(anchor, replacement, 1).encode())
                run = guide_python.run(SELECTOR, expected, SEARCH, evidence, TIMEOUT)
                result = run.result
                intended = "test_guide_producer_results." + witness
                if not result.valid or watch.stopped or result.body_errors:
                    verdict = "REFUSED (invalid, stopped or erroneous instrument producer; no kill)"
                    refused += 1
                    print(run.output)
                elif intended in result.assertion_failed:
                    verdict = f"KILLED by {intended} ({len(result.assertion_failed)} actual body assertions)"
                    killed += 1
                else:
                    verdict = "SURVIVED (no intended body assertion)"
                    survived += 1
                    print(run.output)
            finally:
                path.write_bytes(original)
            print(f"{what}: {verdict}", flush=True)
    finally:
        for path, original in originals.items():
            path.write_bytes(original)
        assert all(path.read_bytes() == original for path, original in originals.items())
    print(f"{killed} of {len(MUTANTS)} mutants killed; {refused} refused; {survived} survived", flush=True)
    return int(bool(refused or survived or watch.stopped))


if __name__ == "__main__":
    sys.exit(main())
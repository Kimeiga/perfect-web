#!/usr/bin/env python3
"""Every device producer control needs its named real unittest body assertion."""
from pathlib import Path
import sys
import tempfile

ROOT = Path(__file__).resolve().parent.parent
sys.path.insert(0, str(ROOT / "scripts"))
import guide_python

DEVICE = ROOT / "scripts/device_store_mutations.py"
BROWSER = ROOT / "scripts/browser_test_results.py"
REPORTER = ROOT / "spikes/own-renderer/assertion-result-reporter.mjs"
MUTANTS = [
    ('invalid cargo native result accepted', 'DeviceProducers.test_cargo_refuses_status_and_build_errors', DEVICE,
     '    result = cargo_test_results.parse(code, out)\n    if not result.valid:\n        return False, 0, 0\n', '    result = cargo_test_results.parse(code, out)\n    if not result.valid:\n        return True, 0, 1\n'),
    ('cargo inventory may change', 'DeviceProducers.test_cargo_inventory_must_be_complete_and_frozen', DEVICE,
     'or (suite in INVENTORIES and names != INVENTORIES[suite])', 'or (False and suite in INVENTORIES and names != INVENTORIES[suite])'),
    ('nonassertion panic accepted', 'DeviceProducers.test_cargo_refuses_nonassertion_panics', DEVICE,
     '            if not 1 <= int(line) <= len(lines) or not re.match(r"\\s*assert(?:_eq|_ne)?!", lines[int(line) - 1]):\n', '            if False:\n'),
    ('build failure accepted', 'DeviceProducers.test_build_failure_and_timeout_are_invalid', DEVICE,
     '        if code != 0:\n', '        if False:\n'),
    ('invalid producer credited as killed', 'DeviceProducers.test_invalid_producer_earns_no_kill', DEVICE,
     '        if not ok or watch.stopped:\n', '        if watch.stopped:\n'),
    ('restored build failure ignored', 'DeviceProducers.test_restored_build_failure_fails_campaign', DEVICE,
     '    if not built() or watch.stopped:\n', '    if False:\n'),
    ('stale browser receipt accepted', 'DeviceProducers.test_browser_requires_complete_fresh_receipt', BROWSER,
     '    if receipt.get("nonce") != nonce:\n', '    if False:\n'),
    ('browser inventory may change', 'DeviceProducers.test_browser_inventory_is_complete_and_frozen', BROWSER,
     '    if expected is not None and set(inventory) != set(expected):\n', '    if False:\n'),
    ('browser exit disagreement accepted', 'DeviceProducers.test_browser_native_exit_must_agree', BROWSER,
     '    if status != (1 if failed else 0):\n', '    if False:\n'),
    ('browser transport exit accepted', 'DeviceProducers.test_browser_native_exit_must_agree', BROWSER,
     '    if type(status) is not int or status not in (0, 1):\n', '    if False:\n'),
    ('hook assertions credited', 'DeviceProducers.test_reporter_refuses_hook_and_runtime_errors', REPORTER,
     '        if (hook) other = true;\n', '        if (false) other = true;\n'),
    ('runtime error credited', 'DeviceProducers.test_reporter_refuses_hook_and_runtime_errors', REPORTER,
     'errors.every(e => assertions.includes(e))', 'true'),
    ('global error credited', 'DeviceProducers.test_reporter_refuses_hook_and_runtime_errors', REPORTER,
     '  onError() { this.problem = true; }', '  onError() {}'),
    ('retry accepted', 'DeviceProducers.test_reporter_refuses_timeout_skip_retry_and_expected_failure', REPORTER,
     ' || result.retry !== 0) this.problem = true;', ' ) this.problem = true;'),
    ('unsupported browser status published', 'DeviceProducers.test_reporter_refuses_timeout_skip_retry_and_expected_failure', REPORTER,
     ' || !["passed", "failed"].includes(result.status)', ' || false'),
    ('duplicate test result accepted', 'DeviceProducers.test_reporter_refuses_incomplete_and_duplicate_callbacks', REPORTER,
     ' || this.results.has(test.id)', ' || false'),
    ('incomplete inventory published', 'DeviceProducers.test_reporter_refuses_incomplete_and_duplicate_callbacks', REPORTER,
     ' || this.results.size !== this.expected.size || this.started.size !== this.expected.size', ' || false'),
    ('skipped cargo tests accepted', 'DeviceProducers.test_cargo_inventory_must_be_complete_and_frozen', DEVICE,
     '    if (re.search(r"; [1-9]\\d* (?:ignored|measured)", out)\n',
     '    if (False\n'),
    ('cancellation leaves owned child running', 'DeviceProducers.test_cancellation_reaps_child_before_source_restoration', DEVICE,
     "    except BaseException:\n        # Cancellation must reap our process group before the caller restores\n        # a mutant's source. Otherwise the old child can still read that source.\n        try:\n            os.killpg(p.pid, signal.SIGKILL)\n        except ProcessLookupError:\n            pass\n        p.communicate()\n        raise\n", '    except BaseException:\n        raise\n'),
 ]
SELECTOR = "test_device_producer_results"
SEARCH = ROOT / "scripts/tests"
PATTERN = "test_device_producer_results.py"
EXPECTED_COUNT = 16
TIMEOUT = 120


def main():
    import mutation_baseline
    watch = mutation_baseline.mutation_bound.start()
    evidence = Path(tempfile.mkdtemp(prefix="pw-device-producer-controls-"))
    print(f"structured instrument receipts: {evidence}", flush=True)
    try:
        expected = guide_python.freeze(SELECTOR, SEARCH, PATTERN, EXPECTED_COUNT, evidence)
    except guide_python.DiscoveryRefused as error:
        print(f"REFUSED: {error}")
        return 1
    witnesses = {"test_device_producer_results." + mutant[1] for mutant in MUTANTS}
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
                intended = "test_device_producer_results." + witness
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

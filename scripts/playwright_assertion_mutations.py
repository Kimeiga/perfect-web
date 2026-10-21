#!/usr/bin/env python3
"""Native browser proof controls; only the intended completed body assertion kills."""
import json
import os
from pathlib import Path
import sys
import tempfile

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / 'scripts'))
import mutation_source
import producer_process
import unittest_producer
import unittest_protocol

EXPECT = ROOT / 'spikes/own-renderer/assertion-expect.mjs'
REPORTER = ROOT / 'spikes/own-renderer/assertion-result-reporter.mjs'
MUTANTS = [
    ('expect steps earn kills without native proof', 'test_missing_native_proof_is_refused', REPORTER,
     '|| !result.errors.every(error => error.cause?.message === `Error: PW_ASSERTION:${this.nonce}`)',
     '|| false'),
    ('old native proof earns a kill', 'test_stale_native_proof_is_refused', REPORTER,
     'error.cause?.message === `Error: PW_ASSERTION:${this.nonce}`',
     'error.cause?.message?.startsWith("Error: PW_ASSERTION:")'),
    ('browser/API errors are branded as matcher failures', 'test_closed_browser_is_not_a_comparison', EXPECT,
     '  if (result && result.name === name && typeof result.pass === "boolean"\n'
     '      && result.pass === isNot && /^[a-f0-9]{64}$/.test(process.env.PW_ASSERTION_NONCE ?? "")) {',
     '  if (/^[a-f0-9]{64}$/.test(process.env.PW_ASSERTION_NONCE ?? "")) {'),
    ('negated comparisons lose native proof', 'test_native_comparisons_messages_and_negation_produce_proof', EXPECT,
     'result.pass === isNot', 'result.pass === false'),
    ('native error causes are discarded', 'test_native_cause_is_preserved_under_proof', EXPECT,
     '{ cause: error.cause }', '{ cause: undefined }'),
    ('poll equality uses ordinary numeric equality', 'test_poll_preserves_object_is_and_completed_mismatches', EXPECT,
     'return { name: "toBePolled", pass: Object.is(actual, expected), actual, expected,',
     'return { name: "toBePolled", pass: actual === expected, actual, expected,'),
    ('callback exceptions become observed mismatches', 'test_poll_callback_errors_are_not_assertion_results', EXPECT,
     '            throw new Error("poll callback failed", { cause });',
     '            return undefined;'),
    ('an unfinished callback becomes an observed mismatch', 'test_pending_poll_callback_is_not_a_mismatch', EXPECT,
     '''          new Promise((_, reject) => {
            timer = setTimeout(() => reject(new Error("poll callback did not complete before its deadline")), remaining);
          }),
        ]);
      } finally { clearTimeout(timer); }
      if (performance.now() >= deadline) throw new Error("poll callback completed after its deadline");''',
     '''          new Promise(resolve => {
            timer = setTimeout(() => resolve(undefined), remaining);
          }),
        ]);
      } finally { clearTimeout(timer); }'''),
]
SELECTOR = 'scripts.tests.test_browser_assertion_proof'
COUNT = 10


def run(selectors, expected, evidence):
    directory = Path(tempfile.mkdtemp(prefix='native-', dir=evidence))
    frozen, report = directory / 'expected.json', directory / 'result.json'
    frozen.write_text(json.dumps(expected))
    env = {**os.environ, 'PW_BROWSER_PROOF': '1',
           'PYTHONPYCACHEPREFIX': str(directory / 'fresh-bytecode'), 'PYTHONDONTWRITEBYTECODE': '1'}
    child = producer_process.run(
        [sys.executable, '-B', 'scripts/unittest_producer.py', '--expected', str(frozen),
         '--report', str(report), *selectors], timeout=120, grace=2, cwd=ROOT, env=env)
    (directory / 'output.txt').write_text(child.stdout)
    (directory / 'process.json').write_text(json.dumps({'native_status': child.returncode}))
    try:
        result = unittest_protocol.validate_result(child.returncode, report.read_text(), expected)
    except OSError:
        return None, child.stdout
    return result, child.stdout


def main():
    import mutation_baseline
    watch = mutation_baseline.mutation_bound.start()
    evidence = Path(tempfile.mkdtemp(prefix='pw-browser-proof-controls-'))
    # The ordinary test command deliberately skips browser cases. This explicit
    # browser campaign freezes and executes all ten, never accepts skip credit.
    os.environ['PW_BROWSER_PROOF'] = '1'
    expected = unittest_producer.load([SELECTOR])[2]
    if len(expected) != COUNT:
        print('REFUSED: unexpected native test inventory')
        return 1
    result, output = run([SELECTOR], expected, evidence)
    print(f'structured instrument receipts: {evidence}', flush=True)
    print(output, flush=True)
    if result is None or not result.valid or result.failed or len(result.passed) != COUNT or watch.stopped:
        print('REFUSED: incomplete or failed baseline')
        return 1
    print(f'baseline: {COUNT} passed, 0 failed', flush=True)
    killed = 0
    for name, method, path, anchor, replacement in MUTANTS:
        witness = SELECTOR + '.BrowserAssertions.' + method
        original = path.read_text()
        if original.count(anchor) != 1 or witness not in expected:
            print(f'REFUSED: {name}: changed authority or witness is not exact')
            return 1
        with mutation_source.changed(path, original, original.replace(anchor, replacement, 1)):
            syntax = producer_process.run(['node', '--check', str(path)], timeout=10, cwd=ROOT)
            if syntax.returncode != 0:
                print(syntax.stdout, flush=True)
                print(f'REFUSED: {name}: invalid JavaScript; no kill')
                return 1
            result, output = run([witness], [witness], evidence)
        print(output, flush=True)
        if result is None or not result.valid or result.body_errors or watch.stopped:
            print(f'REFUSED: {name}: invalid or stopped native producer; no kill')
            return 1
        if witness in result.assertion_failed:
            killed += 1
            print(f'{name}: KILLED by {witness} ({len(result.assertion_failed)} assertion methods fail)', flush=True)
        else:
            print(f'{name}: SURVIVED', flush=True)
    print(f'{killed} of {len(MUTANTS)} mutants killed', flush=True)
    return int(killed != len(MUTANTS))


if __name__ == '__main__':
    sys.exit(main())

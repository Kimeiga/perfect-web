"""The speculation campaign accepts only complete native assertion evidence."""
import contextlib
import io
import json
from pathlib import Path
import signal
import subprocess
import sys
import tempfile
import types
import unittest
from unittest import mock

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / "scripts"))
import speculated_arms_mutations as arms


class Producers(unittest.TestCase):
    def setUp(self):
        arms.INVENTORIES.clear()

    def cargo(self, failed=False, site=None):
        names = sorted(arms.NAMES['core'])
        failing = names[0]
        tests = ''.join(f'test {name} ... {"FAILED" if failed and name == failing else "ok"}\n' for name in names)
        if not failed:
            return tests + f'test result: ok. {len(names)} passed; 0 failed; 0 ignored; 0 measured; 0 filtered out\n'
        if site is None:
            lines = arms.SOURCES['core'].read_text().splitlines()
            line = next(i for i, s in enumerate(lines, 1) if s.lstrip().startswith('assert!(compiled.is_ok()'))
            column = len(lines[line-1]) - len(lines[line-1].lstrip()) + 1
            site = f'compiler/pw-core/tests/speculated_arms.rs:{line}:{column}'
        return (tests + f"\nfailures:\n\n---- {failing} stdout ----\nthread '{failing}' panicked at {site}:\n"
                f'assertion failed\n\nfailures:\n    {failing}\n'
                f'test result: FAILED. {len(names)-1} passed; 1 failed; 0 ignored; 0 measured; 0 filtered out\n')

    def test_recipe_names_each_evidence_owner_once(self):
        import ci_plan
        import evidence_counts
        body = ci_plan.bodies(ROOT)['e14-speculated-arms']
        self.assertEqual(evidence_counts.executed_scripts(body), [
            'speculated_arms_mutations.py', 'speculated_arms_producer_mutations.py',
        ])
        for name in ('speculated_arms_producer_mutations.py', 'speculated_arms_mutations.py'):
            self.assertEqual(body.count('python3 scripts/' + name + ';'), 1)

    def test_cargo_accepts_actual_assertion_and_complete_green(self):
        self.assertEqual(arms.cargo_result('core', 0, self.cargo()), (True, 5, 0))
        self.assertEqual(arms.cargo_result('core', 101, self.cargo(True)), (True, 4, 1))

    def test_cargo_refuses_status_build_and_timeout(self):
        for code in (None, -9, 1, 0, True):
            self.assertFalse(arms.cargo_result('core', code, self.cargo(True))[0])
        self.assertFalse(arms.cargo_result('core', 101, self.cargo(True) + 'error[E123]: broken build')[0])

    def test_cargo_requires_every_named_result(self):
        native = self.cargo()
        for output in (native.replace('test a_page_that_shows_no_target_has_no_speculation ... ok\n', ''),
                       native.replace('a_page_that_shows_no_target_has_no_speculation', 'foreign'),
                       native.replace('0 ignored', '1 ignored')):
            self.assertFalse(arms.cargo_result('core', 0, output)[0])

    def test_cargo_refuses_runtime_and_assertion_operand_panics(self):
        for site in ('compiler/pw-core/src/backend/speculation.rs:1:1',
                     'compiler/pw-core/tests/speculated_arms.rs:1:1', '/tmp/foreign.rs:1:1'):
            self.assertFalse(arms.cargo_result('core', 101, self.cargo(True, site))[0])
        genuine = self.cargo(True)
        import re
        operand = re.sub(r':(\d+):\nassertion failed', ':999:\ncalled Option::unwrap', genuine)
        self.assertFalse(arms.cargo_result('core', 101, operand)[0])

    def test_build_error_or_timeout_is_invalid(self):
        for code in (1, None, -9):
            with mock.patch.object(arms, 'bounded', return_value=('bad build', code)):
                self.assertFalse(arms.build())

    def browser(self, status=0, patch=None, expected=None, present=True):
        def native(command, **kw):
            env = kw['env']
            report = dict(schema=1, complete=True, nonce=env['PW_ASSERTION_NONCE'],
                          inventory=['one'], tests=[['one', 'passed']])
            if patch: report.update(patch)
            if present: Path(env['PW_ASSERTION_REPORT']).write_text(json.dumps(report))
            self.assertIn('--retries=0', command)
            self.assertIn('--reporter=line,./assertion-result-reporter.mjs', command)
            return '999 failed', status
        if expected is not None: arms.INVENTORIES['browser'] = expected
        with mock.patch.object(arms, 'build', return_value=True), mock.patch.object(arms, 'bounded', side_effect=native):
            return arms.browser_tests()

    def test_browser_uses_fresh_native_receipt_not_prose(self):
        self.assertEqual(self.browser(), (True, 1, 0))
        self.assertEqual(self.browser(1, {'tests': [['one', 'failed']]}), (True, 0, 1))
        self.assertFalse(self.browser(1, present=False)[0])
        self.assertFalse(self.browser(1, {'nonce': 'stale'})[0])
        self.assertFalse(self.browser(0, {'tests': [['one', 'failed']]})[0])

    def test_browser_requires_complete_frozen_inventory(self):
        self.assertFalse(self.browser(0, expected=('other',))[0])
        for patch in ({'tests': []}, {'complete': False}, {'tests': [['one', 'skipped']]}):
            arms.INVENTORIES.clear()
            self.assertFalse(self.browser(0, patch)[0])

    def campaign(self, result=(True, 0, 1), stopped=False, restored=True, cancel=False):
        import mutation_baseline
        with tempfile.TemporaryDirectory() as tmp:
            source = Path(tmp) / 'owned'; source.write_text('original')
            watch = types.SimpleNamespace(stopped=False)
            calls = 0
            def suite():
                nonlocal calls
                calls += 1
                if calls == 1: return True, 1, 0
                watch.stopped = stopped
                if cancel: raise SystemExit(37)
                return result
            with mock.patch.object(arms, 'MUTANTS', [('fault', 'cargo', source, 'original', 'mutant')]), \
                 mock.patch.object(arms, 'SUITES', {'cargo': suite}), \
                 mock.patch.object(arms, 'build', return_value=restored), \
                 mock.patch.object(arms.signal, 'signal'), \
                 mock.patch.object(mutation_baseline.mutation_bound, 'start', return_value=watch), \
                 contextlib.redirect_stdout(io.StringIO()):
                try:
                    if cancel:
                        with self.assertRaises(SystemExit) as error: arms.main()
                        self.assertEqual(error.exception.code, 37)
                        code = 1
                    else: code = arms.main()
                finally:
                    self.assertEqual(source.read_text(), 'original')
            return code

    def test_invalid_or_stopped_producers_earn_no_kill(self):
        self.assertEqual(self.campaign(), 0)
        self.assertEqual(self.campaign((False, 0, 1)), 1)
        self.assertEqual(self.campaign(stopped=True), 1)
        self.assertEqual(self.campaign((True, 1, 0)), 1)

    def test_restored_build_must_succeed(self):
        self.assertEqual(self.campaign(restored=False), 1)

    def test_cancelled_campaign_restores_source(self):
        self.assertEqual(self.campaign(cancel=True), 1)

    def test_timeout_retains_invalid_status_and_shared_owner(self):
        with mock.patch.object(arms.producer_process, 'run', side_effect=subprocess.TimeoutExpired(['native'], 1, output='partial')) as run:
            self.assertEqual(arms.bounded(['native'], cwd=ROOT), ('partial', None))
            self.assertEqual(run.call_args.kwargs['timeout'], arms.BOUND)

    def test_feed_runs_all_cases_with_supported_assertions(self):
        source = (ROOT / 'spikes/own-renderer/e2e/feed.spec.mjs').read_text()
        self.assertIn('test.describe.configure({ mode: "default" });', source)
        self.assertIn('import { expect } from "../assertion-expect.mjs";', source)
        self.assertIn('expect.poll(async () => (await rows.count()) > 20).toBe(true)', source)
        self.assertNotIn('expect.poll(() => rows.count()).toBeGreaterThan(20)', source)


if __name__ == '__main__':
    unittest.main()
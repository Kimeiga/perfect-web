"""Device evidence refuses incomplete producers; genuine assertions still kill."""
import contextlib
import io
import json
import os
from pathlib import Path
import subprocess
import signal
import time
import sys
import tempfile
import types
import unittest
from unittest import mock

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / 'scripts'))
import browser_test_results as browser
import device_store_mutations as device


class DeviceProducers(unittest.TestCase):
    def setUp(self):
        device.INVENTORIES.clear()

    def receipt(self):
        return dict(schema=1, complete=True, nonce='fresh', inventory=['one'], tests=[['one', 'passed']])

    def test_browser_requires_complete_fresh_receipt(self):
        for report in (None, {}, {**self.receipt(), 'complete': False}, {**self.receipt(), 'nonce': 'stale'}, {**self.receipt(), 'schema': True}):
            self.assertFalse(browser.parse(0, report, 'fresh').valid)
        self.assertFalse(browser.parse(True, report, 'fresh').valid)
        self.assertTrue(browser.parse(0, self.receipt(), 'fresh').valid)

    def test_browser_native_exit_must_agree(self):
        for code in (None, -9, 2, 1, True):
            self.assertFalse(browser.parse(code, self.receipt(), 'fresh').valid)
        report = self.receipt(); report['tests'][0][1] = 'failed'
        self.assertTrue(browser.parse(1, report, 'fresh').valid)
        self.assertFalse(browser.parse(0, report, 'fresh').valid)
        self.assertFalse(browser.parse(True, report, 'fresh').valid)

    def test_browser_inventory_is_complete_and_frozen(self):
        for patch in ({'tests': []}, {'tests': [['one', 'skipped']]}, {'inventory': []},
                      {'inventory': ['one', 'two']}, {'tests': [['one', 'passed'], ['one', 'passed']]}):
            self.assertFalse(browser.parse(0, {**self.receipt(), **patch}, 'fresh').valid)
        self.assertFalse(browser.parse(0, self.receipt(), 'fresh', ('other',)).valid)

    def reporter(self, mode):
        with tempfile.TemporaryDirectory() as tmp:
            report = Path(tmp) / 'result.json'
            run = subprocess.run(['node', str(ROOT / 'spikes/own-renderer/assertion-result-fixture.mjs'), mode],
                env={**os.environ, 'PW_ASSERTION_REPORT': str(report), 'PW_ASSERTION_NONCE': 'a' * 64},
                capture_output=True, text=True, timeout=10)
            self.assertEqual(run.returncode, 0, run.stdout + run.stderr)
            return json.loads(report.read_text()) if report.exists() else None

    def test_reporter_accepts_body_assertions_and_green(self):
        for mode, status in (('green', 0), ('assertion', 1), ('nested', 1)):
            self.assertTrue(browser.parse(status, self.reporter(mode), 'a' * 64).valid, mode)

    def test_reporter_refuses_hook_and_runtime_errors(self):
        for mode in ('hook', 'runtime', 'global', 'extraError'):
            self.assertIsNone(self.reporter(mode), mode)

    def test_reporter_refuses_timeout_skip_retry_and_expected_failure(self):
        for mode in ('timeout', 'skipped', 'retry', 'unexpected'):
            self.assertIsNone(self.reporter(mode), mode)

    def test_reporter_refuses_incomplete_and_duplicate_callbacks(self):
        for mode in ('missing', 'duplicate', 'doubleEnd', 'noEnd'):
            self.assertIsNone(self.reporter(mode), mode)

    def cargo(self, failed=False, site=None):
        if not failed:
            return 'test one ... ok\ntest result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out\n'
        if site is None:
            source = ROOT / 'compiler/pw-core/tests/device_store.rs'
            line = next(i for i, s in enumerate(source.read_text().splitlines(), 1) if s.strip().startswith('assert_eq!(found(src)'))
            site = f'compiler/pw-core/tests/device_store.rs:{line}:5'
        return (f'test one ... FAILED\n\nfailures:\n\n---- one stdout ----\n'
                f"thread 'one' panicked at {site}:\nassertion failed\n\nfailures:\n    one\n"
                'test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out\n')

    def test_cargo_authentic_assertion_and_green(self):
        self.assertEqual(device.cargo_result('core', 0, self.cargo()), (True, 1, 0))
        self.assertEqual(device.cargo_result('core', 101, self.cargo(True)), (True, 0, 1))

    def test_cargo_refuses_status_and_build_errors(self):
        for code in (None, -9, 1, 0):
            self.assertFalse(device.cargo_result('core', code, self.cargo(True))[0])
        self.assertFalse(device.cargo_result('core', 101, self.cargo(True) + 'error[E123]: build failed')[0])

    def test_cargo_refuses_nonassertion_panics(self):
        for site in ('compiler/pw-core/src/device_store.rs:1:1', 'compiler/pw-core/tests/device_store.rs:1:1', '/tmp/foreign.rs:1:1'):
            self.assertFalse(device.cargo_result('core', 101, self.cargo(True, site))[0])

    def test_cargo_refuses_assertion_operand_panics(self):
        genuine = self.cargo(True)
        self.assertEqual(device.cargo_result('core', 101, genuine), (True, 0, 1))
        # Same line, but inside the operand rather than the assertion macro.
        operand = genuine.replace(':5:\nassertion failed', ':27:\ncalled `Option::unwrap()` on a `None` value')
        self.assertFalse(device.cargo_result('core', 101, operand)[0])
        custom = genuine.replace('assertion failed', 'custom expected behavior')
        self.assertEqual(device.cargo_result('core', 101, custom), (True, 0, 1))

    def test_cargo_inventory_must_be_complete_and_frozen(self):
        self.assertTrue(device.cargo_result('core', 0, self.cargo())[0])
        self.assertFalse(device.cargo_result('core', 0, self.cargo().replace('test one', 'test other'))[0])
        self.assertFalse(device.cargo_result('core', 101, self.cargo(True).replace('test one ... FAILED\n', ''))[0])
        for marker in ('ignored', 'measured'):
            device.INVENTORIES.clear()
            self.assertFalse(device.cargo_result('core', 0, self.cargo().replace('0 ' + marker, '1 ' + marker))[0])
        self.assertFalse(device.cargo_result('core', 0, 'test skipped ... ignored\n' + self.cargo())[0])

    def campaign(self, result, restored=True):
        import mutation_baseline
        with tempfile.TemporaryDirectory() as tmp:
            source = Path(tmp) / 'source'; source.write_text('original')
            suite = mock.Mock(side_effect=[(True, 1, 0), result])
            output = io.StringIO()
            with mock.patch.object(device, 'SUITES', {'core': suite}), \
                 mock.patch.object(device, 'MUTANTS', [('control', 'core', source, 'original', 'mutant')]), \
                 mock.patch.object(device, 'built', return_value=restored), \
                 mock.patch.object(device.signal, 'signal'), \
                 mock.patch.object(mutation_baseline.mutation_bound, 'start', return_value=types.SimpleNamespace(stopped=False)), \
                 contextlib.redirect_stdout(output):
                code = device.main()
            self.assertEqual(source.read_text(), 'original')
            return code, output.getvalue()

    def test_invalid_producer_earns_no_kill(self):
        code, output = self.campaign((False, 0, 1))
        self.assertEqual(code, 1)
        self.assertIn('REFUSED', output)
        self.assertNotIn('KILLED', output)
        self.assertIn('0 of 1 mutants killed', output)

    def test_restored_build_failure_fails_campaign(self):
        code, output = self.campaign((True, 0, 1), restored=False)
        self.assertEqual(code, 1)
        self.assertIn('restored source did not build', output)

    def test_authentic_assertion_kills(self):
        code, output = self.campaign((True, 0, 1))
        self.assertEqual(code, 0)
        self.assertIn('1 of 1 mutants killed', output)

    def test_build_failure_and_timeout_are_invalid(self):
        for status in (1, None, -9):
            with mock.patch.object(device, 'bounded', return_value=('build failed', status)), contextlib.redirect_stdout(io.StringIO()):
                self.assertFalse(device.built())

    def test_cancellation_reaps_child_before_source_restoration(self):
        with tempfile.TemporaryDirectory() as tmp:
            source = Path(tmp) / 'source'; source.write_text('original')
            pidfile = Path(tmp) / 'pid'
            # Publish readiness by rename: existence now means the PID is complete,
            # rather than an empty file between create() and write().
            child = ("import os,time; from pathlib import Path; "
                     "target=Path(" + repr(str(pidfile)) + "); "
                     "ready=target.with_name(target.name+'.ready'); "
                     "ready.write_text(str(os.getpid())); ready.replace(target); "
                     "time.sleep(60)")
            program = "\n".join([
                "import signal,sys", "from pathlib import Path", "sys.path.insert(0," + repr(str(ROOT / 'scripts')) + ")",
                "import device_store_mutations as device", "signal.signal(signal.SIGTERM, lambda *_: sys.exit(1))",
                "source=Path(" + repr(str(source)) + ")", "source.write_text('mutant')", "try:",
                " device.bounded([sys.executable,'-c'," + repr(child) + "])", "finally:", " source.write_text('original')"])
            run = subprocess.Popen([sys.executable, '-c', program], stdout=subprocess.PIPE, stderr=subprocess.STDOUT, text=True)
            pid = None
            try:
                until = time.monotonic() + 5
                while not pidfile.exists() and time.monotonic() < until:
                    time.sleep(0.01)
                self.assertTrue(pidfile.exists(), 'child did not start')
                pid = int(pidfile.read_text())
                run.send_signal(signal.SIGTERM)
                output, _ = run.communicate(timeout=5)
                self.assertEqual(run.returncode, 1, output)
                self.assertEqual(source.read_text(), 'original')
                alive = True
                try:
                    os.kill(pid, 0)
                except ProcessLookupError:
                    alive = False
                self.assertFalse(alive, 'owned child survived source restoration')
            finally:
                if run.poll() is None:
                    run.kill(); run.communicate()
                if pid is not None:
                    try:
                        os.kill(pid, signal.SIGKILL)
                    except ProcessLookupError:
                        pass

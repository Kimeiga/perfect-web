"""The real hours mutation consumer, with only private process results mocked.

No Cargo process, signal, database, project mutation or memory stop is used.
"""
from contextlib import redirect_stdout
import io
import os
from pathlib import Path
import sys
import tempfile
import types
import unittest
from unittest.mock import patch

ROOT = Path(os.environ.get('PW_HOURS_CONSUMER_ROOT', Path(__file__).resolve().parents[2]))
sys.path.insert(0, str(ROOT / 'scripts'))
PASS = 'test result: ok. 1 passed; 0 failed; 0 ignored;\n'
FAIL = 'test a_real_assertion ... FAILED\ntest result: FAILED. 0 passed; 1 failed; 0 ignored;\n'


def consumer():
    path = ROOT / 'scripts/store_hours_mutations.py'
    scope = {'__file__': str(path), '__name__': '_actual_hours_consumer'}
    exec(compile(path.read_bytes(), str(path), 'exec'), scope)
    return scope


class StoreHoursResults(unittest.TestCase):
    def test_complete_success_and_assertion_failure_keep_their_counts(self):
        for output, status, expected in [(PASS, 0, (True, 1, 0)), (FAIL, 101, (True, 0, 1))]:
            with self.subTest(status=status):
                actual = consumer()
                actual['bounded'] = lambda *args: (output, status)
                self.assertEqual(actual['cargo_tests']('server'), expected)

    def test_stopped_empty_build_and_contradictory_native_results_are_refused(self):
        invalid = [
            ('', None), (FAIL, 2), (FAIL, 0),
            ('test result: ok. 0 passed; 0 failed; 0 ignored;\n', 0),
            ('error[E0308]: mismatched types\n' + FAIL, 101),
        ]
        for output, status in invalid:
            with self.subTest(status=status, output=output):
                actual = consumer()
                actual['bounded'] = lambda *args: (output, status)
                self.assertEqual(actual['cargo_tests']('server'), (False, 0, 0))

    def private_main(self, stopped):
        actual = consumer()
        watch = types.SimpleNamespace(stopped=False)
        recorder = types.SimpleNamespace(
            mutation_bound=types.SimpleNamespace(start=lambda: watch), explain=lambda: None)
        calls = []
        with tempfile.TemporaryDirectory(prefix='hours-consumer-test-') as directory:
            source = Path(directory) / 'only-private-fixture.txt'
            source.write_text('original\n')
            actual['CARGO'] = {'server': (['private-command-never-executed'], {})}
            actual['MUTANTS'] = [
                ('first', 'server', source, 'original\n', 'first\n'),
                ('must never run', 'server', source, 'original\n', 'second\n'),
            ]

            def results(*args):
                calls.append(source.read_text())
                if len(calls) == 1:
                    return PASS, 0
                if stopped:
                    watch.stopped = True
                    return FAIL, 101
                return '', None

            actual['bounded'] = results
            with patch.dict(sys.modules, {'mutation_baseline': recorder}), \
                    patch.dict(os.environ, {'PW_STORE_DATABASE_URL': 'private-mocked-only'}), \
                    patch.object(actual['signal'], 'signal'), redirect_stdout(io.StringIO()):
                status = actual['main']()
            self.assertEqual(source.read_text(), 'original\n', 'a refused control restores its only private file')
            self.assertEqual(status, 1, 'invalid/stopped evidence cannot make the main succeed')
            self.assertEqual(calls, ['original\n', 'first\n'], 'no later control is scheduled')

    def test_invalid_control_restores_and_stops_before_a_later_control(self):
        self.private_main(stopped=False)

    def test_a_stopped_watch_refuses_and_restores_without_a_later_control(self):
        self.private_main(stopped=True)


if __name__ == '__main__':
    unittest.main(verbosity=2)

"""Native process ownership, including cancellation between Popen and assignment."""
import json
import os
from pathlib import Path
import signal
import subprocess
import sys
import tempfile
import time
import unittest

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / 'scripts'))
import producer_process

PRODUCERS = ('case_and_delivery_mutations', 'template_mutations',
             'device_store_mutations', 'qualified_device_mutations')


class ProducerOwnership(unittest.TestCase):
    def probe(self, producer, phase, disposition='callable'):
        with tempfile.TemporaryDirectory() as directory:
            tmp = Path(directory)
            source, pidfile, receipt, reaped = (tmp / name for name in ('source', 'pid', 'receipt', 'reaped'))
            source.write_text('original')
            program = r"""
import json, os, signal, subprocess, sys
from pathlib import Path
sys.path.insert(0, SCRIPTS)
import importlib
producer = importlib.import_module(PRODUCER)
source, pidfile, receipt, reaped = map(Path, PATHS)
if DISPOSITION == 'callable':
    signal.signal(signal.SIGTERM, lambda *_: sys.exit(37))
elif DISPOSITION == 'default':
    signal.signal(signal.SIGTERM, signal.SIG_DFL)
native = subprocess.Popen
class Injected(native):
    def __init__(self, *args, **kwargs):
        super().__init__([sys.executable, '-c', 'import time; time.sleep(60)'], **kwargs)
        pidfile.write_text(str(self.pid))
        if PHASE in ('launch', 'cleanup'):
            os.kill(os.getpid(), signal.SIGTERM)
    def communicate(self, *args, **kwargs):
        if PHASE == 'cleanup' and not args and 'timeout' not in kwargs:
            os.kill(os.getpid(), signal.SIGTERM)
        result = super().communicate(*args, **kwargs)
        reaped.write_text(str(self.returncode))
        return result
subprocess.Popen = Injected
source.write_text('mutant')
try:
    if PRODUCER in ('case_and_delivery_mutations', 'template_mutations'):
        producer.TESTS = [['unused']]
        producer.run_tests()
    elif PRODUCER == 'device_store_mutations':
        producer.bounded(['unused'])
    else:
        producer.run()
finally:
    pid = int(pidfile.read_text())
    try:
        os.kill(pid, 0)
        alive = True
    except ProcessLookupError:
        alive = False
    source.write_text('original')
    receipt.write_text(json.dumps({'alive_at_restore': alive, 'source': source.read_text(), 'reaped': reaped.exists()}))
"""
            settings = '\n'.join(f'{name} = {value!r}' for name, value in [
                ('SCRIPTS', str(ROOT / 'scripts')), ('PRODUCER', producer),
                ('PATHS', list(map(str, (source, pidfile, receipt, reaped)))),
                ('PHASE', phase), ('DISPOSITION', disposition)])
            wrapper = subprocess.Popen([sys.executable, '-c', settings + '\n' + program],
                                       stdout=subprocess.PIPE, stderr=subprocess.STDOUT, text=True)
            pid = None
            try:
                end = time.monotonic() + 5
                while not pidfile.exists() and wrapper.poll() is None and time.monotonic() < end:
                    time.sleep(.01)
                self.assertTrue(pidfile.exists(), 'the native owned child did not launch')
                pid = int(pidfile.read_text())
                if phase == 'running':
                    wrapper.send_signal(signal.SIGTERM)
                output, _ = wrapper.communicate(timeout=5)
                expected = 37 if disposition == 'callable' else 128 + signal.SIGTERM
                self.assertEqual(wrapper.returncode, expected, output)
                self.assertTrue(receipt.exists(), 'cancellation skipped source restoration: ' + output)
                self.assertEqual(json.loads(receipt.read_text()),
                                 {'alive_at_restore': False, 'source': 'original', 'reaped': True}, output)
                self.assertEqual(source.read_text(), 'original')
            finally:
                if wrapper.poll() is None:
                    wrapper.kill()
                    wrapper.communicate()
                if pid is not None:
                    try:
                        os.kill(pid, signal.SIGKILL)
                    except ProcessLookupError:
                        pass

    def test_launch_cancellation_reaps_before_all_four_restore(self):
        for producer in PRODUCERS:
            with self.subTest(producer=producer):
                self.probe(producer, 'launch')

    def test_running_cancellation_reaps_before_all_four_restore(self):
        for producer in PRODUCERS:
            with self.subTest(producer=producer):
                self.probe(producer, 'running')

    def test_repeated_cancellation_during_cleanup_cannot_skip_restoration(self):
        for producer in PRODUCERS:
            with self.subTest(producer=producer):
                self.probe(producer, 'cleanup')

    def test_default_cancellation_raises_through_source_restoration(self):
        for producer in PRODUCERS:
            with self.subTest(producer=producer):
                self.probe(producer, 'launch', 'default')


    def test_native_output_and_nonzero_status_are_unchanged(self):
        result = producer_process.run([sys.executable, '-c',
            "import sys; print('out', flush=True); print('err', file=sys.stderr); sys.exit(17)"], timeout=5)
        self.assertEqual(result.returncode, 17)
        self.assertEqual(result.stdout, 'out\nerr\n')

    def test_timeout_reaps_and_preserves_native_partial_output(self):
        with tempfile.TemporaryDirectory() as directory:
            pidfile = Path(directory) / 'pid'
            child = "import os,time; from pathlib import Path; Path(" + repr(str(pidfile)) + ").write_text(str(os.getpid())); print('partial', flush=True); time.sleep(60)"
            pid = None
            try:
                with self.assertRaises(subprocess.TimeoutExpired) as expired:
                    producer_process.run([sys.executable, '-c', child], timeout=.2)
                self.assertEqual(expired.exception.output, 'partial\n')
                self.assertTrue(pidfile.exists())
                pid = int(pidfile.read_text())
                with self.assertRaises(ProcessLookupError):
                    os.kill(pid, 0)
            finally:
                if pid is None and pidfile.exists():
                    pid = int(pidfile.read_text())
                if pid is not None:
                    try:
                        os.kill(pid, signal.SIGKILL)
                    except ProcessLookupError:
                        pass

    def test_child_default_and_ignored_signals_keep_native_semantics(self):
        original = signal.getsignal(signal.SIGTERM)
        try:
            for handler, code, output in [(lambda *_: None, -signal.SIGTERM, ''),
                                           (signal.SIG_IGN, 0, 'continued\n')]:
                signal.signal(signal.SIGTERM, handler)
                result = producer_process.run([sys.executable, '-c',
                    "import os,signal; os.kill(os.getpid(),signal.SIGTERM); print('continued')"], timeout=5)
                self.assertEqual((result.returncode, result.stdout), (code, output))
                self.assertIs(signal.getsignal(signal.SIGTERM), handler)
        finally:
            signal.signal(signal.SIGTERM, original)

    def test_ignored_parent_cancellation_stays_ignored(self):
        original = signal.getsignal(signal.SIGTERM)
        try:
            signal.signal(signal.SIGTERM, signal.SIG_IGN)
            try:
                result = producer_process.run([sys.executable, '-c',
                    "import os,signal; os.kill(os.getppid(),signal.SIGTERM); print('completed')"], timeout=5)
            except SystemExit as stopped:
                self.fail(f'an ignored signal cancelled the parent: {stopped}')
            self.assertEqual((result.returncode, result.stdout), (0, 'completed\n'))
            self.assertEqual(signal.getsignal(signal.SIGTERM), signal.SIG_IGN)
        finally:
            signal.signal(signal.SIGTERM, original)

    def test_child_does_not_inherit_a_blocked_cancellation_signal(self):
        result = producer_process.run([sys.executable, '-c',
            "import signal; print(int(signal.SIGTERM in signal.pthread_sigmask(signal.SIG_BLOCK, [])))"], timeout=5)
        self.assertEqual((result.returncode, result.stdout), (0, '0\n'))

    def test_launch_error_restores_original_handlers(self):
        before = {number: signal.getsignal(number) for number in (signal.SIGINT, signal.SIGTERM)}
        try:
            with self.assertRaises(FileNotFoundError):
                producer_process.run(['/this/producer/does/not/exist'], timeout=5)
            self.assertEqual({number: signal.getsignal(number) for number in before}, before)
        finally:
            for number, handler in before.items():
                signal.signal(number, handler)


if __name__ == '__main__':
    unittest.main()

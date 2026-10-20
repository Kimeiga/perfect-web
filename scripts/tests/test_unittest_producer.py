"""Real private producers: provenance, complete IDs and dedicated output files."""
import importlib.util
import json
import os
from pathlib import Path
import signal
import subprocess
import sys
import tempfile
import time
import unittest
from unittest.mock import Mock, patch

SCRIPTS = Path(__file__).resolve().parent.parent
sys.path.insert(0, str(SCRIPTS))
spec = importlib.util.spec_from_file_location('private_producer_harness', SCRIPTS / 'mutation_slots_mutations.py')
harness = importlib.util.module_from_spec(spec)
spec.loader.exec_module(harness)
PRODUCER = SCRIPTS / 'unittest_producer.py'


@unittest.skipUnless(sys.platform.startswith('linux') and hasattr(os, 'pidfd_open')
                     and hasattr(signal, 'pidfd_send_signal'), 'Linux pidfd private producers')
class ActualProducers(unittest.TestCase):
    def setUp(self):
        self.folder = Path(tempfile.mkdtemp(prefix='unittest-provenance-'))
        self.env = harness.clean_environment()
        self.env['PYTHONPATH'] = str(self.folder) + os.pathsep + self.env.get('PYTHONPATH', '')
        self.number = 0

    def command(self, args, bound=20):
        self.number += 1
        log = self.folder / ('output-' + str(self.number) + '.txt')
        status = harness.execute([sys.executable, '-B', str(PRODUCER), *args], log, self.env, bound)
        return status, log

    def produce(self, source, change_ids=False):
        (self.folder / 'private_fixture.py').write_text('import unittest\n' + source)
        inventory = self.folder / 'inventory.json'
        status, _ = self.command(['--report', str(inventory), '--inventory', 'private_fixture'])
        self.assertEqual(status, 0)
        ids = json.loads(inventory.read_text())['ids']
        frozen = self.folder / 'expected.json'
        frozen.write_text(json.dumps(ids + ['private_fixture.Other.test_absent'] if change_ids else ids))
        report = self.folder / 'report.json'
        status, log = self.command(['--report', str(report), '--expected', str(frozen), 'private_fixture'])
        text = report.read_text() if report.exists() else ''
        return harness.python_result(status, text, ids), status, text, log

    def test_actual_assertion_is_one_failed_method(self):
        result, status, text, _ = self.produce('class Case(unittest.TestCase):\n def test_body(self): self.fail("actual assertion")\n')
        self.assertEqual(result, (True, 0, 1))
        self.assertEqual(json.loads(text)['events'], [{'id': 'private_fixture.Case.test_body', 'kind': 'failure', 'origin': 'body'}])

    def test_actual_unexpected_body_exception_is_one_failed_method(self):
        result, *_ = self.produce('class Case(unittest.TestCase):\n def test_body(self): raise ValueError("actual body exception")\n')
        self.assertEqual(result, (True, 0, 1))

    def test_multiple_failed_subtests_keep_distinct_method_count(self):
        source = ('class Case(unittest.TestCase):\n def test_body(self):\n'
                  '  for i in range(3):\n   with self.subTest(i=i):\n'
                  '    if i == 1: raise ValueError("body subtest")\n'
                  '    self.fail("body subtest")\n'
                  ' def test_green(self): pass\n')
        result, _, text, _ = self.produce(source)
        self.assertEqual(result, (True, 1, 1))
        self.assertEqual(len(json.loads(text)['events']), 4)

    def test_failed_subtest_followed_by_body_error_is_one_method(self):
        result, *_ = self.produce('class Case(unittest.TestCase):\n def test_body(self):\n  with self.subTest(): self.fail("subtest")\n  raise ValueError("body")\n')
        self.assertEqual(result, (True, 0, 1))

    def test_successful_subtests_and_plain_method_are_real_passes(self):
        result, *_ = self.produce('class Case(unittest.TestCase):\n def test_body(self):\n  with self.subTest(): self.assertTrue(True)\n def test_green(self): pass\n')
        self.assertEqual(result, (True, 2, 0))

    def test_actual_setup_error_is_refused(self):
        result, _, text, _ = self.produce('class Case(unittest.TestCase):\n def setUp(self): raise ValueError("setup")\n def test_body(self): pass\n')
        self.assertEqual(result, (False, 0, 0))
        self.assertEqual(json.loads(text)['bodies'][0]['entered'], 0)

    def test_actual_teardown_error_is_refused_after_body(self):
        result, _, text, _ = self.produce('class Case(unittest.TestCase):\n def tearDown(self): raise ValueError("teardown")\n def test_body(self): pass\n')
        self.assertEqual(result, (False, 0, 0))
        self.assertEqual(json.loads(text)['bodies'][0]['entered'], 1)
        self.assertEqual(json.loads(text)['events'][0]['origin'], 'fixture')

    def test_same_exception_reraised_from_teardown_still_refuses(self):
        source = ('class Case(unittest.TestCase):\n def test_body(self):\n'
                  '  self.failure = ValueError("shared"); raise self.failure\n'
                  ' def tearDown(self): raise self.failure\n')
        result, _, text, _ = self.produce(source)
        self.assertEqual(result, (False, 0, 0))
        self.assertEqual([e['origin'] for e in json.loads(text)['events']], ['body', 'fixture'])

    def test_actual_cleanup_error_is_refused(self):
        result, *_ = self.produce('class Case(unittest.TestCase):\n def test_body(self): self.addCleanup(lambda: self.fail("cleanup"))\n')
        self.assertEqual(result, (False, 0, 0))

    def test_actual_class_setup_error_is_refused(self):
        result, _, text, _ = self.produce('class Case(unittest.TestCase):\n @classmethod\n def setUpClass(cls): raise ValueError("class setup")\n def test_body(self): pass\n')
        self.assertEqual(result, (False, 0, 0))
        self.assertEqual(json.loads(text)['started'], [])

    def test_actual_class_teardown_error_is_refused(self):
        result, *_ = self.produce('class Case(unittest.TestCase):\n @classmethod\n def tearDownClass(cls): raise ValueError("class teardown")\n def test_body(self): pass\n')
        self.assertEqual(result, (False, 0, 0))

    def test_actual_module_setup_error_is_refused(self):
        result, *_ = self.produce('def setUpModule(): raise ValueError("module setup")\nclass Case(unittest.TestCase):\n def test_body(self): pass\n')
        self.assertEqual(result, (False, 0, 0))

    def test_actual_module_teardown_error_is_refused(self):
        result, *_ = self.produce('def tearDownModule(): raise ValueError("module teardown")\nclass Case(unittest.TestCase):\n def test_body(self): pass\n')
        self.assertEqual(result, (False, 0, 0))

    def test_actual_missing_import_runs_no_method_and_refuses(self):
        report, frozen = self.folder / 'report.json', self.folder / 'expected.json'
        frozen.write_text('["absent_fixture.Case.test_body"]')
        status, log = self.command(['--report', str(report), '--expected', str(frozen), 'absent_fixture'])
        self.assertEqual(status, 2)
        self.assertFalse(report.exists())
        self.assertIn('loader/import failure', log.read_text())
        self.assertEqual(harness.python_result(status, '', ['absent_fixture.Case.test_body']), (False, 0, 0))

    def test_actual_import_exception_refuses_before_inventory(self):
        (self.folder / 'private_fixture.py').write_text('raise RuntimeError("module import")\n')
        report = self.folder / 'report.json'
        status, log = self.command(['--report', str(report), '--inventory', 'private_fixture'])
        self.assertEqual(status, 2)
        self.assertFalse(report.exists())
        self.assertIn('module import', log.read_text())

    def test_actual_skip_refuses(self):
        result, *_ = self.produce('class Case(unittest.TestCase):\n @unittest.skip("skip")\n def test_body(self): pass\n')
        self.assertEqual(result, (False, 0, 0))

    def test_actual_expected_failure_refuses(self):
        result, *_ = self.produce('class Case(unittest.TestCase):\n @unittest.expectedFailure\n def test_body(self): self.fail("expected")\n')
        self.assertEqual(result, (False, 0, 0))

    def test_actual_unexpected_success_refuses(self):
        result, *_ = self.produce('class Case(unittest.TestCase):\n @unittest.expectedFailure\n def test_body(self): pass\n')
        self.assertEqual(result, (False, 0, 0))

    def test_actual_partial_positive_inventory_refuses(self):
        result, status, text, _ = self.produce('class Case(unittest.TestCase):\n def test_body(self): pass\n', change_ids=True)
        self.assertEqual(result, (False, 0, 0))
        self.assertEqual(status, 2)
        self.assertEqual(text, '')

    def test_actual_early_stop_has_incomplete_inventory_and_refuses(self):
        result, _, text, _ = self.produce('class Case(unittest.TestCase):\n def test_a(self): self._outcome.result.stop()\n def test_b(self): pass\n')
        self.assertEqual(result, (False, 0, 0))
        self.assertEqual(len(json.loads(text)['started']), 1)

    def test_fixture_subtest_failure_never_becomes_a_body_failure(self):
        result, _, text, _ = self.produce('class Case(unittest.TestCase):\n def tearDown(self):\n  with self.subTest(): self.fail("fixture subtest")\n def test_body(self): pass\n')
        self.assertEqual(result, (False, 0, 0))
        self.assertEqual(json.loads(text)['events'][0]['origin'], 'fixture')

    def test_body_skip_and_skipped_subtest_refuse(self):
        result, *_ = self.produce('class Case(unittest.TestCase):\n def test_body(self):\n  with self.subTest(): self.skipTest("body skip")\n')
        self.assertEqual(result, (False, 0, 0))

    def test_custom_execution_refuses_before_loading_inventory(self):
        (self.folder / 'private_fixture.py').write_text('import unittest\nclass Case(unittest.TestCase):\n def run(self, result=None): return result\n def test_body(self): pass\n')
        report = self.folder / 'report.json'
        status, log = self.command(['--report', str(report), '--inventory', 'private_fixture'])
        self.assertEqual(status, 2)
        self.assertFalse(report.exists())
        self.assertIn('custom execution', log.read_text())

    def test_async_generator_methods_and_fixtures_refuse_before_inventory(self):
        for method in ('async def test_body(self): pass', 'def test_body(self): yield 1',
                       'async def setUp(self): pass\n def test_body(self): pass'):
            self.number += 1
            name = 'async_fixture' + str(self.number)
            (self.folder / (name + '.py')).write_text('import unittest\nclass Case(unittest.TestCase):\n ' + method + '\n')
            report = self.folder / (name + '.json')
            status, log = self.command(['--report', str(report), '--inventory', name])
            self.assertEqual(status, 2, log.read_text())
            self.assertFalse(report.exists())

    def test_actual_crash_has_no_report(self):
        result, status, text, _ = self.produce('class Case(unittest.TestCase):\n def test_body(self):\n  import os; os._exit(7)\n')
        self.assertEqual(status, 7)
        self.assertEqual(text, '')
        self.assertEqual(result, (False, 0, 0))

    def test_actual_timeout_has_bounded_pidfd_cleanup_and_file_output(self):
        before = time.monotonic()
        output = self.folder / 'timeout.txt'
        with self.assertRaisesRegex(RuntimeError, 'timed out'):
            harness.execute([sys.executable, '-c', 'import time; print("started",flush=True); time.sleep(120)'],
                            output, self.env, bound=.2)
        self.assertLess(time.monotonic() - before, 8)
        self.assertEqual(output.read_text(), 'started\n')
        with output.open('ab') as stream:
            stream.write(b'closed parent output\n')

    def test_ordinary_discovery_without_host_pidfd_apis_has_explicit_skips(self):
        code = ('import os,signal,sys,unittest; '
                'del os.pidfd_open; del signal.pidfd_send_signal; sys.platform="darwin"; sys.excepthook=sys.__excepthook__; '
                'from scripts.tests.test_mutation_slot_processes import RealPrivateChildren; '
                'from scripts.tests.test_mutation_slots import Toy; '
                'from scripts.tests.test_unittest_producer import ActualProducers; '
                'assert RealPrivateChildren.__unittest_skip__ and Toy.__unittest_skip__ '
                'and ActualProducers.__unittest_skip__; '
                'loader=unittest.TestLoader(); '
                'suite=loader.discover("scripts/tests",pattern="test_mutation_slot*.py"); '
                'suite.addTests(loader.loadTestsFromName("scripts.tests.test_unittest_producer")); '
                'result=unittest.TextTestRunner(verbosity=1).run(suite); '
                'assert result.testsRun==165 and len(result.skipped)==81, (result.testsRun,len(result.skipped)); '
                'assert result.testsRun-len(result.skipped)==84; '
                'print("ordinary discovery: 165 methods, 84 passed, 81 explicit Linux skips"); '
                'sys.exit(0 if result.wasSuccessful() else 1)')
        output = self.folder / 'nonlinux-discovery.txt'
        status = harness.execute([sys.executable, '-B', '-c', code], output, self.env, bound=30)
        self.assertEqual(status, 0, output.read_text())
        self.assertIn('84 passed, 81 explicit Linux skips', output.read_text())

    def test_preexisting_report_is_never_reused(self):
        report = self.folder / 'report.json'; report.write_text('old bytes')
        status, _ = self.command(['--report', str(report), '--inventory', 'unittest'])
        self.assertEqual(status, 2)
        self.assertEqual(report.read_text(), 'old bytes')


    def direct_fixture(self, source):
        (self.folder / 'private_fixture.py').write_text('import unittest\n' + source)
        ids = ['private_fixture.Case.test_body']
        frozen = self.folder / 'expected.json'; frozen.write_text(json.dumps(ids))
        report = self.folder / 'report.json'
        status, log = self.command(['--report', str(report), '--expected', str(frozen), 'private_fixture'])
        text = report.read_text() if report.exists() else ''
        return harness.python_result(status, text, ids), status, text, log

    def test_async_generator_body_is_refused(self):
        result, status, text, log = self.direct_fixture('class Case(unittest.TestCase):\n async def test_body(self):\n  self.fail("actual body never executes")\n  yield 1\n')
        self.assertEqual(result, (False, 0, 0), log.read_text())
        self.assertEqual(status, 2, log.read_text())

    def test_async_generator_setup_is_refused(self):
        result, status, text, log = self.direct_fixture('class Case(unittest.TestCase):\n async def setUp(self):\n  self.fail("actual fixture never executes")\n  yield 1\n def test_body(self): self.fail("method assertion with unsupported fixture")\n')
        self.assertEqual(result, (False, 0, 0), log.read_text())
        self.assertEqual(status, 2, log.read_text())

    def test_deferred_setup_is_refused(self):
        result, status, text, log = self.direct_fixture('def deferred():\n yield 1\nclass Case(unittest.TestCase):\n def setUp(self): return deferred()\n def test_body(self): self.fail("method assertion with unsupported fixture")\n')
        self.assertEqual(result, (False, 0, 0), log.read_text())
        self.assertEqual(status, 2, log.read_text())

    def test_deferred_teardown_is_refused(self):
        result, status, text, log = self.direct_fixture('def deferred():\n yield 1\nclass Case(unittest.TestCase):\n def tearDown(self): return deferred()\n def test_body(self): self.fail("method assertion with unsupported fixture")\n')
        self.assertEqual(result, (False, 0, 0), log.read_text())
        self.assertEqual(status, 2, log.read_text())

    def test_deferred_cleanup_is_refused(self):
        result, status, text, log = self.direct_fixture('def deferred():\n yield 1\nclass Case(unittest.TestCase):\n def test_body(self):\n  self.addCleanup(lambda: deferred())\n  self.fail("method assertion with unsupported fixture")\n')
        self.assertEqual(result, (False, 0, 0), log.read_text())
        self.assertEqual(status, 2, log.read_text())

    def test_deferred_class_setup_is_refused(self):
        result, status, text, log = self.direct_fixture('def deferred():\n yield 1\nclass Case(unittest.TestCase):\n @classmethod\n def setUpClass(cls): return deferred()\n def test_body(self): self.fail("method assertion with unsupported fixture")\n')
        self.assertEqual(result, (False, 0, 0), log.read_text())
        self.assertEqual(status, 2, log.read_text())

    def test_deferred_class_teardown_is_refused(self):
        result, status, text, log = self.direct_fixture('def deferred():\n yield 1\nclass Case(unittest.TestCase):\n @classmethod\n def tearDownClass(cls): return deferred()\n def test_body(self): self.fail("method assertion with unsupported fixture")\n')
        self.assertEqual(result, (False, 0, 0), log.read_text())
        self.assertEqual(status, 2, log.read_text())

    def test_deferred_module_setup_is_refused(self):
        result, status, text, log = self.direct_fixture('def deferred():\n yield 1\ndef setUpModule(): return deferred()\nclass Case(unittest.TestCase):\n def test_body(self): self.fail("method assertion with unsupported fixture")\n')
        self.assertEqual(result, (False, 0, 0), log.read_text())
        self.assertEqual(status, 2, log.read_text())

    def test_deferred_module_teardown_is_refused(self):
        result, status, text, log = self.direct_fixture('def deferred():\n yield 1\ndef tearDownModule(): return deferred()\nclass Case(unittest.TestCase):\n def test_body(self): self.fail("method assertion with unsupported fixture")\n')
        self.assertEqual(result, (False, 0, 0), log.read_text())
        self.assertEqual(status, 2, log.read_text())

    def test_deferred_dynamic_class_cleanup_is_refused(self):
        result, status, text, log = self.direct_fixture('def deferred():\n yield 1\nclass Case(unittest.TestCase):\n @classmethod\n def setUpClass(cls): cls.addClassCleanup(lambda: deferred())\n def test_body(self): self.fail("method assertion with unsupported fixture")\n')
        self.assertEqual(result, (False, 0, 0), log.read_text())
        self.assertEqual(status, 2, log.read_text())

    def test_deferred_dynamic_module_cleanup_is_refused(self):
        result, status, text, log = self.direct_fixture('def deferred():\n yield 1\ndef setUpModule(): unittest.addModuleCleanup(lambda: deferred())\nclass Case(unittest.TestCase):\n def test_body(self): self.fail("method assertion with unsupported fixture")\n')
        self.assertEqual(result, (False, 0, 0), log.read_text())
        self.assertEqual(status, 2, log.read_text())

    def test_runtime_async_generator_body_is_refused(self):
        result, status, text, log = self.direct_fixture('async def deferred():\n yield 1\nclass Case(unittest.TestCase):\n def test_body(self): return deferred()\n')
        self.assertEqual(result, (False, 0, 0), log.read_text())
        self.assertEqual(status, 2, log.read_text())

    def test_runtime_async_generator_setup_is_refused(self):
        result, status, text, log = self.direct_fixture('async def deferred():\n yield 1\nclass Case(unittest.TestCase):\n def setUp(self): return deferred()\n def test_body(self): self.fail("body with unexecuted fixture")\n')
        self.assertEqual(result, (False, 0, 0), log.read_text())
        self.assertEqual(status, 2, log.read_text())

    def test_runtime_coroutine_cleanup_is_refused(self):
        result, status, text, log = self.direct_fixture('async def deferred(): return 1\nclass Case(unittest.TestCase):\n def test_body(self):\n  self.addCleanup(lambda: deferred())\n  self.fail("body with unexecuted cleanup")\n')
        self.assertEqual(result, (False, 0, 0), log.read_text())
        self.assertEqual(status, 2, log.read_text())
        self.assertNotIn('was never awaited', log.read_text())

    def test_custom_instance_queue_is_refused(self):
        result, status, text, log = self.direct_fixture('class Custom(list): pass\nclass Case(unittest.TestCase):\n def __init__(self,*a,**k):\n  super().__init__(*a,**k); self._cleanups=Custom()\n def test_body(self): self.fail("unsupported storage")\n')
        self.assertEqual(result, (False, 0, 0), log.read_text())
        self.assertEqual(status, 2, log.read_text())

    def test_replaced_instance_queue_is_refused(self):
        result, status, text, log = self.direct_fixture('class Case(unittest.TestCase):\n def test_body(self):\n  self._cleanups=[]; self.fail("replaced storage")\n')
        self.assertEqual(result, (False, 0, 0), log.read_text())
        self.assertEqual(status, 2, log.read_text())

    def test_custom_class_queue_is_refused(self):
        result, status, text, log = self.direct_fixture('class Custom(list): pass\nclass Case(unittest.TestCase):\n def test_body(self): self.fail("unsupported storage")\nCase._class_cleanups=Custom()\n')
        self.assertEqual(result, (False, 0, 0), log.read_text())
        self.assertEqual(status, 2, log.read_text())

    def test_replaced_class_queue_is_refused(self):
        result, status, text, log = self.direct_fixture('class Case(unittest.TestCase):\n def test_body(self):\n  type(self)._class_cleanups=[]; self.fail("replaced storage")\n')
        self.assertEqual(result, (False, 0, 0), log.read_text())
        self.assertEqual(status, 2, log.read_text())

    def test_custom_module_queue_is_refused(self):
        result, status, text, log = self.direct_fixture('class Custom(list): pass\nunittest.case._module_cleanups=Custom()\nclass Case(unittest.TestCase):\n def test_body(self): self.fail("unsupported storage")\n')
        self.assertEqual(result, (False, 0, 0), log.read_text())
        self.assertEqual(status, 2, log.read_text())

    def test_replaced_module_queue_is_refused(self):
        result, status, text, log = self.direct_fixture('class Case(unittest.TestCase):\n def test_body(self):\n  unittest.case._module_cleanups=[]; self.fail("replaced storage")\n')
        self.assertEqual(result, (False, 0, 0), log.read_text())
        self.assertEqual(status, 2, log.read_text())

    def test_synchronous_fixture_order_aliases_reentrant_cleanups_and_restoration(self):
        events = self.folder / 'events.json'
        restored = self.folder / 'restored.json'
        source = """import atexit,inspect,json
from pathlib import Path
from unittest import addModuleCleanup as alias
seen=[]
def mark(label, suffix=''): seen.append(label+suffix)
def module_pre():
 mark('module-pre')
 Path(EVENTS).write_text(json.dumps(seen))
alias(module_pre)
original_module_queue=unittest.case._module_cleanups
original_module_executor=unittest.case.doModuleCleanups
def module_new():
 mark('module-new'); alias(mark,'module-from-cleanup')
def setUpModule():
 mark('module-setup'); alias(module_new); return 7
def tearDownModule(): mark('module-teardown'); return object()
original_setup=setUpModule
original_teardown=tearDownModule
instances=[]
class Case(unittest.TestCase):
 def __init__(self,*a,**k):
  super().__init__(*a,**k); self.addCleanup(mark,'instance-pre'); instances.append((self,self._cleanups))
 @classmethod
 def setUpClass(cls):
  mark('class-setup')
  def later():
   mark('class-new'); cls.addClassCleanup(mark,'class-from-cleanup')
  cls.addClassCleanup(later); return 8
 @classmethod
 def tearDownClass(cls): mark('class-teardown'); return object()
 def setUp(self): mark('setup'); self.addCleanup(mark,'instance-setup'); return 9
 def tearDown(self): mark('teardown'); return object()
 def test_body(self):
  mark('body')
  def later():
   mark('instance-new'); self.addCleanup(mark,'instance-from-cleanup',suffix='-kw')
  self.addCleanup(later)
Case.addClassCleanup(mark,'class-pre')
original_class_queue=Case._class_cleanups
original_descriptors={name:Case.__dict__.get(name) for name in ('setUpClass','tearDownClass','setUp','tearDown','test_body','doClassCleanups')}
def verify():
 flags={'module-bindings':setUpModule is original_setup and tearDownModule is original_teardown and unittest.case.doModuleCleanups is original_module_executor,
        'descriptors':all(Case.__dict__.get(name) is value for name,value in original_descriptors.items()),
        'class-queue':Case._class_cleanups is original_class_queue and not original_class_queue,
        'module-queue':unittest.case._module_cleanups is original_module_queue and not original_module_queue,
        'instance-queues':all(test._cleanups is queue and not queue for test,queue in instances),
        'instance-bindings':all(not any(name in test.__dict__ for name in ('test_body','setUp','tearDown','subTest','doCleanups','_callCleanup')) for test,queue in instances)}
 Path(RESTORED).write_text(json.dumps(flags))
atexit.register(verify)
""".replace('EVENTS', repr(str(events))).replace('RESTORED', repr(str(restored)))
        result, status, text, log = self.produce(source)
        self.assertEqual(result, (True, 1, 0), log.read_text())
        observed = json.loads(events.read_text())
        self.assertTrue(all(json.loads(restored.read_text()).values()), restored.read_text())
        reference = self.folder / 'reference.txt'
        status = harness.execute([sys.executable, '-B', '-m', 'unittest', 'private_fixture'], reference, self.env, bound=20)
        self.assertEqual(status, 0, reference.read_text())
        self.assertEqual(observed, json.loads(events.read_text()), 'same actual stdlib fixture/LIFO semantics')
        self.assertTrue(all(json.loads(restored.read_text()).values()), restored.read_text())
        self.assertEqual(observed, ['module-setup','class-setup','setup','body','teardown',
                         'instance-new','instance-from-cleanup-kw','instance-setup','instance-pre',
                         'class-teardown','class-new','class-from-cleanup','class-pre',
                         'module-teardown','module-new','module-from-cleanup','module-pre'])

    def test_runtime_generator_result_is_closed_before_refusal(self):
        closed = self.folder / 'closed.txt'
        witnessed = self.folder / 'closed-before-body.txt'
        source = ("from pathlib import Path\nheld=[]\ndef deferred():\n try: yield 1\n finally: Path(" + repr(str(closed)) + ").write_text('closed')\n"
                  "class Case(unittest.TestCase):\n def setUp(self):\n  pending=deferred(); next(pending); held.append(pending); return pending\n"
                  " def test_body(self):\n  Path(" + repr(str(witnessed)) + ").write_text(str(Path(" + repr(str(closed)) + ").exists()))\n  self.fail('unsupported fixture')\n")
        result, status, text, log = self.direct_fixture(source)
        self.assertEqual(result, (False, 0, 0), log.read_text())
        self.assertEqual(status, 2)
        self.assertEqual(closed.read_text(), 'closed')
        self.assertEqual(witnessed.read_text(), 'True', 'close occurred before body, not only during process shutdown')


    def test_instance_cleanup_hook_is_refused(self):
        result, status, text, log = self.direct_fixture('class Case(unittest.TestCase):\n def test_body(self):\n  self.addCleanup(lambda:self.fail("fixture")); self.fail("body")\ndef load_tests(loader,tests,pattern):\n case=Case("test_body"); case._callCleanup=lambda *a,**k:None\n return unittest.TestSuite([case])\n')
        self.assertEqual(result, (False, 0, 0), log.read_text())
        self.assertEqual(status, 2, log.read_text())

    def test_base_cleanup_hook_is_refused(self):
        result, status, text, log = self.direct_fixture('unittest.TestCase._callCleanup=lambda *a,**k:None\nclass Case(unittest.TestCase):\n def test_body(self):\n  self.addCleanup(lambda:self.fail("fixture")); self.fail("body")\n')
        self.assertEqual(result, (False, 0, 0), log.read_text())
        self.assertEqual(status, 2, log.read_text())

    def test_custom_suite_fixture_omission_is_refused(self):
        result, status, text, log = self.direct_fixture('class Case(unittest.TestCase):\n @classmethod\n def setUpClass(cls): raise ValueError("fixture")\n def test_body(self): self.fail("body")\nclass Quiet(unittest.TestSuite):\n def run(self,result,debug=False):\n  for case in self: case.run(result)\n  return result\ndef load_tests(loader,tests,pattern): return Quiet([Case("test_body")])\n')
        self.assertEqual(result, (False, 0, 0), log.read_text())
        self.assertEqual(status, 2, log.read_text())

    def test_instance_suite_fixture_omission_is_refused(self):
        result, status, text, log = self.direct_fixture('class Case(unittest.TestCase):\n @classmethod\n def setUpClass(cls): raise ValueError("fixture")\n def test_body(self): self.fail("body")\ndef load_tests(loader,tests,pattern):\n case=Case("test_body"); suite=unittest.TestSuite([case])\n suite.run=lambda result,debug=False:case.run(result)\n return suite\n')
        self.assertEqual(result, (False, 0, 0), log.read_text())
        self.assertEqual(status, 2, log.read_text())

    def test_base_suite_fixture_dispatch_is_refused(self):
        result, status, text, log = self.direct_fixture('unittest.TestSuite._handleClassSetUp=lambda *a,**k:None\nclass Case(unittest.TestCase):\n @classmethod\n def setUpClass(cls): raise ValueError("fixture")\n def test_body(self): self.fail("body")\n')
        self.assertEqual(result, (False, 0, 0), log.read_text())
        self.assertEqual(status, 2, log.read_text())

    def test_skip_forwarding_hook_is_refused(self):
        result, status, text, log = self.direct_fixture('unittest.TestCase._addSkip=lambda self,result,test,reason:result.addSuccess(test)\nclass Case(unittest.TestCase):\n def test_body(self): self.skipTest("actual skip")\n')
        self.assertEqual(result, (False, 0, 0), log.read_text())
        self.assertEqual(status, 2, log.read_text())

    def test_cleanup_registration_hook_is_refused(self):
        result, status, text, log = self.direct_fixture('unittest.TestCase.addCleanup=lambda *a,**k:None\nclass Case(unittest.TestCase):\n def test_body(self):\n  self.addCleanup(lambda:self.fail("fixture")); self.fail("body")\n')
        self.assertEqual(result, (False, 0, 0), log.read_text())
        self.assertEqual(status, 2, log.read_text())

    def test_module_registration_alias_is_refused(self):
        result, status, text, log = self.direct_fixture('unittest.addModuleCleanup=lambda *a,**k:None\ndef setUpModule(): unittest.addModuleCleanup(lambda:(_ for _ in ()).throw(ValueError("fixture")))\nclass Case(unittest.TestCase):\n def test_body(self): self.fail("body")\n')
        self.assertEqual(result, (False, 0, 0), log.read_text())
        self.assertEqual(status, 2, log.read_text())

    def test_late_base_execution_hook_is_refused(self):
        result, status, text, log = self.direct_fixture('class Case(unittest.TestCase):\n def test_body(self):\n  unittest.TestCase._callCleanup=lambda *a,**k:None; self.fail("body")\n')
        self.assertEqual(result, (False, 0, 0), log.read_text())
        self.assertEqual(status, 2, log.read_text())

    def test_late_module_registration_alias_is_refused(self):
        result, status, text, log = self.direct_fixture('class Case(unittest.TestCase):\n def test_body(self):\n  unittest.addModuleCleanup=lambda *a,**k:None; self.fail("body")\n')
        self.assertEqual(result, (False, 0, 0), log.read_text())
        self.assertEqual(status, 2, log.read_text())

    def test_stock_suite_and_genuine_bound_hook_alias_are_supported(self):
        source = ('class Case(unittest.TestCase):\n def setUp(self): self.value=3\n def test_body(self): self.assertEqual(self.value,3)\n'
                  'def load_tests(loader,tests,pattern):\n case=Case("test_body")\n case._callCleanup=unittest.TestCase._callCleanup.__get__(case,Case)\n return unittest.TestSuite([case])\n')
        result, status, text, log = self.direct_fixture(source)
        self.assertEqual(result, (True, 1, 0), log.read_text())
        self.assertEqual(status, 0)


    def test_suite_global_dispatch_helpers_are_refused(self):
        for name in ('_isnotsuite', '_call_if_exists'):
            with self.subTest(name=name):
                source = ('unittest.suite.' + name + '=lambda *a,**k:False\n'
                          'class Case(unittest.TestCase):\n @classmethod\n'
                          ' def setUpClass(cls): raise OSError("required fixture")\n'
                          ' def test_body(self): self.fail("body")\n')
                result, status, text, log = self.direct_fixture(source)
                self.assertEqual(result, (False, 0, 0), log.read_text())
                self.assertEqual(status, 2)

    def test_runner_execution_cannot_omit_required_fixture(self):
        source = ('def altered(self,test):\n result=self._makeResult()\n'
                  ' def walk(node):\n  if isinstance(node,unittest.TestSuite):\n'
                  '   for child in node: walk(child)\n  else: node.run(result)\n'
                  ' walk(test)\n return result\nunittest.TextTestRunner.run=altered\n'
                  'class Case(unittest.TestCase):\n @classmethod\n'
                  ' def setUpClass(cls): raise OSError("required fixture")\n'
                  ' def test_body(self): self.fail("body")\n')
        result, status, text, log = self.direct_fixture(source)
        self.assertEqual(result, (False, 0, 0), log.read_text())
        self.assertEqual(status, 2)

    def test_runner_canonical_descriptors_and_aliases_are_refused(self):
        for edit in ('unittest.TextTestRunner.__init__=lambda *a,**k:None',
                     'unittest.TextTestRunner._makeResult=lambda *a,**k:None',
                     'unittest.TextTestRunner=type("Changed",(unittest.runner.TextTestRunner,),{})',
                     'unittest.runner.TextTestRunner=type("Changed",(unittest.TextTestRunner,),{})'):
            with self.subTest(edit=edit):
                result, status, text, log = self.direct_fixture(edit+'\nclass Case(unittest.TestCase):\n def test_body(self): self.fail("body")\n')
                self.assertEqual(result, (False, 0, 0), log.read_text())
                self.assertEqual(status, 2)

    def test_outcome_executor_cannot_suppress_required_fixture(self):
        source = ('from contextlib import contextmanager\n'
                  'original=unittest.case._Outcome.testPartExecutor\n@contextmanager\n'
                  'def changed(self,test_case,isTest=False):\n if isTest:\n'
                  '  with original(self,test_case,isTest): yield\n else:\n'
                  '  try: yield\n  except Exception: pass\n'
                  'unittest.case._Outcome.testPartExecutor=changed\n'
                  'class Case(unittest.TestCase):\n def setUp(self): raise OSError("required fixture")\n'
                  ' def test_body(self): self.fail("body")\n')
        result, status, text, log = self.direct_fixture(source)
        self.assertEqual(result, (False, 0, 0), log.read_text())
        self.assertEqual(status, 2)

    def test_outcome_canonical_constructor_and_alias_are_refused(self):
        for edit in ('unittest.case._Outcome.__init__=lambda *a,**k:None',
                     'unittest.case._Outcome=type("Changed",(unittest.case._Outcome,),{})'):
            with self.subTest(edit=edit):
                result, status, text, log = self.direct_fixture(edit+'\nclass Case(unittest.TestCase):\n def test_body(self): self.fail("body")\n')
                self.assertEqual(result, (False, 0, 0), log.read_text())
                self.assertEqual(status, 2)

    def test_late_dispatch_binding_changes_are_refused(self):
        for edit in ('unittest.suite._isnotsuite=lambda *a:False',
                     'unittest.TextTestRunner.run=lambda *a:None',
                     'unittest.case._Outcome.testPartExecutor=lambda *a:None'):
            with self.subTest(edit=edit):
                result, status, text, log = self.direct_fixture('class Case(unittest.TestCase):\n def tearDown(self): '+edit+'\n def test_body(self): self.fail("body")\n')
                self.assertEqual(result, (False, 0, 0), log.read_text())
                self.assertEqual(status, 2)

    def test_suppressed_fixture_exception_without_callback_is_refused(self):
        source = ('from contextlib import contextmanager\nclass Case(unittest.TestCase):\n'
                  ' def setUp(self):\n  original=self._outcome.testPartExecutor\n'
                  '  @contextmanager\n  def forwarding(test_case,isTest=False):\n'
                  '   if isTest:\n    with original(test_case,isTest): yield\n'
                  '   else:\n    try: yield\n    except Exception: pass\n'
                  '  self._outcome.testPartExecutor=forwarding\n'
                  ' def tearDown(self): raise OSError("suppressed fixture")\n'
                  ' def test_body(self): self.fail("body")\n')
        result, status, text, log = self.direct_fixture(source)
        self.assertEqual(result, (False, 0, 0), log.read_text())
        self.assertEqual(status, 2)
        self.assertTrue(any('unmatched captured exception' in reason for reason in __import__('json').loads(text)['refusals']))

    def test_suppressed_subtest_exception_without_callback_is_refused(self):
        # The subtest's failure is kept from the result's `addSubTest`, which
        # every Python's unittest calls with it: 3.10 from `_Outcome.errors`,
        # 3.11 and later from the part executor (they have no `errors`, so a
        # filter of it, as this fixture was until train-11, raised in setUp on
        # CI's Python 3.12 and was refused for that instead).
        source = ('class Case(unittest.TestCase):\n def setUp(self):\n  result=self._outcome.result\n'
                  '  add=result.addSubTest\n'
                  '  def suppressed(test,subtest,err):\n   if err is None: add(test,subtest,err)\n'
                  '  result.addSubTest=suppressed\n'
                  ' def test_body(self):\n  with self.subTest(): self.fail("suppressed subtest")\n  self.fail("body")\n')
        result, status, text, log = self.direct_fixture(source)
        self.assertEqual(result, (False, 0, 0), log.read_text())
        self.assertEqual(status, 2)
        self.assertTrue(any('unmatched captured exception' in reason for reason in __import__('json').loads(text)['refusals']))

    def test_stock_runner_outcome_and_genuine_bound_aliases_are_supported(self):
        source = ('unittest.TextTestRunner=unittest.runner.TextTestRunner\n'
                  'class Case(unittest.TestCase):\n def setUp(self):\n'
                  '  self._outcome.testPartExecutor=unittest.case._Outcome.testPartExecutor.__get__(self._outcome,unittest.case._Outcome)\n'
                  ' def test_body(self):\n  with self.subTest(): self.assertEqual(2+2,4)\n')
        result, status, text, log = self.direct_fixture(source)
        self.assertEqual(result, (True, 1, 0), log.read_text())
        self.assertEqual(status, 0)

    def test_native_setup_oserror_cannot_claim_required_body(self):
        result, status, text, log = self.direct_fixture('class Case(unittest.TestCase):\n def setUp(self): raise OSError("fixture")\n def test_body(self): self.fail("required body")\n')
        self.assertEqual(result, (False, 0, 0), log.read_text())
        self.assertEqual(status, 2)
        self.assertEqual(__import__('json').loads(text)['bodies'][0]['entered'], 0)

    def test_native_setup_assertion_cannot_claim_required_body(self):
        result, status, text, log = self.direct_fixture('class Case(unittest.TestCase):\n def setUp(self): self.fail("fixture assertion")\n def test_body(self): self.fail("required body")\n')
        self.assertEqual(result, (False, 0, 0), log.read_text())
        self.assertEqual(status, 2)
        self.assertEqual(__import__('json').loads(text)['bodies'][0]['entered'], 0)

if __name__ == '__main__':
    unittest.main(verbosity=2)

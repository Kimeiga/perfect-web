"""The bounded-statement campaign requires real Cargo verdicts and restores source."""
import contextlib
import importlib.util
import io
from pathlib import Path
import sys
import tempfile
import types
import unittest
from unittest.mock import patch

SCRIPTS=Path(__file__).resolve().parent.parent
sys.path.insert(0,str(SCRIPTS))
spec=importlib.util.spec_from_file_location('bounded_statement_results_subject',SCRIPTS/'bounded_statements_mutations.py')
subject=importlib.util.module_from_spec(spec)
spec.loader.exec_module(subject)
OK='test result: ok. 1 passed; 0 failed; 0 ignored\n'
FAIL='test witness ... FAILED\ntest result: FAILED. 0 passed; 1 failed; 0 ignored\n'

class BoundedStatementResults(unittest.TestCase):
    def test_only_matching_native_cargo_statuses_are_valid(self):
        for status,text in [(None,''),(-9,FAIL),(137,FAIL),(1,FAIL),(0,FAIL),
                            (101,OK),(101,'error[E0308]: wrong type\n'+FAIL),
                            (101,'unrecognized test result\n'),(0,'')]:
            with self.subTest(status=status,text=text),patch.object(subject,'bounded',return_value=(text,status)):
                self.assertEqual(subject.cargo_tests('postgres'),(False,0,0))

    def test_valid_positive_and_named_assertion_results_remain_distinct(self):
        for status,text,expected in [(0,OK,(True,1,0)),(101,FAIL,(True,0,1))]:
            with self.subTest(status=status),patch.object(subject,'bounded',return_value=(text,status)):
                self.assertEqual(subject.cargo_tests('postgres'),expected)

    def campaign(self,mutant_result,stopped=False,primary=None):
        with tempfile.TemporaryDirectory(prefix='pw-bounded-statement-results-') as temporary:
            path=Path(temporary)/'private-source.rs'
            original=b'one original anchor\n'
            path.write_bytes(original)
            watch=types.SimpleNamespace(stopped=[])
            baseline=types.SimpleNamespace(mutation_bound=types.SimpleNamespace(start=lambda:watch),explain=lambda:None)
            calls=[]
            def bounded(*args):
                calls.append(args)
                if len(calls)==1:return OK,0
                self.assertNotEqual(path.read_bytes(),original)
                if stopped:watch.stopped.append('synthetic stop flag only')
                if primary is not None:raise primary
                return mutant_result
            controls=[('first private witness','postgres',path,'original','changed'),
                      ('must not run after refusal','postgres',path,'original','different')]
            output=io.StringIO()
            with patch.dict(sys.modules,{'mutation_baseline':baseline}), \
                 patch.dict(subject.os.environ,{'PW_STORE_DATABASE_URL':'private fixture only'}), \
                 patch.object(subject,'MUTANTS',controls), \
                 patch.object(subject,'bounded',side_effect=bounded), \
                 patch.object(subject.signal,'signal',return_value=None), \
                 patch.object(subject.subprocess,'Popen',side_effect=AssertionError('real child forbidden')), \
                 contextlib.redirect_stdout(output):
                try:
                    status=subject.main()
                except BaseException as error:
                    self.assertEqual(path.read_bytes(),original)
                    raise
            self.assertEqual(path.read_bytes(),original)
            return status,len(calls),output.getvalue()

    def test_invalid_mutation_refuses_without_a_kill_or_next_control(self):
        for result in [('',None),(FAIL,-9),(FAIL,137),(FAIL,1),(OK,101),
                       ('error[E0308]: wrong type\n'+FAIL,101)]:
            with self.subTest(result=result):
                status,calls,output=self.campaign(result)
                self.assertEqual(status,1)
                self.assertEqual(calls,2)
                self.assertIn('REFUSED',output)
                self.assertNotIn('KILLED',output)

    def test_named_assertions_earn_kills_and_restore_both_controls(self):
        status,calls,output=self.campaign((FAIL,101))
        self.assertEqual((status,calls),(0,3))
        self.assertEqual(output.count(': KILLED'),2)

    def test_memory_stop_flag_refuses_an_apparently_valid_assertion(self):
        status,calls,output=self.campaign((FAIL,101),stopped=True)
        self.assertEqual((status,calls),(1,2))
        self.assertNotIn('KILLED',output)

    def test_original_exception_propagates_with_source_restored(self):
        primary=RuntimeError('private producer exception')
        with self.assertRaises(RuntimeError) as found:
            self.campaign((FAIL,101),primary=primary)
        self.assertIs(found.exception,primary)

if __name__=='__main__':
    unittest.main()

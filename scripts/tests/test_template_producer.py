"""Template controls require complete native runs and test-file assertions."""
import contextlib
import io
from pathlib import Path
import sys
import tempfile
import types
import unittest
from unittest import mock

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import template_mutations as campaign


class TemplateProducer(unittest.TestCase):
    def setUp(self):
        campaign.INVENTORIES.clear()

    def output(self, failed=False, line=None):
        output = f'test witness ... {"FAILED" if failed else "ok"}\n'
        if failed:
            source = campaign.ROOT / 'compiler/pw-core/tests/template_blocks.rs'
            if line is None:
                line = next(i for i, text in enumerate(source.read_text().splitlines(), 1)
                            if text.strip().startswith('assert_eq!'))
            output += (f"\nfailures:\n\n---- witness stdout ----\n"
                       f"thread 'witness' panicked at compiler/pw-core/tests/template_blocks.rs:{line}:5:\n"
                       "assertion `left == right` failed\n\nfailures:\n    witness\n")
        output += (f'test result: {"FAILED" if failed else "ok"}. {int(not failed)} passed; '
                   f'{int(failed)} failed; 0 ignored; 0 measured; 0 filtered out\n')
        return output

    def test_native_green_and_assertion_results(self):
        self.assertEqual(campaign.result_of(0, 0, self.output()), (True, 1, 0))
        self.assertEqual(campaign.result_of(0, 101, self.output(True)), (True, 0, 1))

    def test_native_disagreement_and_build_failures_are_refused(self):
        for status in (None, -9, 1, 0, True):
            self.assertFalse(campaign.result_of(0, status, self.output(True))[0])
        self.assertFalse(campaign.result_of(0, 101, self.output(True) + 'error[E123]: cannot compile')[0])

    def test_changed_missing_and_skipped_inventory_is_refused(self):
        self.assertTrue(campaign.result_of(0, 0, self.output())[0])
        self.assertFalse(campaign.result_of(0, 0, self.output().replace('witness', 'other'))[0])
        self.assertFalse(campaign.result_of(0, 0, self.output().split('\n', 1)[1])[0])
        self.assertFalse(campaign.result_of(0, 0, self.output().replace('0 ignored', '1 ignored'))[0])

    def test_setup_or_foreign_suite_panic_is_refused(self):
        self.assertFalse(campaign.result_of(0, 101, self.output(True, line=1))[0])
        self.assertFalse(campaign.result_of(1, 101, self.output(True))[0])
        self.assertFalse(campaign.result_of(0, 101, self.output(True).replace(':5:', ':27:'))[0])

    def test_invalid_mutant_earns_no_kill_and_source_is_restored(self):
        import mutation_baseline
        with tempfile.TemporaryDirectory() as directory:
            source = Path(directory) / 'source'; source.write_text('original')
            output = io.StringIO()
            with mock.patch.object(campaign, 'run_tests', side_effect=[(True, 1, 0), (False, 0, 0)]), \
                 mock.patch.object(campaign, 'MUTANTS', [('control', source, 'original', 'mutant')]), \
                 mock.patch.object(campaign.signal, 'signal'), \
                 mock.patch.object(mutation_baseline.mutation_bound, 'start', return_value=types.SimpleNamespace(stopped=False)), \
                 contextlib.redirect_stdout(output):
                self.assertEqual(campaign.main(), 1)
            self.assertEqual(source.read_text(), 'original')
            self.assertIn('0 of 1 mutants killed; 1 refused', output.getvalue())
            self.assertNotIn('KILLED', output.getvalue())

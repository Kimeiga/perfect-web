"""A case/delivery control earns credit only from the full native test run."""
import contextlib
import io
from pathlib import Path
import sys
import tempfile
import types
import unittest
from unittest import mock

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import case_and_delivery_mutations as campaign


class CaseDeliveryProducer(unittest.TestCase):
    def output(self, failed=False, line=None):
        names = sorted(campaign.NAMES)
        output = ''.join(f'test {name} ... {"FAILED" if failed and i == 0 else "ok"}\n'
                         for i, name in enumerate(names))
        if failed:
            source = campaign.ROOT / 'compiler/pw-core/tests/case_and_delivery.rs'
            if line is None:
                line = next(i for i, text in enumerate(source.read_text().splitlines(), 1)
                            if text.strip().startswith('assert_eq!'))
            output += (f"\nfailures:\n\n---- {names[0]} stdout ----\n"
                       f"thread '{names[0]}' panicked at compiler/pw-core/tests/case_and_delivery.rs:{line}:5:\n"
                       f"assertion `left == right` failed\n\nfailures:\n    {names[0]}\n")
        output += (f'test result: {"FAILED" if failed else "ok"}. {len(names) - int(failed)} passed; '
                   f'{int(failed)} failed; 0 ignored; 0 measured; 0 filtered out\n')
        return output

    def test_native_green_and_assertion_results(self):
        self.assertEqual(campaign.result_of(0, self.output()), (True, 6, 0))
        self.assertEqual(campaign.result_of(101, self.output(True)), (True, 5, 1))

    def test_build_transport_and_native_exit_disagreement_are_refused(self):
        for status in (None, -9, 1, 0, True):
            self.assertFalse(campaign.result_of(status, self.output(True))[0])
        self.assertFalse(campaign.result_of(101, self.output(True) + 'error[E123]: cannot compile')[0])

    def test_missing_renamed_and_skipped_tests_are_refused(self):
        output = self.output()
        self.assertFalse(campaign.result_of(0, output.split('\n', 1)[1])[0])
        self.assertFalse(campaign.result_of(0, output.replace(sorted(campaign.NAMES)[0], 'foreign'))[0])
        self.assertFalse(campaign.result_of(0, output.replace('0 ignored', '1 ignored'))[0])

    def test_setup_panic_is_refused(self):
        self.assertFalse(campaign.result_of(101, self.output(True, line=1))[0])
        # An unwrap inside the assertion's operand shares its source line,
        # but its native panic points at the operand rather than the macro.
        self.assertFalse(campaign.result_of(101, self.output(True).replace(':5:', ':27:'))[0])

    def test_invalid_mutant_earns_no_kill_and_source_is_restored(self):
        import mutation_baseline
        with tempfile.TemporaryDirectory() as directory:
            source = Path(directory) / 'source'; source.write_text('original')
            output = io.StringIO()
            with mock.patch.object(campaign, 'run_tests', side_effect=[(True, 6, 0), (False, 0, 0)]), \
                 mock.patch.object(campaign, 'MUTANTS', [('control', source, 'original', 'mutant')]), \
                 mock.patch.object(campaign.signal, 'signal'), \
                 mock.patch.object(mutation_baseline.mutation_bound, 'start', return_value=types.SimpleNamespace(stopped=False)), \
                 contextlib.redirect_stdout(output):
                self.assertEqual(campaign.main(), 1)
            self.assertEqual(source.read_text(), 'original')
            self.assertIn('0 of 1 mutants killed; 1 refused', output.getvalue())
            self.assertNotIn('KILLED', output.getvalue())

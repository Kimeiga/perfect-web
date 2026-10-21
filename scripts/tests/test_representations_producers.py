"""The representation campaign uses shared native Cargo assertion authority."""
import contextlib
import io
from pathlib import Path
import subprocess
import sys
import tempfile
import types
import unittest
from unittest import mock

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / "scripts"))
import template_representations_mutations as campaign


class Producers(unittest.TestCase):
    def output(self, suite="core", failed=False):
        names = sorted(campaign.NAMES[suite])
        first = names[0]
        output = ''.join(f'test {name} ... {"FAILED" if failed and name == first else "ok"}\n' for name in names)
        if failed:
            source = campaign.SOURCES[suite]
            lines = source.read_text().splitlines()
            number = next(i for i, line in enumerate(lines, 1) if line.lstrip().startswith('assert!('))
            column = len(lines[number - 1]) - len(lines[number - 1].lstrip()) + 1
            output += (f"\nfailures:\n\n---- {first} stdout ----\nthread '{first}' panicked at "
                       f"{source.relative_to(ROOT)}:{number}:{column}:\nassertion failed\n\nfailures:\n    {first}\n")
        return output + f'test result: {"FAILED" if failed else "ok"}. {len(names)-int(failed)} passed; {int(failed)} failed; 0 ignored; 0 measured; 0 filtered out\n'

    def test_recipe_names_each_evidence_owner_once(self):
        import ci_plan
        import evidence_counts
        body = ci_plan.bodies(ROOT)['e14-representations']
        self.assertEqual(evidence_counts.executed_scripts(body), [
            'representations_producer_mutations.py', 'template_representations_mutations.py',
        ])
        for name in ('representations_producer_mutations.py', 'template_representations_mutations.py'):
            self.assertEqual(body.count('python3 scripts/' + name + ';'), 1)

    def test_complete_results_and_assertions_are_valid(self):
        self.assertEqual(campaign.cargo_result('core', 0, self.output()), (True, 4, 0))
        self.assertEqual(campaign.cargo_result('core', 101, self.output(failed=True)), (True, 3, 1))
        self.assertEqual(campaign.cargo_result('host', 0, self.output('host')), (True, 1, 0))

    def test_bad_native_status_build_or_panic_is_invalid(self):
        output = self.output(failed=True)
        for code in (None, -9, 1, 0, True):
            self.assertFalse(campaign.cargo_result('core', code, output)[0])
        self.assertFalse(campaign.cargo_result('core', 101, output + 'error[E123]: bad build')[0])
        self.assertFalse(campaign.cargo_result('core', 101, output.replace('compiler/pw-core/tests/template_representations.rs:', '/tmp/foreign.rs:'))[0])

    def test_exact_inventory_is_required(self):
        output = self.output()
        for changed in (output.replace(campaign.NAMES['core'][0], 'foreign'), output.replace('0 ignored', '1 ignored')):
            self.assertFalse(campaign.cargo_result('core', 0, changed)[0])

    def test_partial_command_set_is_invalid(self):
        with mock.patch.object(campaign, 'bounded', side_effect=[(self.output(), 0), ('build error', 1)]), contextlib.redirect_stdout(io.StringIO()):
            self.assertEqual(campaign.cargo_tests(), (False, 0, 0))

    def test_timeout_keeps_invalid_native_status(self):
        with mock.patch.object(campaign.producer_process, 'run', side_effect=subprocess.TimeoutExpired(['native'], 1, output='partial')) as child:
            self.assertEqual(campaign.bounded(['native'], cwd=ROOT), ('partial', None))
            self.assertEqual(child.call_args.kwargs['timeout'], campaign.BOUND)

    def test_restoration_build_errors_are_refused(self):
        for status in (1, None, -9):
            with mock.patch.object(campaign, 'bounded', return_value=('failure', status)), contextlib.redirect_stdout(io.StringIO()):
                self.assertFalse(campaign.build())

    def run_campaign(self, result=(True, 0, 1), *, stopped=False, restored=True, cancel=False, baseline=(True, 5, 0)):
        import mutation_baseline
        with tempfile.TemporaryDirectory() as tmp:
            source = Path(tmp) / 'owned'; source.write_text('original')
            watch = types.SimpleNamespace(stopped=False)
            output = io.StringIO()
            calls = 0
            def suite():
                nonlocal calls
                calls += 1
                if calls == 1: return baseline
                watch.stopped = stopped
                if cancel: raise SystemExit(37)
                return result
            with mock.patch.object(campaign, 'MUTANTS', [('fault', 'cargo', source, 'original', 'mutant')]), \
                 mock.patch.object(campaign, 'SUITES', {'cargo': suite}), \
                 mock.patch.object(campaign, 'build', return_value=restored), \
                 mock.patch.object(campaign.signal, 'signal'), \
                 mock.patch.object(mutation_baseline.mutation_bound, 'start', return_value=watch), \
                 mock.patch.object(mutation_baseline, 'explain'), contextlib.redirect_stdout(output):
                try:
                    if cancel:
                        with self.assertRaises(SystemExit) as error: campaign.main()
                        self.assertEqual(error.exception.code, 37)
                        code = 1
                    else: code = campaign.main()
                finally:
                    self.assertEqual(source.read_text(), 'original')
            if stopped or not result[0]:
                self.assertNotIn('KILLED', output.getvalue())
            return code

    def test_invalid_or_stopped_results_earn_no_kill(self):
        self.assertEqual(self.run_campaign(), 0)
        self.assertEqual(self.run_campaign((False, 0, 1)), 1)
        self.assertEqual(self.run_campaign(stopped=True), 1)
        self.assertEqual(self.run_campaign((True, 5, 0)), 1)

    def test_baseline_must_be_complete_and_green(self):
        for baseline in ((False, 5, 0), (True, 4, 1), (True, 0, 0)):
            self.assertEqual(self.run_campaign(baseline=baseline), 1)

    def test_failed_restored_build_fails_campaign(self):
        self.assertEqual(self.run_campaign(restored=False), 1)

    def test_cancellation_restores_source_and_identity(self):
        self.assertEqual(self.run_campaign(cancel=True), 1)


if __name__ == '__main__':
    unittest.main()
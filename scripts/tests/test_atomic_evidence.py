"""A self-checking recipe reads complete evidence and publishes on success."""
from pathlib import Path
import importlib.util
import os
import shutil
import subprocess
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[2]
PREVIOUS = 'previous complete report\n'
PROGRAM = r'''#!/usr/bin/env bash
set -eu
case "$(basename "$0")" in
  python3)
    if [ "$1" = --version ]; then echo 'Python stand-in'; exit 0; fi
    case "$*" in
      *test_evidence_counts.py*|scripts/evidence_counts.py)
        # The evidence reader may see a complete previous report or none.
        # A report currently being written is neither.
        if [ -e docs/evidence/E14/evidence-integrity.txt ] &&
           ! cmp -s docs/evidence/E14/evidence-integrity.txt previous.txt; then
          echo 'FAIL: the gate observed a partially written report' >&2
          exit 9
        fi ;;
    esac
    if [ "${ATOMIC_FAIL:-}" = controls ] && [ "$1" = scripts/evidence_integrity_mutations.py ]; then
      # Real mutation verdicts go to the report, so the trap must reveal them.
      echo 'FAIL: a mutation survived'
      exit 8
    fi
    echo 'test_stand_in ... ok'
    echo 'Ran 1 test'
    echo 'OK'
    echo '3 of 3 mutants killed'
    ;;
  cargo) echo 'test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out'; echo 'check-latency cold 1ms' ;;
  nproc) echo 1 ;;
  sleep|timeout) exit 0 ;;
  rustc|node) echo 'stand-in version' ;;
esac
'''


class AtomicEvidence(unittest.TestCase):
    def test_the_published_report_remains_a_planned_evidence_output(self):
        spec=importlib.util.spec_from_file_location('atomic_ci_plan',ROOT/'scripts/ci_plan.py')
        plan=importlib.util.module_from_spec(spec)
        spec.loader.exec_module(plan)
        final='docs/evidence/E14/evidence-integrity.txt'
        actual=(ROOT/'just/evidence-integrity.just').read_text()
        self.assertEqual(plan.evidence_files(actual),[final])
        # Ordinary writes remain recognized and duplicate paths stay unique.
        self.assertEqual(plan.evidence_files('} > '+final+'\nmv "$report" '+final+'\n'),[final])
        self.assertEqual(plan.evidence_files('mv "$report" unrelated.txt\n'),[])

    def run_recipe(self, previous=True, fail=False):
        with tempfile.TemporaryDirectory(prefix='pw-atomic-evidence-') as directory:
            root=Path(directory)
            (root/'just').mkdir()
            shutil.copy(ROOT/'just/evidence-integrity.just',root/'just/evidence-integrity.just')
            (root/'justfile').write_text('set shell := ["bash", "-euo", "pipefail", "-c"]\nimport "just/evidence-integrity.just"\n')
            (root/'bin').mkdir()
            for name in ['python3','cargo','nproc','sleep','timeout','rustc','node']:
                path=root/'bin'/name
                path.write_text(PROGRAM)
                path.chmod(0o755)
            (root/'tmp').mkdir()
            (root/'previous.txt').write_text(PREVIOUS)
            git=['git','-c','user.name=test','-c','user.email=test@test','-c','commit.gpgsign=false']
            subprocess.run(git+['init','-q'],cwd=root,check=True)
            subprocess.run(git+['add','.'],cwd=root,check=True)
            subprocess.run(git+['commit','-qm','fixture'],cwd=root,check=True)
            final=root/'docs/evidence/E14/evidence-integrity.txt'
            if previous:
                final.parent.mkdir(parents=True)
                final.write_text(PREVIOUS)
            env=dict(os.environ,PATH=str(root/'bin')+os.pathsep+os.environ['PATH'],TMPDIR=str(root/'tmp'))
            if fail: env['ATOMIC_FAIL']='controls'
            result=subprocess.run(['just','e14-evidence-integrity'],cwd=root,env=env,capture_output=True,text=True,timeout=30)
            return result.returncode,result.stdout+result.stderr,final.read_text() if final.exists() else None,list((root/'tmp').iterdir()) + [p for p in final.parent.iterdir() if p != final]

    def test_a_complete_report_stays_visible_until_success(self):
        status,said,report,left=self.run_recipe()
        self.assertEqual(status,0,said)
        self.assertIn('== mutation controls',report)
        self.assertIn('3 of 3 mutants killed',report)
        self.assertNotEqual(report,PREVIOUS)
        self.assertEqual(left,[])

    def test_a_first_run_publishes_only_the_complete_report(self):
        status,said,report,left=self.run_recipe(previous=False)
        self.assertEqual(status,0,said)
        self.assertIn('3 of 3 mutants killed',report)
        self.assertEqual(left,[])

    def test_a_failed_run_keeps_the_previous_complete_report(self):
        status,said,report,left=self.run_recipe(fail=True)
        self.assertNotEqual(status,0,said)
        self.assertEqual(report,PREVIOUS)
        self.assertIn('FAIL: a mutation survived',said)
        self.assertEqual(left,[])


if __name__=='__main__': unittest.main(verbosity=2)

"""The each-head campaign refuses invalid Cargo producers and restores source."""
import contextlib
import importlib.util
import io
from pathlib import Path
import subprocess
import sys
import tempfile
import types
import unittest
from unittest.mock import patch

SCRIPTS = Path(__file__).resolve().parent.parent
sys.path.insert(0, str(SCRIPTS))
spec = importlib.util.spec_from_file_location("each_results_subject", SCRIPTS / "each_head_effects_mutations.py")
each = importlib.util.module_from_spec(spec)
spec.loader.exec_module(each)
OK = "test result: ok. 1 passed; 0 failed; 0 ignored\n"
FAIL = "test witness ... FAILED\ntest result: FAILED. 0 passed; 1 failed; 0 ignored\n"

class EachResults(unittest.TestCase):
    def campaign(self, status, output):
        with tempfile.TemporaryDirectory(prefix="pw-each-results-") as temporary:
            path = Path(temporary)/"source.rs"
            path.write_text("original\n")
            results = [
                subprocess.CompletedProcess(["cargo","test"],0,OK,""),
                subprocess.CompletedProcess(["cargo","test"],status,output,""),
            ]
            text = io.StringIO()
            with patch.object(each,"MUTANTS",[("one source control",path,"original\n","changed\n")]), patch.object(
                each.subprocess,"run",side_effect=results
            ), patch.dict(sys.modules,{"mutation_baseline":types.SimpleNamespace()}), contextlib.redirect_stdout(text):
                code = each.main()
            self.assertEqual(path.read_text(),"original\n")
            return code,text.getvalue()

    def test_invalid_outcome_cannot_earn_a_kill(self):
        for status, output in [(-9,FAIL),(137,FAIL),(1,FAIL),(101,"error[E0308]: cannot compile\n")]:
            with self.subTest(status=status):
                code,text = self.campaign(status,output)
                self.assertEqual(code,1,text)
                self.assertIn("REFUSED",text)
                self.assertNotIn("KILLED",text)

    def test_named_test_failure_earns_a_kill_and_restores(self):
        code,text = self.campaign(101,FAIL)
        self.assertEqual(code,0,text)
        self.assertIn("KILLED",text)

if __name__ == "__main__":
    unittest.main()

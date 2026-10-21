"""Cancellation unwinds actual runner mains in isolated bounded children."""
import json
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest

SCRIPTS = Path(__file__).resolve().parents[1]
ROOT = SCRIPTS.parent
CHILD = SCRIPTS / "tests/mutation_cancel_child.py"
sys.path.insert(0, str(SCRIPTS))


class MutationCancellation(unittest.TestCase):
    def run_cancel_probe(self, runner, source):
        original = source.read_bytes()
        with tempfile.TemporaryDirectory(prefix="mutation-cancel-") as directory:
            target = Path(directory) / ("mutant" + source.suffix)
            target.write_bytes(original)
            child = subprocess.run(
                [sys.executable, str(CHILD), runner, str(target)],
                cwd=ROOT, capture_output=True, text=True, timeout=8, check=False,
            )
            source_restored = target.exists() and target.read_bytes() == original
            self.assertTrue(
                child.returncode == 128 + 15 and source_restored,
                f"exit={child.returncode}, source_restored={source_restored}\n"
                + child.stdout + child.stderr,
            )
            receipts = [line for line in child.stdout.splitlines() if line.startswith("CANCEL_RECEIPT ")]
            self.assertEqual(len(receipts), 1, child.stdout + child.stderr)
            receipt = json.loads(receipts[0].removeprefix("CANCEL_RECEIPT "))
            self.assertTrue(receipt["source_restored"], child.stdout)
            self.assertTrue(receipt["handler_restored"], child.stdout)
            self.assertEqual(target.read_bytes(), original)
        # subprocess.run has reaped the bounded child before this check returns.

    def test_playwright_main_restores_mutant_on_sigterm(self):
        from playwright_assertion_mutations import MUTANTS
        self.run_cancel_probe("playwright_assertion_mutations", MUTANTS[0][2])

    def test_todomvc_main_restores_mutant_on_sigterm(self):
        from todomvc_compat_mutations import SOURCE
        self.run_cancel_probe("todomvc_compat_mutations", SOURCE)


if __name__ == "__main__":
    unittest.main()

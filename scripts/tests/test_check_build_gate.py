#!/usr/bin/env python3
"""Exercise the real test-compile recipe with failing corpus producers."""
import os
import pathlib
import subprocess
import tempfile
import unittest

ROOT = pathlib.Path(__file__).resolve().parents[2]
class CorpusGate(unittest.TestCase):
    def assert_failure_propagates(self, target):
        with tempfile.TemporaryDirectory(prefix="pw-corpus-gate-") as directory:
            scratch = pathlib.Path(directory)
            cargo = scratch / "cargo"
            cargo.write_text("#!/usr/bin/env python3\nimport os,sys\nwith open(os.environ['TRACE'], 'a') as f: f.write(' '.join(sys.argv[1:]) + '\\n')\nsys.exit(67 if os.environ['FAIL_TARGET'] in ' '.join(sys.argv[1:]) else 0)\n")
            cargo.chmod(0o755)
            trace = scratch / "calls"
            env = dict(os.environ, TRACE=str(trace), FAIL_TARGET=target)
            result = subprocess.run(["just", "--justfile", str(ROOT / "justfile"), "--set", "toolchain_bin", str(scratch), "test-compile"], cwd=ROOT, env=env, capture_output=True, text=True, timeout=30)
            calls = trace.read_text() if trace.exists() else ""
            self.assertIn(target, calls, result.stdout + result.stderr)
            self.assertNotEqual(result.returncode, 0, "a failed corpus producer was swallowed")
            self.assertIn("exit code 67", result.stderr)
    def test_semantic_language_gate_runs_and_propagates_its_failure(self):
        self.assert_failure_propagates("--test checking_source no_accepted_corpus_file_reports_a_body_level_error")
    def test_complete_support_gate_runs_and_propagates_its_failure(self):
        self.assert_failure_propagates("--test check_build_corpus accepted_corpus_has_exactly_its_known_backend_refusals")

if __name__ == "__main__":
    unittest.main()

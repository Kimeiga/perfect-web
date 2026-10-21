"""Native Rust distinguishes an assertion from a panic evaluating its operand."""
import contextlib
import io
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest
from unittest import mock

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / 'scripts'))
import qualified_device_mutations as controls


class AssertionProvenance(unittest.TestCase):
    def test_native_operand_unwrap_earns_no_assertion_kill(self):
        with tempfile.TemporaryDirectory() as tmp:
            source = Path(tmp) / 'assertions.rs'
            binary = Path(tmp) / 'assertions'
            source.write_text('''#[test]
fn actual_assertion() {
    assert_eq!(1, 2);
}
#[test]
fn operand_unwrap() {
    assert_eq!(None::<u8>.unwrap(), 1);
}
''')
            built = subprocess.run(['rustc', '--test', str(source), '-o', str(binary)],
                                   capture_output=True, text=True, timeout=30)
            self.assertEqual(built.returncode, 0, built.stdout + built.stderr)
            for case, valid in [('actual_assertion', True), ('operand_unwrap', False)]:
                native = subprocess.run([str(binary), '--exact', case], capture_output=True,
                                        text=True, timeout=10)
                self.assertEqual(native.returncode, 101, native.stdout + native.stderr)
                with mock.patch.object(controls, 'TEST', source), mock.patch.object(controls, 'INVENTORY', None), contextlib.redirect_stdout(io.StringIO()):
                    self.assertEqual(controls.validate(native.returncode, native.stdout + native.stderr)[0], valid, native.stdout)


if __name__ == '__main__':
    unittest.main()
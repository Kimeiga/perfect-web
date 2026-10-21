"""Native assertion failures cannot be replaced by setup/build/exit faults."""
import pathlib
import sys
import tempfile
import unittest

ROOT = pathlib.Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / 'scripts'))
import cargo_assertion_results
import producer_process


class CargoAssertions(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.directory = tempfile.TemporaryDirectory(prefix='pw-cargo-assertions-')
        cls.path = pathlib.Path(cls.directory.name)
        cls.source = cls.path / 'native.rs'
        cls.source.write_text('''#[test]
fn green() { assert_eq!(1, 1); }
#[test]
fn comparison() {
    assert_eq!(1, 2, "custom diagnostic");
}
#[test]
fn runtime() {
    panic!("not a comparison");
}
#[test]
fn same_line_operand() {
    assert_eq!(None::<u8>.unwrap(), 2);
}
#[test]
fn message_operand() {
    assert_eq!(1, 2, "{}", None::<u8>.unwrap());
}
#[test]
fn setup_before_assertion() {
    let _ = None::<u8>.unwrap(); assert_eq!(1, 2);
}
''')
        cls.binary = cls.path / 'native'
        build = producer_process.run(['rustc', '--test', '--edition=2024', str(cls.source), '-o', str(cls.binary)], timeout=30)
        if build.returncode:
            raise RuntimeError(build.stdout)

    @classmethod
    def tearDownClass(cls):
        cls.directory.cleanup()

    def native(self, name):
        return producer_process.run([str(self.binary), '--exact', name, '--format=pretty'], timeout=10)

    def parsed(self, native, expected, status=None, output=None, sources=None):
        return cargo_assertion_results.validate(
            native.returncode if status is None else status,
            native.stdout if output is None else output,
            root=ROOT, sources=[self.source] if sources is None else sources, expected=[expected])

    def test_native_green_and_custom_comparison_have_exact_inventory(self):
        for name, failures in [('green', 0), ('comparison', 1)]:
            native = self.native(name)
            result = self.parsed(native, name)
            self.assertTrue(result.valid, native.stdout)
            self.assertEqual(result.failed, failures)

    def test_native_runtime_and_same_line_operand_panics_are_refused(self):
        for name in ('runtime', 'same_line_operand', 'message_operand', 'setup_before_assertion'):
            native = self.native(name)
            self.assertEqual(native.returncode, 101, native.stdout)
            self.assertFalse(self.parsed(native, name).valid, native.stdout)

    def test_exit_inventory_and_source_disagreement_are_refused(self):
        native = self.native('comparison')
        self.assertFalse(self.parsed(native, 'comparison', status=0).valid)
        self.assertFalse(self.parsed(native, 'missing').valid)
        self.assertFalse(self.parsed(native, 'comparison', sources=[]).valid)
        self.assertFalse(self.parsed(native, 'comparison', output=native.stdout.split('test result:')[0]).valid)
        self.assertFalse(self.parsed(native, 'comparison', output=native.stdout.replace('0 ignored', '1 ignored')).valid)

    def test_native_build_failure_is_refused(self):
        source = self.path / 'broken.rs'
        source.write_text('this is not valid Rust')
        build = producer_process.run(['rustc', '--test', str(source), '-o', str(self.path / 'broken')], timeout=30)
        self.assertNotEqual(build.returncode, 0)
        self.assertFalse(self.parsed(build, 'comparison').valid)


if __name__ == '__main__':
    unittest.main()

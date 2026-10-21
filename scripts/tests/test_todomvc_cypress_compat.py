"""The explicit compatibility copy preserves originals and refuses all drift."""
import contextlib
import io
import json
from pathlib import Path
import shutil
import sys
import tempfile
import unittest
from unittest import mock

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import todomvc_cypress_compat as compat
import todomvc_cypress_result as result
import todomvc_cypress_vendored as upstream


class Compatibility(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        self.source = self.root / "source"
        self.source.mkdir()
        for name in (*upstream.BLOBS, "package.json"):
            destination = self.source / name
            destination.parent.mkdir(parents=True, exist_ok=True)
            shutil.copyfile(upstream.DIR / name, destination)
        self.destination = self.root / "project"

    def create(self, package="15.21.1", binary="15.21.1"):
        return compat.create(self.destination, package, binary, source=self.source)

    def verify(self):
        return compat.verify(self.destination, "15.21.1", "15.21.1", source=self.source)

    def test_exact_two_edits_preserve_all_originals_and_29_test_contracts(self):
        before = {name: (self.source / name).read_bytes() for name in upstream.BLOBS}
        identity = self.create()
        self.assertEqual(identity, self.verify())
        changed = [name for name, data in before.items() if (self.destination / name).read_bytes() != data]
        self.assertEqual(changed, ["cypress/e2e/spec.cy.js", "cypress/support/commands.js"])
        for name, data in before.items():
            self.assertEqual((self.source / name).read_bytes(), data)
            self.assertEqual(upstream.blob(data), upstream.BLOBS[name])
        spec = "cypress/e2e/spec.cy.js"
        self.assertEqual((self.destination / spec).read_bytes(),
                         before[spec].replace(b".invoke('reset')", b".invoke('resetHistory')"))
        commands = "cypress/support/commands.js"
        insertion = b"    return cy.get(todoListSelector, { log: false }).contains('li', todo.trim(), { log: false })\n"
        generated = (self.destination / commands).read_bytes()
        self.assertEqual(generated.count(insertion), 1)
        self.assertEqual(generated.replace(insertion, b""), before[commands])
        original_tests = result.spec_tests((self.source / spec).read_text())
        staged_tests = result.spec_tests((self.destination / spec).read_text())
        self.assertEqual(len(staged_tests), 29)
        self.assertEqual(staged_tests, original_tests)
        manifest = json.loads((self.destination / compat.MANIFEST).read_text())
        self.assertEqual(manifest["identity"], "todomvc-cypress-compat-v1")
        self.assertEqual(len(manifest["patches"]), 2)
        self.assertEqual(manifest["upstream_blobs"], upstream.BLOBS)
        self.assertEqual(len(manifest["output_sha256"]), 8)

    def test_each_original_drift_or_missing_file_refuses_before_creation(self):
        # The first witness is unpatched: removing its blob check must really
        # accept drift, not merely fail later at a patched-output guard.
        for name in ("LICENSE-todomvc.md", *(name for name in upstream.BLOBS if name != "LICENSE-todomvc.md")):
            path = self.source / name
            original = path.read_bytes()
            path.write_bytes(original + b"\n")
            with self.assertRaisesRegex(ValueError, "upstream blob drift"):
                self.create()
            self.assertFalse(self.destination.exists())
            path.unlink()
            with self.assertRaisesRegex(ValueError, "missing or nonregular"):
                self.create()
            self.assertFalse(self.destination.exists())
            path.write_bytes(original)

    def test_package_and_binary_versions_must_be_exact(self):
        for package, binary in (("15.21.0", "15.21.1"), ("15.21.1", "15.21.0"),
                                ("15.21.1\n", "15.21.1"), ("", "")):
            with self.assertRaisesRegex(ValueError, "not pinned"):
                self.create(package, binary)
            self.assertFalse(self.destination.exists())

    def test_declared_pin_drift_is_refused(self):
        path = self.source / "package.json"
        data = json.loads(path.read_text())
        data["devDependencies"]["cypress"] = "^15.21.1"
        path.write_text(json.dumps(data))
        with self.assertRaisesRegex(ValueError, "declared Cypress"):
            self.create()
        self.assertFalse(self.destination.exists())

    def test_patch_identity_cannot_silently_change_under_v1(self):
        patches = list(compat.PATCHES)
        patches[0] = ("different-patch-name", *patches[0][1:])
        with mock.patch.object(compat, "PATCHES", patches):
            with self.assertRaisesRegex(ValueError, "patch identity drift"):
                self.create()
        self.assertFalse(self.destination.exists())

    def test_ambiguous_anchor_is_refused(self):
        files = compat.inputs(self.source, "15.21.1", "15.21.1")
        files["cypress/e2e/spec.cy.js"] += b"\n.invoke('reset')\n"
        with mock.patch.object(compat, "inputs", return_value=files):
            with self.assertRaisesRegex(ValueError, "anchor is not unique"):
                self.create()
        self.assertFalse(self.destination.exists())

    def test_output_identity_cannot_silently_change_under_v1(self):
        with mock.patch.object(compat, "PATCHED_SHA256", {"cypress/e2e/spec.cy.js": "0" * 64}):
            with self.assertRaisesRegex(ValueError, "output identity drift"):
                self.create()
        self.assertFalse(self.destination.exists())

    def test_staged_file_and_manifest_drift_are_refused(self):
        self.create()
        for name in (*upstream.BLOBS, "package.json", compat.MANIFEST):
            path = self.destination / name
            original = path.read_bytes()
            path.write_bytes(original + b"\n")
            with self.assertRaisesRegex(ValueError, "staged content drift"):
                self.verify()
            path.write_bytes(original)
        self.verify()

    def test_staged_added_missing_and_symlink_files_are_refused(self):
        self.create()
        extra = self.destination / "cypress/e2e/extra.cy.js"
        extra.write_text("it.skip('hidden', () => {})")
        with self.assertRaisesRegex(ValueError, "inventory drift"):
            self.verify()
        extra.unlink()
        path = self.destination / "LICENSE-todomvc.md"
        data = path.read_bytes()
        path.unlink()
        with self.assertRaisesRegex(ValueError, "inventory drift"):
            self.verify()
        path.symlink_to(self.source / "LICENSE-todomvc.md")
        with self.assertRaisesRegex(ValueError, "symlink"):
            self.verify()
        path.unlink()
        path.write_bytes(data)
        self.verify()

    def test_recheck_catches_source_drift_and_never_overwrites(self):
        self.create()
        before = (self.destination / compat.MANIFEST).read_bytes()
        sentinel = b"user-owned content must survive an attempted second create\n"
        (self.destination / compat.MANIFEST).write_bytes(sentinel)
        with self.assertRaises(FileExistsError):
            self.create()
        self.assertEqual((self.destination / compat.MANIFEST).read_bytes(), sentinel)
        (self.destination / compat.MANIFEST).write_bytes(before)
        path = self.source / "cypress.config.js"
        path.write_bytes(path.read_bytes() + b"\n")
        with self.assertRaisesRegex(ValueError, "upstream blob drift"):
            self.verify()

    def test_cli_refuses_before_staging_and_prints_explicit_identity(self):
        # The CLI's source is fixed to the repository's seven originals.
        args = ["create", str(self.destination), "--package-version", "wrong", "--binary-version", "15.21.1"]
        with contextlib.redirect_stderr(io.StringIO()) as error:
            code = compat.main(args)
        self.assertEqual(code, 1)
        self.assertIn("REFUSED", error.getvalue())
        self.assertFalse(self.destination.exists())
        args[3] = "15.21.1"
        with contextlib.redirect_stdout(io.StringIO()) as output:
            code = compat.main(args)
        self.assertEqual(code, 0)
        self.assertIn("todomvc-cypress-compat-v1: create", output.getvalue())
        self.assertIn("exact 2-patch execution copy; manifest sha256", output.getvalue())


if __name__ == "__main__":
    unittest.main()

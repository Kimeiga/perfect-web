"""`todomvc_cypress_vendored.py`: the official suite is vendored unchanged.

Track `todomvc` runs TodoMVC's own Cypress spec unchanged as its acceptance,
which is a claim about bytes. The check hashes each vendored file as git does
and compares it with upstream's blob; this tests the check: the files in the
tree are upstream's, and a file edited by one byte is not, so a check that
passed anything would fail here.
"""

import importlib.util
import pathlib
import tempfile
import unittest

SCRIPTS = pathlib.Path(__file__).resolve().parent.parent


def load(name: str):
    spec = importlib.util.spec_from_file_location(name, SCRIPTS / f"{name}.py")
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


vendored = load("todomvc_cypress_vendored")


class Vendored(unittest.TestCase):
    def test_a_blob_is_gits(self):
        # `printf 'hello\n' | git hash-object --stdin`, and the empty blob's
        # well-known hash.
        self.assertEqual(
            vendored.blob(b"hello\n"), "ce013625030ba8dba906f756967f9e9ca394464a"
        )
        self.assertEqual(vendored.blob(b""), "e69de29bb2d1d6434b8b29ae775ad8c2e48c5391")

    def test_the_suite_in_the_tree_is_upstreams(self):
        self.assertEqual(vendored.problems(), [])

    def test_a_file_changed_by_a_byte_is_not(self):
        real = vendored.DIR
        with tempfile.TemporaryDirectory() as tmp:
            copy = pathlib.Path(tmp)
            for path in list(vendored.BLOBS) + ["package.json"]:
                target = copy / path
                target.parent.mkdir(parents=True, exist_ok=True)
                target.write_bytes((real / path).read_bytes())
            vendored.DIR = copy
            try:
                self.assertEqual(vendored.problems(), [])
                spec = copy / "cypress/e2e/spec.cy.js"
                spec.write_bytes(spec.read_bytes() + b" ")
                found = vendored.problems()
                self.assertEqual(len(found), 1, found)
                self.assertIn("spec.cy.js", found[0])
                # And a file gone is a problem too, and so is another Cypress.
                spec.unlink()
                self.assertTrue(any("missing" in p for p in vendored.problems()))
                (copy / "package.json").write_text('{"devDependencies": {"cypress": "16.2.0"}}')
                self.assertTrue(any("16.2.0" in p for p in vendored.problems()))
            finally:
                vendored.DIR = real


if __name__ == "__main__":
    unittest.main()

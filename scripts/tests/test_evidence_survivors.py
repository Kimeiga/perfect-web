"""`evidence_survivors.py`: no committed evidence records a mutant that
survived. Read on a tree of its own, with its control, and held of the
repository's own evidence.
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


survivors = load("evidence_survivors")


class Survivors(unittest.TestCase):
    def tree(self, *files: tuple[str, str]) -> pathlib.Path:
        root = pathlib.Path(tempfile.mkdtemp(prefix="pw-survivors-"))
        for rel, text in files:
            path = root / "docs/evidence" / rel
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_text(text)
        return root

    def test_a_survivor_a_short_count_and_a_lost_anchor_are_named(self):
        root = self.tree(
            ("E10/names.txt", "a: KILLED (1 of 2 tests fail)\nb: SURVIVED\n1 of 2 mutants killed\n"),
            ("E14/x.txt", "c: ANCHOR NOT FOUND EXACTLY ONCE in main.rs\n"),
        )
        self.assertEqual(
            survivors.survivors(root),
            [
                ("docs/evidence/E10/names.txt", 2, "b: SURVIVED"),
                ("docs/evidence/E10/names.txt", 3, "1 of 2 mutants killed"),
                ("docs/evidence/E14/x.txt", 1, "c: ANCHOR NOT FOUND EXACTLY ONCE in main.rs"),
            ],
        )

    def test_the_control_every_mutant_killed_names_nothing(self):
        root = self.tree(
            ("E14/y.txt", "a: KILLED (1 of 2 tests fail)\n2 of 2 mutants killed\n"),
            # A test that failed in a baseline recorded as one is no mutant.
            ("E9/baseline.txt", "test x ... FAILED\n"),
        )
        self.assertEqual(survivors.survivors(root), [])

    def test_the_repositorys_evidence_records_no_survivor(self):
        self.assertEqual(survivors.survivors(), [])


if __name__ == "__main__":
    unittest.main()

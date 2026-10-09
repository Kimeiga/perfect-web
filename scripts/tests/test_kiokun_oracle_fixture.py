"""`kiokun_oracle_fixture.py`: an oracle's answers, written as the fixture CI
holds the rewrite to, with what made them."""

import importlib.util
import json
import pathlib
import tempfile
import unittest

SCRIPTS = pathlib.Path(__file__).resolve().parent.parent


def load(name: str):
    spec = importlib.util.spec_from_file_location(name, SCRIPTS / f"{name}.py")
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


fixture = load("kiokun_oracle_fixture")


class Fixture(unittest.TestCase):
    def test_the_fixture_names_what_made_it_and_keeps_the_answers(self) -> None:
        with tempfile.TemporaryDirectory() as d:
            answers = pathlib.Path(d) / "answers.json"
            out = pathlib.Path(d) / "fixture.json"
            oracle = {
                "stride": 1,
                "read": 2,
                "answered": 1,
                "named": {},
                "answers": {"人": {"title": "人 | Kiokun", "description": "d", "named": []}},
            }
            answers.write_text(json.dumps(oracle), encoding="utf-8")
            argv = ["x", str(answers), str(out), "just e14-kiokun-seo", "abc", "def", "v22"]
            self.assertEqual(fixture.main(argv), 0)
            written = json.loads(out.read_text(encoding="utf-8"))
            self.assertEqual(written["produced_by"], "just e14-kiokun-seo")
            self.assertEqual(written["kiokun_commit"], "abc")
            self.assertEqual(written["pleris_commit"], "def")
            self.assertEqual(written["node"], "v22")
            self.assertEqual(written["oracle"], oracle)

    def test_a_short_argument_list_is_refused(self) -> None:
        self.assertEqual(fixture.main(["x", "a"]), 2)


if __name__ == "__main__":
    unittest.main()

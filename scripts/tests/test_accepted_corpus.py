"""scripts/accepted_corpus.py: the accepted corpus passes only as examples/accepted/build-refusals.json lists it."""

import importlib.util
import json
import pathlib
import unittest

SCRIPTS = pathlib.Path(__file__).resolve().parent.parent
spec = importlib.util.spec_from_file_location("accepted_corpus", SCRIPTS / "accepted_corpus.py")
corpus = importlib.util.module_from_spec(spec)
spec.loader.exec_module(corpus)

OUTPUT = """error: [PW6601] a template part the renderer cannot render: one
  --> A-007-x.pw:30:1
   |
warning: [PW0365] a warning
  --> A-052-y.pw:24:5
error: [PW6601] the generated WIT does not parse: two
  --> A-008-z.pw:18:1
"""


class AcceptedCorpus(unittest.TestCase):
    def test_each_error_is_read_with_its_file_code_and_message(self):
        self.assertEqual(
            corpus.errors("[31m" + OUTPUT),
            [("A-007-x.pw", "PW6601", "a template part the renderer cannot render: one"),
             ("A-008-z.pw", "PW6601", "the generated WIT does not parse: two")],
        )

    def test_the_corpus_passes_only_exactly_as_listed(self):
        expected = corpus.errors(OUTPUT)
        self.assertEqual(corpus.compare(expected, expected), ([], []))
        # The controls: an error the list does not name, a listed refusal not reported, and one whose message
        # differs by a character.
        extra = ("A-001-w.pw", "PW0101", "a new error")
        self.assertEqual(corpus.compare(expected, expected + [extra]), ([extra], []))
        self.assertEqual(corpus.compare(expected, expected[:1]), ([], [expected[1]]))
        changed = [(expected[0][0], expected[0][1], expected[0][2] + "."), expected[1]]
        unexpected, missing = corpus.compare(expected, changed)
        self.assertEqual((len(unexpected), len(missing)), (1, 1))

    def test_the_list_names_each_refusal_with_its_message_and_why(self):
        listed = json.loads(corpus.LIST.read_text())["refused"]
        self.assertTrue(listed)
        for entry in listed:
            self.assertEqual(entry["code"], "PW6601")
            for key in ("file", "stage", "responsible", "message", "why"):
                self.assertTrue(entry[key], (entry, key))
            self.assertTrue((corpus.ROOT / "examples/accepted" / entry["file"]).exists(), entry["file"])


if __name__ == "__main__":
    unittest.main()

"""`kiokun_search_sample.py`: CI's search index holds cleared rows only (the
integrator's ruling of 2026-10-10)."""

import importlib.util
import pathlib
import unittest

SCRIPTS = pathlib.Path(__file__).resolve().parent.parent
spec = importlib.util.spec_from_file_location("kiokun_search_sample", SCRIPTS / "kiokun_search_sample.py")
sample = importlib.util.module_from_spec(spec)
spec.loader.exec_module(sample)

HEADER = ["word", "language", "definition", "pronunciation", "reading_search", "is_common"]
CLEARED = {("人", "person"), ("學", "learning")}


class Kept(unittest.TestCase):
    def kept(self, rows):
        return sample.kept_rows(HEADER, rows, CLEARED)

    def test_a_chinese_row_is_kept_only_where_a_cleared_item_gives_its_definition(self) -> None:
        rows = [
            ["人", "chinese", "person", "rén", "ren", "0"],
            ["人", "chinese", "Dong Chinese's own", "rén", "ren", "0"],
        ]
        self.assertEqual(self.kept(rows), [rows[0]])

    def test_a_japanese_row_is_kept(self) -> None:
        rows = [["人", "japanese", "person", "ひと", "hito", "1"]]
        self.assertEqual(self.kept(rows), rows)

    def test_a_korean_rows_ipa_is_left_out(self) -> None:
        rows = [["人間", "korean", "human", "inɡan", "ingan", "0"]]
        self.assertEqual(self.kept(rows), [["人間", "korean", "human", "", "ingan", "0"]])

    def test_a_word_without_the_characters_or_too_long_is_not_kept(self) -> None:
        rows = [
            ["水", "japanese", "water", "みず", "mizu", "1"],
            ["人人人", "japanese", "everyone", "", "", "0"],
        ]
        self.assertEqual(self.kept(rows), [])


if __name__ == "__main__":
    unittest.main()

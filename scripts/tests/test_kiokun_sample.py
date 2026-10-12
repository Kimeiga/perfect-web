"""`kiokun_sample.py`: the repository's sample holds the fields a committed
test reads, from cleared sources only (the integrator's ruling of
2026-10-10)."""

import importlib.util
import json
import pathlib
import unittest
import zlib

SCRIPTS = pathlib.Path(__file__).resolve().parent.parent
spec = importlib.util.spec_from_file_location("kiokun_sample", SCRIPTS / "kiokun_sample.py")
sample = importlib.util.module_from_spec(spec)
spec.loader.exec_module(sample)

ENTRY = {
    "key": "人",
    "chinese_words": [
        {
            "_id": "a",
            "simp": "人",
            "trad": "人",
            "gloss": "person",
            "statistics": {"hskLevel": 1},
            "items": [
                {"source": "cedict", "pinyin": "rén", "definitions": ["person"], "jyutping": "jan4"},
                {"source": "dong-chinese", "pinyin": "rén", "definitions": ["Dong Chinese's"]},
                {"source": "unicode", "pinyin": "rén", "definitions": ["Unihan's"]},
            ],
        },
        {"_id": "b", "simp": "x", "trad": "x", "items": [{"source": "dong-chinese", "definitions": ["only"]}]},
    ],
    "chinese_char": {
        "char": "人",
        "gloss": "person",
        "cantonese": ["jan4"],
        "statistics": {"hskLevel": 1},
        "pinyinFrequencies": [{"pinyin": "rén"}],
        "oldPronunciations": [{"pinyin": "rén"}],
        "ids": "⿰",
        "images": [{"source": "academia-sinica"}],
        "simpVariants": ["x"],
    },
    "japanese_words": [
        {
            "id": "1",
            "kanji": [{"text": "人", "tags": [], "common": True}],
            "kana": [{"text": "ひと", "tags": [], "common": True, "appliesToKanji": ["*"]}],
            "frequencyRank": 12,
            "sense": [
                {
                    "partOfSpeech": ["n"],
                    "gloss": [{"text": "person", "lang": "eng", "type": None}],
                    "examples": [
                        {
                            "source": {"type": "tatoeba", "value": "12345"},
                            "text": "人",
                            "sentences": [{"land": "jpn", "text": "人だ。"}, {"land": "eng", "text": "A person."}],
                        }
                    ],
                }
            ],
        }
    ],
    "japanese_char": {
        "literal": "人",
        "misc": {"jlptLevel": 4, "grade": 1, "frequency": 5, "strokeCounts": [2], "radicalNames": []},
        "queryCodes": [{"type": "skip", "value": "4-2-0"}],
        "dictionaryReferences": [{"type": "moro", "value": "1"}],
        "readingMeaning": {
            "groups": [
                {
                    "readings": [
                        {"type": "pinyin", "value": "ren2"},
                        {"type": "korean_h", "value": "인"},
                        {"type": "ja_on", "value": "ジン"},
                        {"type": "ja_kun", "value": "ひと"},
                    ],
                    "meanings": [{"lang": "en", "value": "person"}],
                }
            ]
        },
        "ids": "⿰",
    },
    "korean_words": [
        {"id": "k", "hangul": "인", "hanja": "人", "pos": "affix", "pronunciation": "in", "definitions": [{"text": "person", "examples": [{"korean": "x", "translation": "Gemini's"}]}]}
    ],
    "korean_char": {"character": "人", "hanjaForm": "人", "meaningsEn": ["person"], "readings": [{"hangul": "인", "romanization": "in"}]},
    "contained_in_chinese": [{"w": "女人", "p": "nǚ rén", "d": "woman", "fr": 258}],
    "contained_in_japanese": [{"w": "人間", "jp": "にんげん", "d": "human", "c": True, "fr": 140}],
    "contained_in_korean": [{"w": "부인", "d": "wife", "fr": 389}],
    "semantic_mnemonic": {"character": "人", "meaning": "person"},
    "semantic_mnemonic_variants": [],
    "related_japanese_words": ["人"],
}


UNIHAN = (
    "# Unihan_Variants.txt\n"
    "U+8AFA\tkSimplifiedVariant\tU+8C1A\n"
    "U+9762\tkTraditionalVariant\tU+9762 U+9EB5\n"
    "U+9EB5\tkSemanticVariant\tU+9EAA<kMeyerWempe\n"
)
PAIRS = sample.unihan_pairs(UNIHAN)
ROOT = SCRIPTS.parent


class Relations(unittest.TestCase):
    def test_unihan_states_a_simplified_form_from_either_side(self) -> None:
        self.assertIn(("谚", "諺"), PAIRS)
        self.assertIn(("面", "麵"), PAIRS)
        self.assertNotIn(("諺", "谚"), PAIRS)
        self.assertNotIn(("麵", "麪"), PAIRS)

    def test_a_relation_unihan_states_is_kept(self) -> None:
        self.assertEqual(sample.cleared({"key": "谚", "redirect": "諺"}, PAIRS), {"key": "谚", "redirect": "諺"})
        self.assertEqual(
            sample.cleared({"key": "面", "simplified_form_of": "麵"}, PAIRS),
            {"key": "面", "simplified_form_of": "麵"},
        )

    def test_a_relation_unihan_does_not_state_is_left_out(self) -> None:
        self.assertEqual(sample.cleared({"key": "谚", "redirect": "言"}, PAIRS), {"key": "谚"})
        self.assertEqual(sample.cleared({"key": "諺", "simplified_form_of": "谚"}, PAIRS), {"key": "諺"})
        self.assertEqual(sample.cleared({"key": "面", "redirect": ["麵"]}, PAIRS), {"key": "面"})

    def test_every_relation_the_committed_sample_keeps_is_one_of_unihans(self) -> None:
        """Each entry's relation is listed in the MANIFEST with Unihan's lines,
        and those lines state it."""
        data = ROOT / "examples/kiokun/data" / sample.SHARD
        manifest = (data / "MANIFEST.txt").read_text()
        listed = manifest.split("The relations kept,", 1)[1].split("The relations left out", 1)[0]
        stated = {}
        for line in listed.splitlines():
            if ": U+" in line:
                head, lines = line.strip().split(": ", 1)
                stated[tuple(head.split(" "))] = sample.unihan_pairs("\n".join(lines.split("; ")))
        seen = 0
        for f in sorted(data.rglob("*.json.deflate")):
            entry = json.loads(zlib.decompress(f.read_bytes(), -15))
            for field in sample.RELATIONS:
                if field in entry:
                    seen += 1
                    pairs = stated.get((entry["key"], field, entry[field]), {})
                    self.assertIn((entry["key"], entry[field]), pairs, f.name)
        self.assertGreater(seen, 0)


class Notice(unittest.TestCase):
    def test_unicodes_notice_is_reproduced_whole_and_named(self) -> None:
        """The Unicode License v3 asks for its copyright and permission notice
        with the data: in full, not a link (the integrator's ruling of
        2026-10-11)."""
        data = ROOT / "examples/kiokun/data"
        notice = (data / "LICENSE-UNICODE.txt").read_text()
        self.assertTrue(notice.startswith("UNICODE LICENSE V3\n\nCOPYRIGHT AND PERMISSION NOTICE\n\n"))
        self.assertIn("Copyright © 1991-2026 Unicode, Inc.", notice)
        for part in ["NOTICE TO USER:", "Permission is hereby granted", 'PROVIDED "AS IS"', "IN NO EVENT SHALL"]:
            self.assertIn(part, notice)
        self.assertTrue(notice.rstrip().endswith("authorization of the copyright holder."))
        self.assertIn("`LICENSE-UNICODE.txt`, beside this file", (data / "NOTICE.md").read_text())


class Deinflection(unittest.TestCase):
    def test_the_deinflection_sample_keeps_the_key_and_jmdicts_words_alone(self) -> None:
        out = sample.deinflection_kept(ENTRY, PAIRS)
        self.assertEqual(set(out), {"key", "japanese_words"})
        self.assertEqual(out["japanese_words"], sample.cleared(ENTRY, PAIRS)["japanese_words"])

    def test_the_committed_deinflection_sample_holds_nothing_else(self) -> None:
        data = ROOT / "examples/kiokun/data/deinflection"
        files = sorted(data.rglob("*.json.deflate"))
        self.assertEqual(len(files), len(sample.DEINFLECTION_WORDS))
        for f in files:
            entry = json.loads(zlib.decompress(f.read_bytes(), -15))
            self.assertLessEqual(set(entry), {"key", "japanese_words"}, f.name)


class Cleared(unittest.TestCase):
    def setUp(self) -> None:
        self.out = sample.cleared(ENTRY, PAIRS)

    def test_a_chinese_item_is_kept_only_where_the_export_tags_it_cedict(self) -> None:
        words = self.out["chinese_words"]
        self.assertEqual([w["_id"] for w in words], ["a"])
        self.assertEqual(words[0]["items"], [{"pinyin": "rén", "definitions": ["person"], "jyutping": "jan4"}])
        self.assertNotIn("gloss", words[0])
        self.assertNotIn("statistics", words[0])

    def test_dong_chinese_baxter_sagart_chise_and_images_are_left_out(self) -> None:
        self.assertEqual(self.out["chinese_char"], {"char": "人", "cantonese": ["jan4"]})

    def test_jmdict_is_kept_with_its_tatoeba_ids_and_jpdb_is_not(self) -> None:
        word = self.out["japanese_words"][0]
        self.assertNotIn("frequencyRank", word)
        self.assertNotIn("appliesToKanji", word["kana"][0])
        self.assertEqual(word["sense"][0]["examples"][0]["source"], {"type": "tatoeba", "value": "12345"})

    def test_kanjidic2_keeps_its_levels_and_japanese_readings_alone(self) -> None:
        char = self.out["japanese_char"]
        self.assertEqual(set(char), {"literal", "misc", "readingMeaning"})
        self.assertEqual(char["misc"], {"jlptLevel": 4, "grade": 1, "frequency": 5, "strokeCounts": [2]})
        readings = char["readingMeaning"]["groups"][0]["readings"]
        self.assertEqual([r["type"] for r in readings], ["ja_on", "ja_kun"])

    def test_krdict_keeps_its_text_without_ipa_or_machine_translations(self) -> None:
        word = self.out["korean_words"][0]
        self.assertNotIn("pronunciation", word)
        self.assertEqual(word["definitions"], [{"text": "person"}])
        self.assertEqual(self.out["korean_char"]["readings"], [{"hangul": "인"}])

    def test_previews_keep_their_cleared_fields(self) -> None:
        self.assertEqual(self.out["contained_in_chinese"], [{"w": "女人"}])
        self.assertEqual(self.out["contained_in_japanese"], [{"w": "人間", "jp": "にんげん", "d": "human", "c": True}])
        self.assertEqual(self.out["contained_in_korean"], [{"w": "부인", "d": "wife", "fr": 389}])

    def test_the_owners_mnemonics_and_unread_fields_are_left_out(self) -> None:
        for key in ["semantic_mnemonic", "semantic_mnemonic_variants", "related_japanese_words"]:
            self.assertNotIn(key, self.out)

    def test_every_kept_field_names_a_source(self) -> None:
        for path, source in sample.FIELDS:
            self.assertIn(source, sample.SOURCES, path)


if __name__ == "__main__":
    unittest.main()

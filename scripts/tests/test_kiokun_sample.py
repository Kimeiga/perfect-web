"""`kiokun_sample.py`: the repository's sample holds the fields a committed
test reads, from cleared sources only (the integrator's ruling of
2026-10-10)."""

import importlib.util
import pathlib
import unittest

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


class Cleared(unittest.TestCase):
    def setUp(self) -> None:
        self.out = sample.cleared(ENTRY)

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

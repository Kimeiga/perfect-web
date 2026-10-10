#!/usr/bin/env python3
"""Write a sample of kiokun.com's shard `han-1char-3` into the repository.

The repository is public. Committed data is the minimum a committed test
reads, from cleared sources only (the integrator's ruling of 2026-10-10): a
source is cleared when its terms were read at the source and permit its
redistribution here, in separate data files under their own licence. So each
entry is read from a kiokun-data checkout's output (KIOKUN_DATA) and written
back with only the fields named in FIELDS below, each with the input file
kiokun-data's builder made it from (its source at HEAD, `src/main.rs`) and
that source's licence. Everything else is left out: Dong Chinese's own
content (glosses, statistics, pinyin frequencies, comments), Baxter–Sagart's
reconstructions, JPDB's ranks, CHISE's IDS, images, KANJIDIC2's codes and
references, the Korean IPA, and the owner's mnemonics, which the owner rules
on. A field a test reads that is not here is read locally only.

The files stay in kiokun's format: raw DEFLATE, one JSON entry per word, in
the `<subdirectory>/<word>.json.deflate` layout its build writes, so the
slice's host reads kiokun's own format, and the same loader serves this
sample and a whole shard from KIOKUN_DATA. They are no longer kiokun's bytes.

The shard rule is kiokun's (kiokun-data src/main.rs, sveltekit-app
src/lib/shard-utils.ts): count Han characters; hash h = h*31 + codepoint,
wrapping; one Han character is `han-1char-{h % 8 + 1}`; the subdirectory is
`h & 0xFF` in two hex digits.

usage: kiokun_sample.py /path/to/kiokun-data/output_dictionary
"""
import hashlib
import json
import pathlib
import shutil
import sys
import zlib

SHARD = "han-1char-3"
# Common single characters, and four simplified forms whose traditional
# targets are in the same shard: each stub, and the entry it names.
WORDS = "人市魚酒時夢空青上多面色聲樂錢園終半形博" + "谚諺贪貪攒攢缢縊"

# The sources the kept fields come from, each read at its source.
SOURCES = {
    "builder": "kiokun-data's builder (MIT, Hakan Alpay): the entry's structure",
    "cc-cedict": "CC-CEDICT (MDBG), CC BY-SA 4.0, through Dong Chinese's export of it "
    "(data/chinese_dictionary_word_2025-06-25.jsonl, items tagged `cedict`)",
    "cc-canto": "Pleco's CC-Canto readings for CC-CEDICT, CC BY-SA 3.0 "
    "(data/cantonese/cc-cedict-canto, cc-canto: MarvNC's Yomitan conversion)",
    "unihan": "the Unicode Character Database's Unihan, Unicode License v3 "
    "(data/unihan/Unihan_Readings.txt, Unihan_Variants.txt, Unihan_IRGSources.txt)",
    "jmdict": "JMdict (EDRDG), CC BY-SA 4.0, through jmdict-simplified 3.6.1 "
    "(data/jmdict-examples-eng-3.6.1.json)",
    "tatoeba": "Tatoeba's sentences, CC BY 2.0 FR, each with its id, as JMdict's "
    "examples carry them (data/jmdict-examples-eng-3.6.1.json)",
    "kanjidic2": "KANJIDIC2 (EDRDG), CC BY-SA 4.0, through jmdict-simplified 3.6.1 "
    "(data/kanjidic2-en-3.6.1.json)",
    "kanjidic2-muller": "KANJIDIC2's Korean readings, Charles Muller's, under EDRDG's "
    "licence (its section 8), or Unihan's kHangul where KANJIDIC2 has none",
    "jmnedict": "JMnedict (EDRDG), CC BY-SA 4.0, through jmdict-simplified 3.6.1 "
    "(data/jmnedict-all-3.6.1.json)",
    "krdict": "KRDICT (National Institute of Korean Language), CC BY-SA 2.0 KR, through "
    "Lyroxide's Yomitan export (data/krdict-en)",
}

# Each kept field: its path in an entry, `[]` for each item of a list, and its
# source. Nothing else is written.
FIELDS = [
    ("key", "builder"),
    ("redirect", "builder"),
    ("simplified_form_of", "builder"),
    ("chinese_words[]._id", "cc-cedict"),
    ("chinese_words[].simp", "cc-cedict"),
    ("chinese_words[].trad", "cc-cedict"),
    ("chinese_words[].pinyinSearchString", "cc-cedict"),
    ("chinese_words[].items[].pinyin", "cc-cedict"),
    ("chinese_words[].items[].definitions", "cc-cedict"),
    ("chinese_words[].items[].jyutping", "cc-canto"),
    ("chinese_char.char", "builder"),
    ("chinese_char.hkChar", "unihan"),
    ("chinese_char.cantonese", "unihan"),
    ("japanese_words[].id", "jmdict"),
    ("japanese_words[].kanji[].text", "jmdict"),
    ("japanese_words[].kanji[].tags", "jmdict"),
    ("japanese_words[].kanji[].common", "jmdict"),
    ("japanese_words[].kana[].text", "jmdict"),
    ("japanese_words[].kana[].tags", "jmdict"),
    ("japanese_words[].kana[].common", "jmdict"),
    ("japanese_words[].sense[].partOfSpeech", "jmdict"),
    ("japanese_words[].sense[].field", "jmdict"),
    ("japanese_words[].sense[].misc", "jmdict"),
    ("japanese_words[].sense[].dialect", "jmdict"),
    ("japanese_words[].sense[].info", "jmdict"),
    ("japanese_words[].sense[].gloss[].text", "jmdict"),
    ("japanese_words[].sense[].gloss[].type", "jmdict"),
    ("japanese_words[].sense[].gloss[].lang", "jmdict"),
    ("japanese_words[].sense[].examples[].source", "tatoeba"),
    ("japanese_words[].sense[].examples[].text", "tatoeba"),
    ("japanese_words[].sense[].examples[].sentences[].land", "tatoeba"),
    ("japanese_words[].sense[].examples[].sentences[].lang", "tatoeba"),
    ("japanese_words[].sense[].examples[].sentences[].text", "tatoeba"),
    ("japanese_char.literal", "kanjidic2"),
    ("japanese_char.misc.grade", "kanjidic2"),
    ("japanese_char.misc.jlptLevel", "kanjidic2"),
    ("japanese_char.misc.frequency", "kanjidic2"),
    ("japanese_char.misc.strokeCounts", "kanjidic2"),
    ("japanese_char.readingMeaning.groups[].readings[]", "kanjidic2"),
    ("japanese_char.readingMeaning.groups[].meanings[]", "kanjidic2"),
    ("japanese_names[].id", "jmnedict"),
    ("japanese_names[].kanji[].text", "jmnedict"),
    ("japanese_names[].kana[].text", "jmnedict"),
    ("japanese_names[].translation[].type", "jmnedict"),
    ("japanese_names[].translation[].translation[].text", "jmnedict"),
    ("korean_words[].id", "krdict"),
    ("korean_words[].hangul", "krdict"),
    ("korean_words[].hanja", "krdict"),
    ("korean_words[].pos", "krdict"),
    ("korean_words[].definitions[].text", "krdict"),
    ("korean_char.character", "builder"),
    ("korean_char.hanjaForm", "unihan"),
    ("korean_char.meaningsEn", "kanjidic2"),
    ("korean_char.readings[].hangul", "kanjidic2-muller"),
    ("contained_in_chinese[].w", "cc-cedict"),
    ("contained_in_japanese[].w", "jmdict"),
    ("contained_in_japanese[].jp", "jmdict"),
    ("contained_in_japanese[].d", "jmdict"),
    ("contained_in_japanese[].c", "jmdict"),
    ("contained_in_korean[].w", "krdict"),
    ("contained_in_korean[].d", "krdict"),
    ("contained_in_korean[].fr", "krdict"),
]

# KANJIDIC2's readings kept: on'yomi and kun'yomi. Its pinyin (Wittern and
# Yasuoka), Korean (Muller) and Vietnamese readings are not read from here.
KANJI_READINGS = {"ja_on", "ja_kun"}

HAN = [(0x4E00, 0x9FFF), (0x3400, 0x4DBF), (0x20000, 0x2A6DF), (0x2A700, 0x2B73F),
       (0x2B740, 0x2B81F), (0x2B820, 0x2CEAF), (0x2CEB0, 0x2EBEF), (0x30000, 0x3134F),
       (0x31350, 0x323AF), (0x2EBF0, 0x2EE5F), (0xF900, 0xFAFF), (0x2F800, 0x2FA1F)]


def han(c):
    return any(lo <= ord(c) <= hi for lo, hi in HAN)


def kiokun_hash(word):
    h = 0
    for c in word:
        h = (h * 31 + ord(c)) & 0xFFFFFFFF
    return h


def shard(word):
    n, h = sum(han(c) for c in word), kiokun_hash(word)
    if n == 0:
        return f"non-han-{h % 4 + 1}"
    return f"{['han-1char', 'han-2char', 'han-3plus'][min(n, 3) - 1]}-{h % 8 + 1}"


def tree(paths):
    """The kept paths as a tree: each key to its subtree, `[]` a list's items,
    an empty tree a value kept whole."""
    root = {}
    for path in paths:
        node = root
        for part in path.replace("[]", ".[]").split("."):
            node = node.setdefault(part, {})
    return root


def kept(value, node):
    """`value` with only what `node` names."""
    if not node:
        return value
    if isinstance(value, list):
        return [kept(item, node["[]"]) for item in value] if "[]" in node else value
    if isinstance(value, dict):
        return {k: kept(value[k], sub) for k, sub in node.items() if k in value}
    return value


def cleared(entry):
    """The entry with only the kept fields, from cleared sources."""
    entry = json.loads(json.dumps(entry))
    # A Chinese item is CC-CEDICT's only when its export tags it so.
    for word in entry.get("chinese_words", []):
        word["items"] = [i for i in word.get("items", []) if i.get("source") == "cedict"]
    entry["chinese_words"] = [w for w in entry.get("chinese_words", []) if w["items"]]
    if not entry["chinese_words"]:
        del entry["chinese_words"]
    rm = (entry.get("japanese_char") or {}).get("readingMeaning") or {}
    for group in rm.get("groups", []):
        group["readings"] = [r for r in group.get("readings", []) if r.get("type") in KANJI_READINGS]
    out = kept(entry, tree(p for p, _ in FIELDS))
    return {k: v for k, v in out.items() if v not in (None, [], {})}


def main():
    source = pathlib.Path(sys.argv[1])
    root = pathlib.Path(__file__).resolve().parent.parent
    out = root / "examples/kiokun/data" / SHARD
    if out.exists():
        shutil.rmtree(out)
    lines = []
    for word in WORDS:
        assert shard(word) == SHARD, f"{word} is in {shard(word)}"
        sub = format(kiokun_hash(word) & 0xFF, "02x")
        src = source / sub / f"{word}.json.deflate"
        entry = json.loads(zlib.decompress(src.read_bytes(), -15))
        assert entry["key"] == word, src
        text = json.dumps(cleared(entry), ensure_ascii=False, separators=(",", ":"))
        c = zlib.compressobj(9, zlib.DEFLATED, -15)
        data = c.compress(text.encode()) + c.flush()
        (out / sub).mkdir(parents=True, exist_ok=True)
        (out / sub / src.name).write_bytes(data)
        kind = f"redirect -> {entry['redirect']}" if "redirect" in entry else "entry"
        lines.append(f"{sub}/{src.name}  {len(data):6d}  {hashlib.sha256(data).hexdigest()}  {kind}")
    fields = "\n".join(f"  {path:56s} {name}" for path, name in FIELDS)
    sources = "\n".join(f"  {name:17s} {text}" for name, text in SOURCES.items())
    (out / "MANIFEST.txt").write_text(
        f"kiokun.com shard {SHARD}: {len(lines)} entries read from kiokun-data's output and written\n"
        f"with only the fields below by scripts/kiokun_sample.py; see ../NOTICE.md.\n\n"
        + "\n".join(lines)
        + "\n\nThe fields kept, and the source each is made from:\n\n"
        + fields
        + "\n\nThe sources:\n\n"
        + sources
        + "\n"
    )
    print(f"{len(lines)} files -> {out}")


if __name__ == "__main__":
    main()

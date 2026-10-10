#!/usr/bin/env python3
"""Write the repository's sample of kiokun.com's search index, for CI.

kiokun.com searches one FTS5 table over the builder's search index CSV
(kiokun-data `output_search_index.csv`, committed at a47956fb). CI has no
kiokun-data checkout, so it searches a sample of that CSV, from cleared
sources only (the integrator's ruling of 2026-10-10): committed data is the
minimum a committed test reads.

The rows kept are those whose word holds one of WORDS' characters, and is
no longer than LONGEST characters. Of them:
- a Chinese row is kept only where its definition is an item's that Dong
  Chinese's export (`data/chinese_dictionary_word_2025-06-25.jsonl`, read at
  kiokun-data's HEAD) tags `cedict` (CC-CEDICT) or `unicode` (Unihan) for
  that traditional form, the builder's key;
- a Japanese row is JMdict's;
- a Korean row is KRDICT's; its pronunciation, the IPA of an export whose
  terms are not stated (`data/krdict-ipa`), is left empty.
Everything is read with `git show HEAD:…`, never from the working tree, and
nothing is written in the checkout.

usage: kiokun_search_sample.py /path/to/kiokun-data
"""
import csv
import io
import json
import os
import pathlib
import subprocess
import sys

# The characters whose words CI searches: 人, from the entries'
# sample, and 学 and 學, for a script variant the hand-made aliases name.
WORDS = "人学學"
LONGEST = 2


def committed(root: pathlib.Path, path: str) -> str:
    env = {**os.environ, "GIT_OPTIONAL_LOCKS": "0"}
    return subprocess.run(
        ["git", "-C", str(root), "show", f"HEAD:./{path}"],
        check=True,
        capture_output=True,
        env=env,
    ).stdout.decode()


def cleared_chinese(root: pathlib.Path) -> set[tuple[str, str]]:
    """Each (traditional form, definition) a CC-CEDICT or Unihan item gives."""
    out = set()
    for line in committed(root, "data/chinese_dictionary_word_2025-06-25.jsonl").splitlines():
        entry = json.loads(line)
        if not any(c in entry.get("trad", "") for c in WORDS):
            continue
        for item in entry.get("items", []):
            if item.get("source") in ("cedict", "unicode"):
                for d in item.get("definitions") or []:
                    out.add((entry["trad"], d))
    return out


def kept_rows(header: list[str], rows, chinese: set[tuple[str, str]]) -> list[list[str]]:
    """The rows the sample keeps, from cleared sources."""
    at = {name: i for i, name in enumerate(header)}
    kept = []
    for row in rows:
        row = list(row)
        word = row[at["word"]]
        if len(word) > LONGEST or not any(c in word for c in WORDS):
            continue
        language = row[at["language"]]
        if language == "chinese" and (word, row[at["definition"]]) not in chinese:
            continue
        if language == "korean":
            row[at["pronunciation"]] = ""
        if language not in ("chinese", "japanese", "korean"):
            continue
        kept.append(row)
    return kept


def main() -> int:
    root = pathlib.Path(sys.argv[1])
    repo = pathlib.Path(__file__).resolve().parent.parent
    chinese = cleared_chinese(root)
    reader = csv.reader(io.StringIO(committed(root, "output_search_index.csv")))
    header = next(reader)
    at = {name: i for i, name in enumerate(header)}
    kept = kept_rows(header, reader, chinese)
    out = io.StringIO()
    writer = csv.writer(out, lineterminator="\n")
    writer.writerow(header)
    writer.writerows(kept)
    path = repo / "examples/kiokun/data/search-sample.csv"
    path.write_text(out.getvalue(), encoding="utf-8")
    counts = {lang: sum(1 for r in kept if r[at["language"]] == lang) for lang in ("chinese", "japanese", "korean")}
    print(f"{len(kept)} rows -> {path} ({counts})")
    return 0


if __name__ == "__main__":
    sys.exit(main())

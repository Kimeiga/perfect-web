#!/usr/bin/env python3
"""Copy a sample of kiokun.com's shard `han-1char-3` into the repository.

The files are copied byte for byte: raw DEFLATE, one JSON entry per word, in the
`<subdirectory>/<word>.json.deflate` layout kiokun's build writes. So the slice's
host reads kiokun's own format, and the same loader serves this sample and a
whole shard from a kiokun-data checkout (KIOKUN_DATA).

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
        data = src.read_bytes()
        entry = json.loads(zlib.decompress(data, -15))
        assert entry["key"] == word, src
        (out / sub).mkdir(parents=True, exist_ok=True)
        (out / sub / src.name).write_bytes(data)
        kind = f"redirect -> {entry['redirect']}" if "redirect" in entry else "entry"
        lines.append(f"{sub}/{src.name}  {len(data):6d}  {hashlib.sha256(data).hexdigest()}  {kind}")
    (out / "MANIFEST.txt").write_text(
        f"kiokun.com shard {SHARD}: {len(lines)} files copied byte for byte by "
        f"scripts/kiokun_sample.py\n\n" + "\n".join(lines) + "\n"
    )
    print(f"{len(lines)} files -> {out}")


if __name__ == "__main__":
    main()

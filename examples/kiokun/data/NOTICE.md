# The kiokun sample shard: sources and licences

`han-1char-3/` holds 28 files copied byte for byte from kiokun.com's build output
(the `kiokun-data` repository, `output_dictionary/`) by
`scripts/kiokun_sample.py`. `han-1char-3/MANIFEST.txt` lists each with its SHA-256.

The dictionary content in these files is derived from:

- **CC-CEDICT**, © MDBG, licensed under the Creative Commons
  Attribution-ShareAlike 4.0 International licence (CC BY-SA 4.0),
  <https://www.mdbg.net/chinese/dictionary?page=cedict>;
- **JMdict**, © the Electronic Dictionary Research and Development Group,
  used in conformance with the Group's licence (CC BY-SA 4.0),
  <https://www.edrdg.org/edrdg/licence.html>;
- Tatoeba example sentences carried in some entries, CC BY 2.0 FR,
  <https://tatoeba.org>.

These files, and only these files, are under those licences. The rest of this
repository is under its own licence (`LICENSE`). The kiokun build that merged
them is MIT-licensed (`kiokun-data/LICENSE`).

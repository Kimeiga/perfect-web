# The oracle fixtures: sources and licences

Each `*-sample.json` here holds kiokun.com's answers for the words of the
repository's kiokun sample (`examples/kiokun/data/han-1char-3/`): text
derived from kiokun.com's data for those words, such as titles,
descriptions, example sentences and the words that contain them. They are
written by the `just e14-kiokun-*` recipes, which run kiokun.com's own code
locally; the code itself is not here.

That text is derived from the same sources as the sample, under the same
licences and with the same attributions:

- **CC-CEDICT**, © MDBG, licensed under the Creative Commons
  Attribution-ShareAlike 4.0 International licence (CC BY-SA 4.0),
  <https://www.mdbg.net/chinese/dictionary?page=cedict>;
- **JMdict**, © the Electronic Dictionary Research and Development Group,
  used in conformance with the Group's licence (CC BY-SA 4.0),
  <https://www.edrdg.org/edrdg/licence.html>;
- Tatoeba example sentences, CC BY 2.0 FR, <https://tatoeba.org>.

See `examples/kiokun/data/NOTICE.md`. These files, and only the
`*-sample.json` files here, are under those licences; the rest of this
directory, and of the repository, is under the repository's own licence
(`LICENSE`).

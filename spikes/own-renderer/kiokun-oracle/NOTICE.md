# The oracle fixtures: sources and licences

Each `*-sample.json` here holds kiokun.com's answers for the words of the
repository's kiokun sample (`examples/kiokun/data/han-1char-3/`). The
`just e14-kiokun-*` recipes compute them by running kiokun.com's own code,
copied from kiokun-data's commit at HEAD into a temporary directory, over the
sample; that code is not here. Each fixture names the source of each of its
fields in its `provenance`, and holds only the fields its CI test compares
(the integrator's ruling of 2026-10-10). A field from a source not cleared
is not here; it is held locally, by the same recipes over a kiokun-data
checkout.

**These files, and only the `*-sample.json` files here, are distributed under
the Creative Commons Attribution-ShareAlike 4.0 International licence
(CC BY-SA 4.0), <https://creativecommons.org/licenses/by-sa/4.0/>**, as the
sources' terms allow; the rest of this directory, and of the repository, is
under the repository's own licence (`LICENSE`, Apache-2.0).

What was done: the sample's fields, extracted by kiokun-data's builder and
trimmed by `scripts/kiokun_sample.py` (see `examples/kiokun/data/NOTICE.md`),
were turned by kiokun.com's own functions into what its page shows, and
written here.

## Each fixture

- **`seo-sample.json`**: each word's page title and description, as
  kiokun.com's `buildDictionarySeo` writes them: the word, and its meanings
  from CC-CEDICT's definitions, JMdict's glosses and KRDICT's definitions.
- **`examples-sample.json`**: the Japanese example sentences and their
  English translations each word's senses show (kiokun.com's
  `japaneseExamplesForSense`), from Tatoeba through JMdict, and under
  `tatoeba` each sentence's Tatoeba id, which credits its author at
  `https://tatoeba.org/sentences/show/<id>`.
- **`moves-sample.json`**: written forms alone, and the word each moves to
  (kiokun.com's `equivalentTraditionalTarget`).
- **`contains-sample.json`**: each word's Appears in columns (kiokun.com's
  `rankAppearsIn`), each trimmed to its length and its first 20 and last 5
  items: Chinese words' forms from CC-CEDICT; Japanese words, readings,
  common flags and glosses from JMdict; Korean words and definitions from
  KRDICT. Chinese readings, Jyutping and definitions are held locally.
- **`search-sample.json`**: what kiokun.com's `api/search` answers for 97
  queries taken from the repository's search index sample and six hand-made
  ones, over that sample and its hand-made aliases: each hit's forms,
  readings and definitions from CC-CEDICT's and Unihan's definitions and
  JMdict's glosses, with the builder's romanizations.

## The sources

The same as the sample's, each read at its primary source on 2026-10-10; see
`examples/kiokun/data/NOTICE.md` for each rights holder, licence, URL and
the files they come through:

- **CC-CEDICT**: CC BY-SA 4.0 (the CC-CEDICT project, at MDBG), through Dong
  Chinese's export, its `cedict` items alone.
- **JMdict**: CC BY-SA 4.0, the property of the Electronic Dictionary
  Research and Development Group, used in conformance with the Group's
  licence (<https://www.edrdg.org/edrdg/licence.html>).
- **Tatoeba**: CC BY 2.0 FR, each sentence credited to its author by its id.
- **KRDICT**: CC BY-SA 2.0 KR, the National Institute of Korean Language
  (국립국어원); its text alone, no media.
- **Unihan**: the Unicode License v3 (© 1991-2026 Unicode, Inc.,
  <https://www.unicode.org/license.txt>), through Dong Chinese's export,
  its items tagged `unicode`.

// kiokun.com's own Homophones section, run over a sample of kiokun's entries
// and its search index: the oracle the rewrite's section is held to (track
// `kiokun`, `just e14-kiokun-homophones`; the integrator's rulings of
// 2026-10-09 on oracles and of 2026-10-11 on homophones).
//
// usage: node --experimental-strip-types homophones.mjs LIB DATA STRIDE OUT
//   LIB     a directory holding copies of kiokun.com's
//           `src/routes/api/lookup-reading/+server.ts` (as `lookup.ts`) and
//           `migrations/0005_search_index_with_jyutping.sql` (as `schema.sql`),
//           and kiokun-data's `output_search_index.csv` (as `index.csv`, or the
//           repository's sample of it), each copied by the recipe from its
//           commit at HEAD (`scripts/kiokun_app_source.py`; never committed)
//   DATA    a directory of kiokun's entries in its build's layout: the whole
//           `output_dictionary`, or the repository's sample
//   STRIDE  every how manyth file of each subdirectory, in name order
//   OUT     where the JSON goes: { stride, read, answered, lookups, named,
//           provenance, answers }, `answers` mapping each word with Japanese
//           words to its groups, [{ reading, words: [{ word, pronunciation,
//           is_common }] }], an empty list where it has none
//
// **The endpoint** is kiokun.com's own, over the index as search.mjs loads it
// (index.mjs, `is_common` an integer), with D1's binding the stand-in
// d1.mjs and SvelteKit's `json` and `error` kit.mjs's. Each reading is
// asked once, as the page asks it: `/api/lookup-reading?q=<reading>&limit=20`.
//
// **The page's part** is in `[word]/+page.svelte`'s script, an `$effect`
// that is no module, so it is transcribed here, named, not imported:
//   - the word's own forms are the page's word and each Japanese word's
//     kanji and kana texts;
//   - its readings are each Japanese word's kana texts, each once, in order;
//   - each reading's results are kept where the word is none of the forms
//     and not the reading itself, and a reading with none left has no group.
// The page adds each group when its fetch answers, so its groups' order is
// whichever request finished first; here they are in the readings' order
// (the rewrite's, Difference: the search ADR's homophones).
//
// Only entries kiokun.com's loader shows as they are, as contains.mjs takes
// them: no stub, and no character with simplified variants or a form it is
// the simplified form of.
//
// **Named differences**: none here.
import { readFileSync, readdirSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { pathToFileURL } from "node:url";
import { inflateRawSync } from "node:zlib";
import { d1Of } from "./d1.mjs";
import { indexed } from "./index.mjs";
import { prepared } from "./kit.mjs";

const [, , lib, data, stride, out] = process.argv;
const step = Number(stride);

prepared(lib, ["lookup.ts"], {});
const { loaded } = indexed(lib);
const platform = { env: { DB: d1Of(loaded(true)) } };
const { GET } = await import(pathToFileURL(join(lib, "lookup.ts")).href);

const asked = new Map();
async function lookedUp(reading) {
  if (!asked.has(reading)) {
    const url = new URL(`http://kiokun/api/lookup-reading?q=${encodeURIComponent(reading)}&limit=20`);
    try {
      const response = await GET({ url, platform });
      asked.set(reading, { results: (await response.json()).results });
    } catch (e) {
      asked.set(reading, { error: e?.status ? `${e.status}: ${e.body?.message}` : String(e) });
    }
  }
  return asked.get(reading);
}

const answers = {};
let read = 0;
const failed = [];
for (const sub of readdirSync(data).filter((d) => /^[0-9a-f]{2}$/.test(d)).sort()) {
  const files = readdirSync(join(data, sub)).filter((f) => f.endsWith(".json.deflate")).sort();
  for (let i = 0; i < files.length; i += step) {
    const entry = JSON.parse(inflateRawSync(readFileSync(join(data, sub, files[i]))).toString("utf8"));
    read += 1;
    const merges = entry.chinese_char?.simpVariants?.length || entry.simplified_form_of;
    if (entry.redirect || merges || !entry.key) continue;
    const japanese = entry.japanese_words || [];
    if (!japanese.length) continue;
    // The page's part, transcribed (above).
    const forms = new Set(
      [entry.key, ...japanese.flatMap((w) => [...(w.kanji || []).map((k) => k.text), ...(w.kana || []).map((k) => k.text)])].filter(Boolean),
    );
    const readings = [];
    for (const w of japanese) for (const k of w.kana || []) if (k.text && !readings.includes(k.text)) readings.push(k.text);
    const groups = [];
    for (const reading of readings) {
      const answer = await lookedUp(reading);
      if (answer.error) {
        failed.push(`${entry.key} ${reading}: ${answer.error}`);
        continue;
      }
      const words = answer.results.filter((w) => !forms.has(w.word) && w.word !== reading);
      if (words.length) groups.push({ reading, words });
    }
    answers[entry.key] = groups;
  }
}

// What each field of the answers is made from (kiokun-oracle/NOTICE.md).
const provenance = {
  "reading, words[].word, words[].pronunciation, words[].is_common":
    "JMdict (EDRDG, CC BY-SA 4.0, through jmdict-simplified 3.6.1): each Japanese word's kana, and the search index's Japanese rows (a JMdict entry's first kanji or kana, its first kana, its common flag), as kiokun-data's builder writes them",
  "the index": "kiokun-data's output_search_index.csv at a47956fb, or the repository's sample of it (examples/kiokun/data/search-sample.csv)",
};
writeFileSync(
  out,
  JSON.stringify(
    { stride: step, read, answered: Object.keys(answers).length, lookups: asked.size, named: {}, failed, provenance, answers },
    null,
    1,
  ),
);
console.log(`stride ${step}: read ${read} entries; ${Object.keys(answers).length} with Japanese words; ${asked.size} readings looked up; ${failed.length} failed`);

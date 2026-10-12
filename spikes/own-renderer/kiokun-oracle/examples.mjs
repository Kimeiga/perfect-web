// kiokun.com's own `japaneseExamplesForSense`, run over a sample of kiokun's
// entries: the oracle the rewrite's Japanese examples are held to (track
// `kiokun`, `just e14-kiokun-examples`; the integrator's ruling of
// 2026-10-09 on oracles).
//
// usage: node --experimental-strip-types examples.mjs LIB DATA STRIDE OUT
//   LIB     a directory holding a copy of kiokun.com's
//           `src/lib/japanese-examples.ts`, copied by the recipe from
//           KIOKUN_APP's commit at HEAD each run
//           (`scripts/kiokun_app_source.py`; never committed here)
//   DATA    a directory of kiokun's entries in its build's layout
//   STRIDE  every how manyth file of each subdirectory, in name order
//   OUT     where the JSON goes: { stride, read, answered, named, provenance, answers, tatoeba },
//           `answers` mapping each word to the examples its Japanese words'
//           senses show, in order, each `text / translation`
//
// Only entries kiokun.com's loader shows as they are (no stub, no character
// with simplified variants or a form it is the simplified form of), and
// only those with a Japanese word. No difference is named: any is a defect.
import { readdirSync, readFileSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { pathToFileURL } from "node:url";
import { inflateRawSync } from "node:zlib";

const [, , lib, data, stride, out] = process.argv;
// Every module the copied file imports must be one the recipe copied:
// nothing is stubbed. `japanese-examples.ts` imports types alone.
const source = readFileSync(join(lib, "japanese-examples.ts"), "utf8");
for (const [, spec] of source.matchAll(/^import\s+(?!type\b)[^'"]*['"]([^'"]+)['"]/gm)) {
  throw new Error(`japanese-examples.ts imports ${spec}, which the harness does not copy`);
}
const { japaneseExamplesForSense } = await import(
  pathToFileURL(join(lib, "japanese-examples.ts")).href
);
const step = Number(stride);
const answers = {};
// Each shown example's Tatoeba sentence id, as JMdict's example records carry
// it (`source: { type: "tatoeba", value }`): Tatoeba's licence (CC BY 2.0 FR)
// attributes a sentence to its author, at tatoeba.org/sentences/show/<id>.
const tatoeba = {};
let read = 0;
for (const sub of readdirSync(data).filter((d) => /^[0-9a-f]{2}$/.test(d)).sort()) {
  const files = readdirSync(join(data, sub)).filter((f) => f.endsWith(".json.deflate")).sort();
  for (let i = 0; i < files.length; i += step) {
    const entry = JSON.parse(inflateRawSync(readFileSync(join(data, sub, files[i]))).toString("utf8"));
    read += 1;
    const merges = entry.chinese_char?.simpVariants?.length || entry.simplified_form_of;
    if (entry.redirect || merges || !entry.key || !entry.japanese_words?.length) continue;
    const shown = entry.japanese_words.flatMap((word) =>
      (word.sense || []).flatMap((sense) =>
        japaneseExamplesForSense(sense).map((e) => {
          const record = (sense.examples || []).find((x) =>
            (x.sentences || []).some((t) => (t.land === "jpn" || t.lang === "jpn") && t.text === e.text),
          );
          if (record?.source?.type !== "tatoeba" || !record.source.value) {
            throw new Error(`${entry.key}: an example without its Tatoeba id: ${e.text}`);
          }
          return { shown: `${e.text} / ${e.translation ?? ""}`, id: String(record.source.value) };
        }),
      ),
    );
    answers[entry.key] = shown.map((e) => e.shown);
    tatoeba[entry.key] = shown.map((e) => e.id);
  }
}
// What each field of the answers is made from, which the fixture carries
// (kiokun-oracle/NOTICE.md; the integrator's ruling of 2026-10-10).
const provenance = {
  'answers (sentence / translation)': "Tatoeba (CC BY 2.0 FR), each sentence with its id, as JMdict's examples carry them (kiokun-data data/jmdict-examples-eng-3.6.1.json)",
  'tatoeba': "each shown sentence's Tatoeba id, attributing it to its author at tatoeba.org/sentences/show/<id>",
  'the sample': "read from the repository's sample (examples/kiokun/data/han-1char-3, its MANIFEST), which holds cleared fields alone; see NOTICE.md",
};
writeFileSync(
  out,
  JSON.stringify({ stride: step, read, answered: Object.keys(answers).length, named: {}, provenance, answers, tatoeba }, null, 1),
);
console.log(`stride ${step}: read ${read} entries; ${Object.keys(answers).length} answered; named differences {}`);

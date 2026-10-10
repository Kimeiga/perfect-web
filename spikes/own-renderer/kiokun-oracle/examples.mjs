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
//   OUT     where the JSON goes: { stride, read, answered, named, answers },
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
let read = 0;
for (const sub of readdirSync(data).filter((d) => /^[0-9a-f]{2}$/.test(d)).sort()) {
  const files = readdirSync(join(data, sub)).filter((f) => f.endsWith(".json.deflate")).sort();
  for (let i = 0; i < files.length; i += step) {
    const entry = JSON.parse(inflateRawSync(readFileSync(join(data, sub, files[i]))).toString("utf8"));
    read += 1;
    const merges = entry.chinese_char?.simpVariants?.length || entry.simplified_form_of;
    if (entry.redirect || merges || !entry.key || !entry.japanese_words?.length) continue;
    answers[entry.key] = entry.japanese_words.flatMap((word) =>
      (word.sense || []).flatMap((sense) =>
        japaneseExamplesForSense(sense).map((e) => `${e.text} / ${e.translation ?? ""}`),
      ),
    );
  }
}
writeFileSync(
  out,
  JSON.stringify({ stride: step, read, answered: Object.keys(answers).length, named: {}, answers }, null, 1),
);
console.log(`stride ${step}: read ${read} entries; ${Object.keys(answers).length} answered; named differences {}`);

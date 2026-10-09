// kiokun.com's own `buildDictionarySeo`, run over a sample of the whole
// dictionary: the oracle the rewrite's page head is held to (track `kiokun`,
// `just e14-kiokun-seo`).
//
// usage: node --experimental-strip-types seo.mjs LIB DATA EVERY OUT
//   LIB    a directory holding copies of kiokun.com's `src/lib/seo.ts` and
//          `src/lib/character-forms.ts`, made by the recipe from KIOKUN_APP
//          (never committed here: the owner's source stays in kiokun-data)
//   DATA   KIOKUN_DATA, the build's `output_dictionary`
//   EVERY  every how manyth file of each subdirectory, in name order
//   OUT    where the JSON goes: word -> { title, description }
//
// Only entries kiokun.com's loader shows as they are: no stub (a stub's page
// is its target's, with the stub's card), and no character with simplified
// variants or a form it is the simplified form of (whose page may merge an
// equivalent form's data). For those, `buildDictionarySeo(word, entry)` is
// exactly what kiokun.com's page computes.
import { readdirSync, readFileSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { pathToFileURL } from "node:url";
import { inflateRawSync } from "node:zlib";

const [, , lib, data, every, out] = process.argv;
const { buildDictionarySeo } = await import(pathToFileURL(join(lib, "seo.ts")).href);
const step = Number(every);
const answers = {};
let read = 0;
for (const sub of readdirSync(data).filter((d) => /^[0-9a-f]{2}$/.test(d)).sort()) {
  const files = readdirSync(join(data, sub)).filter((f) => f.endsWith(".json.deflate")).sort();
  for (let i = 0; i < files.length; i += step) {
    const entry = JSON.parse(inflateRawSync(readFileSync(join(data, sub, files[i]))).toString("utf8"));
    read += 1;
    const merges = entry.chinese_char?.simpVariants?.length || entry.simplified_form_of;
    if (entry.redirect || merges || !entry.key) continue;
    const seo = buildDictionarySeo(entry.key, entry);
    answers[entry.key] = { title: seo.title, description: seo.description };
  }
}
writeFileSync(out, JSON.stringify(answers));
console.log(`read ${read} entries; ${Object.keys(answers).length} answered`);

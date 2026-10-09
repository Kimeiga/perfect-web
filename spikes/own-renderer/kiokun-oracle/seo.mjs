// kiokun.com's own `buildDictionarySeo`, run over a sample of kiokun's
// entries: the oracle the rewrite's page head is held to (track `kiokun`,
// `just e14-kiokun-seo`; the integrator's ruling of 2026-10-09 on oracles).
//
// usage: node --experimental-strip-types seo.mjs LIB DATA STRIDE OUT
//   LIB     a directory holding copies of kiokun.com's `src/lib/seo.ts` and
//           `src/lib/character-forms.ts`, made by the recipe from KIOKUN_APP
//           each run (never committed here: the owner's source stays in
//           kiokun-data)
//   DATA    a directory of kiokun's entries in its build's layout: the whole
//           `output_dictionary`, or the repository's sample
//   STRIDE  every how manyth file of each subdirectory, in name order
//   OUT     where the JSON goes: { stride, read, answered, named, answers },
//           `answers` mapping each word to its { title, description } and the
//           differences named for it
//
// Only entries kiokun.com's loader shows as they are: no stub (a stub's page
// is its target's, with the stub's card), and no character with simplified
// variants or a form it is the simplified form of (whose page may merge an
// equivalent form's data). For those, `buildDictionarySeo(word, entry)` is
// what kiokun.com's page computes.
//
// **Named differences** (the word page's ADRs): where kiokun.com's answer
// differs from the rewrite's by a difference the ADRs name, the answer
// carries the rewrite's text and the difference's name, and the harness
// counts it. Any other difference fails the comparison.
// - `surrogate-pair cut`: JavaScript's `slice` can cut a code point past
//   U+FFFF in half; the rewrite stops before it (ADR-0293, the page head,
//   Differences 14).
import { readdirSync, readFileSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { pathToFileURL } from "node:url";
import { inflateRawSync } from "node:zlib";

const [, , lib, data, stride, out] = process.argv;
// Every module the copied files import must be one the recipe copied:
// nothing is stubbed (the ruling's fifth point).
for (const file of ["seo.ts", "character-forms.ts"]) {
  const source = readFileSync(join(lib, file), "utf8");
  for (const [, spec] of source.matchAll(/^import\s+(?!type\b)[^'"]*['"]([^'"]+)['"]/gm)) {
    if (!["./character-forms.ts"].includes(spec)) {
      throw new Error(`${file} imports ${spec}, which the harness does not copy`);
    }
  }
}
const { buildDictionarySeo } = await import(pathToFileURL(join(lib, "seo.ts")).href);
const step = Number(stride);

// A lone surrogate, as a cut leaves one.
const LONE = /[\uD800-\uDBFF](?![\uDC00-\uDFFF])|(?<![\uD800-\uDBFF])[\uDC00-\uDFFF]/;

// The rewrite's text for a truncated value whose cut split a pair: the half
// pair dropped, and the rest trimmed before the ellipsis, as the rewrite cuts.
function withoutHalf(value) {
  if (!value.endsWith("…")) return value;
  return `${value.slice(0, -1).replace(new RegExp(LONE.source, "g"), "").trimEnd()}…`;
}

const answers = {};
const named = {};
let read = 0;
for (const sub of readdirSync(data).filter((d) => /^[0-9a-f]{2}$/.test(d)).sort()) {
  const files = readdirSync(join(data, sub)).filter((f) => f.endsWith(".json.deflate")).sort();
  for (let i = 0; i < files.length; i += step) {
    const entry = JSON.parse(inflateRawSync(readFileSync(join(data, sub, files[i]))).toString("utf8"));
    read += 1;
    const merges = entry.chinese_char?.simpVariants?.length || entry.simplified_form_of;
    if (entry.redirect || merges || !entry.key) continue;
    const seo = buildDictionarySeo(entry.key, entry);
    const answer = { title: seo.title, description: seo.description, named: [] };
    for (const field of ["title", "description"]) {
      if (LONE.test(answer[field])) {
        answer[field] = withoutHalf(answer[field]);
        answer.named.push("surrogate-pair cut");
        named["surrogate-pair cut"] = (named["surrogate-pair cut"] ?? 0) + 1;
      }
    }
    answers[entry.key] = answer;
  }
}
writeFileSync(
  out,
  JSON.stringify({ stride: step, read, answered: Object.keys(answers).length, named, answers }, null, 1),
);
console.log(`stride ${step}: read ${read} entries; ${Object.keys(answers).length} answered; named differences ${JSON.stringify(named)}`);

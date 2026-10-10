// kiokun.com's own `equivalentTraditionalTarget`, run over a sample of
// kiokun's entries: the oracle the rewrite's word-page move is held to
// (track `kiokun`, `just e14-kiokun-moves`; the integrator's ruling of
// 2026-10-09 on oracles).
//
// usage: node --experimental-strip-types moves.mjs LIB DATA STRIDE OUT
//   LIB     a directory holding a copy of kiokun.com's
//           `src/lib/character-forms.ts`, copied by the recipe from KIOKUN_APP's
//           commit at HEAD each run (`scripts/kiokun_app_source.py`; never
//           committed here: the owner's source stays in kiokun-data)
//   DATA    a directory of kiokun's entries in its build's layout: the whole
//           `output_dictionary`, or the repository's sample
//   STRIDE  every how manyth control, in name order
//   OUT     where the JSON goes: { stride, read, candidates, controls,
//           answered, moved, named, provenance, answers }, `answers` mapping each word to
//           the word its page moves to, or null
//
// kiokun.com's page answers 308 to `equivalentTraditionalTarget(data)` where
// that is a word other than the one asked for, `data` being the word's file
// as parsed, before a stub is followed or forms merged (`[word]/+page.ts`,
// 415-425). The function answers null at once for an entry with no
// `simplified_form_of`, and for a source of more than one character; so the
// sample is every entry whose file name is one code point:
// - **candidates**: each such entry with a `simplified_form_of`, all of
//   them, the ones that move and the ones whose meanings differ (后/後);
// - **controls**: every STRIDE-th of the others, in each subdirectory's
//   name order.
//
// **Named differences**: none.
import { readdirSync, readFileSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { pathToFileURL } from "node:url";
import { inflateRawSync } from "node:zlib";

const [, , lib, data, stride, out] = process.argv;
// Every module the copied file imports must be one the recipe copied:
// nothing is stubbed (the ruling's fifth point). Type imports are erased.
const source = readFileSync(join(lib, "character-forms.ts"), "utf8");
for (const [, spec] of source.matchAll(/^import\s+(?!type\b)[^'"]*['"]([^'"]+)['"]/gm)) {
  throw new Error(`character-forms.ts imports ${spec}, which the harness does not copy`);
}
const { equivalentTraditionalTarget } = await import(
  pathToFileURL(join(lib, "character-forms.ts")).href
);
const step = Number(stride);
const SUFFIX = ".json.deflate";

const answers = {};
let read = 0;
let candidates = 0;
let controls = 0;
let moved = 0;
for (const sub of readdirSync(data).filter((d) => /^[0-9a-f]{2}$/.test(d)).sort()) {
  const files = readdirSync(join(data, sub))
    .filter((f) => f.endsWith(SUFFIX) && [...f.slice(0, -SUFFIX.length)].length === 1)
    .sort();
  let others = 0;
  for (const file of files) {
    const entry = JSON.parse(inflateRawSync(readFileSync(join(data, sub, file))).toString("utf8"));
    read += 1;
    if (!entry.key) continue;
    if (entry.simplified_form_of) {
      candidates += 1;
    } else {
      others += 1;
      if ((others - 1) % step !== 0) continue;
      controls += 1;
    }
    const target = equivalentTraditionalTarget(entry);
    const to = target && target !== entry.key ? target : null;
    if (to) moved += 1;
    answers[entry.key] = to;
  }
}
// What each field of the answers is made from, which the fixture carries
// (kiokun-oracle/NOTICE.md; the integrator's ruling of 2026-10-10).
const provenance = {
  'answers': "written forms alone, and the word each moves to, from the entries' structure (kiokun-data's builder)",
  'the sample': "read from the repository's sample (examples/kiokun/data/han-1char-3, its MANIFEST), which holds cleared fields alone; see NOTICE.md",
};
writeFileSync(
  out,
  JSON.stringify(
    { stride: step, read, candidates, controls, answered: Object.keys(answers).length, moved, named: {}, provenance, answers },
    null,
    1,
  ),
);
console.log(
  `stride ${step}: read ${read} one-character entries; ${candidates} candidates, ${controls} controls; ${moved} move`,
);

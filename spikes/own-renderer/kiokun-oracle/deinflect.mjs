// kiokun.com's own resolution of an inflected word, run as a black box: the
// oracle the rewrite's deinflection is held to (track `kiokun`, `just
// e14-kiokun-deinflection`; the integrator's rulings of 2026-10-11).
//
// usage: node --experimental-strip-types deinflect.mjs LIB DATA STRIDE OUT
//   LIB     a directory holding copies of kiokun.com's
//           `src/lib/utils/search-navigation.ts`, `deinflect.ts`,
//           `korean-deinflect.ts`, `src/routes/api/lookup-reading/+server.ts`
//           (as `lookup.ts`) and `migrations/0005_search_index_with_jyutping.sql`
//           (as `schema.sql`), and kiokun-data's `output_search_index.csv` (as
//           `index.csv`, or the repository's sample of it), each copied by
//           the recipe from its commit at HEAD (`scripts/kiokun_app_source.py`).
//           **Never committed**: `deinflect.ts` is adapted from 10ten-ja-reader,
//           GPL-3.0, and is run here, never read or transcribed.
//   DATA    a directory of kiokun's entries in its build's layout: the whole
//           `output_dictionary`, or the repository's deinflection sample
//   STRIDE  every how manyth file of each subdirectory gives the words whose
//           inflected forms are asked, in name order
//   OUT     where the JSON goes: { stride, read, words, asked, named,
//           provenance, answers }, `answers` mapping each inflected form to
//           kiokun.com's answer, { from, how, primary, alternatives }, primary
//           null where it finds none
//
// **The inputs** are the rewrite's own: for each entry read whose key is a
// Japanese word's first written form, of a part of speech the rewrite's
// conjugator covers (`inflect.mjs`), each inflected form it makes that has
// no file of its own (kiokun.com's word page deinflects only on a 404).
//
// **The resolution** is kiokun.com's `findWordsWithDeinflection`, taken from
// `search-navigation.ts` by name with the functions it calls, as written, and
// importing `deinflect.ts` and `korean-deinflect.ts` as they are. Its two
// reaches outside are given stand-ins: `getDictionaryUrl` names an entry's
// file by kiokun's shard rule (`h = h*31 + code point`, the subdirectory `h &
// 0xFF`, the name as `create_safe_filename` writes it), and the `fetch` it is
// given answers that file from DATA (404 where there is none) and
// `/api/lookup-reading` with kiokun.com's own endpoint over the index
// (index.mjs, d1.mjs, kit.mjs). `dev` is false. Any other import stops the
// run.
//
// **Named differences**: none here; the rewrite's test names them.
import { existsSync, readFileSync, readdirSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { pathToFileURL } from "node:url";
import { inflateRawSync } from "node:zlib";
import { d1Of } from "./d1.mjs";
import { indexed } from "./index.mjs";
import { prepared } from "./kit.mjs";
import { covered, inflections } from "./inflect.mjs";

const [, , lib, data, stride, out] = process.argv;
const step = Number(stride);

for (const file of ["deinflect.ts", "korean-deinflect.ts"]) {
  const source = readFileSync(join(lib, file), "utf8");
  for (const [, spec] of source.matchAll(/^import\s+(?!type\b)[^'"]*['"]([^'"]+)['"]/gm)) {
    throw new Error(`${file} imports ${spec}, which the harness does not copy`);
  }
}

// The function `name` as `source` writes it: from `function name(` to the
// brace that closes its body, the body's brace the first at a line's end.
function taken(source, name) {
  const heads = [...source.matchAll(new RegExp(`(?:^|\\n)\\s*(?:export\\s+)?(?:async\\s+)?function ${name}\\s*[(<]`, "g"))];
  if (heads.length !== 1) throw new Error(`search-navigation.ts: function ${name} found ${heads.length} times`);
  // From the head's first word: `export`, `async` or `function`.
  const start = heads[0].index + heads[0][0].search(/\S/);
  const body = source.slice(start).search(/\{[ \t]*\r?\n/);
  if (body < 0) throw new Error(`search-navigation.ts: function ${name} has no body`);
  let depth = 0;
  for (let i = start + body; i < source.length; i += 1) {
    if (source[i] === "{") depth += 1;
    if (source[i] === "}") {
      depth -= 1;
      if (depth === 0) return source.slice(start, i + 1).replace(/^export\s+/, "");
    }
  }
  throw new Error(`search-navigation.ts: function ${name} does not close`);
}

const navigation = readFileSync(join(lib, "search-navigation.ts"), "utf8");
const names = [
  "isHiraganaOnly",
  "getEntryWordType",
  "validatePosMatch",
  "hasKoreanVerbOrAdjective",
  "hasKoreanContent",
  "findKanjiFormsFromReading",
  "fetchDictionaryEntry",
  "fetchDictionaryBytes",
  "findWordsWithDeinflection",
];
writeFileSync(
  join(lib, "resolution.ts"),
  [
    "// Taken from kiokun.com's search-navigation.ts by the harness, each function as written.",
    "import { deinflect, formatReasonChains, WordType, posToWordType } from './deinflect.ts';",
    "import { koreanDeinflect, formatKoreanReasons, isHangulOnly } from './korean-deinflect.ts';",
    "const dev = false;",
    "let dictionaryUrlOf: (word: string) => string = () => '';",
    "export function setDictionaryUrl(f: (word: string) => string): void { dictionaryUrlOf = f; }",
    "async function getDictionaryUrl(word: string, _dev: boolean, _fetch: unknown, _options?: unknown): Promise<string> { return dictionaryUrlOf(word); }",
    ...names.map((n) => taken(navigation, n)),
    "export { findWordsWithDeinflection };",
    "",
  ].join("\n\n"),
);

// kiokun's shard rule and file names (kiokun-data `src/main.rs`).
function placeOf(word) {
  let h = 0;
  for (const c of word) h = (Math.imul(h, 31) + c.codePointAt(0)) >>> 0;
  const sub = (h & 0xff).toString(16).padStart(2, "0");
  const file = [...word].map((c) => (/[\/\\:*?"<>|]/.test(c) || c.codePointAt(0) <= 31 || (c.codePointAt(0) >= 127 && c.codePointAt(0) <= 159) ? "_" : c)).join("");
  return `${sub}/${file}.json.deflate`;
}

prepared(lib, ["lookup.ts"], {});
const { loaded } = indexed(lib);
const platform = { env: { DB: d1Of(loaded(true)) } };
const { GET: lookup } = await import(pathToFileURL(join(lib, "lookup.ts")).href);
const resolution = await import(pathToFileURL(join(lib, "resolution.ts")).href);
resolution.setDictionaryUrl((word) => `/dict/${placeOf(word)}`);

async function fetchStandIn(path) {
  const url = new URL(path, "http://kiokun");
  if (url.pathname.startsWith("/dict/")) {
    const file = join(data, decodeURIComponent(url.pathname.slice("/dict/".length)));
    return existsSync(file) ? new Response(readFileSync(file)) : new Response(null, { status: 404 });
  }
  if (url.pathname === "/api/lookup-reading") {
    try {
      return await lookup({ url, platform });
    } catch (e) {
      return new Response(null, { status: e?.status || 500 });
    }
  }
  return new Response(null, { status: 404 });
}

// The words asked: each entry read whose key is a Japanese word's first
// written form, with the first covered part of speech of its senses.
const words = [];
let read = 0;
for (const sub of readdirSync(data).filter((d) => /^[0-9a-f]{2}$/.test(d)).sort()) {
  const files = readdirSync(join(data, sub)).filter((f) => f.endsWith(".json.deflate")).sort();
  for (let i = 0; i < files.length; i += step) {
    const entry = JSON.parse(inflateRawSync(readFileSync(join(data, sub, files[i]))).toString("utf8"));
    read += 1;
    if (entry.redirect || !entry.key) continue;
    for (const w of entry.japanese_words || []) {
      const first = (w.kanji?.[0] || w.kana?.[0])?.text;
      if (first !== entry.key) continue;
      const pos = (w.sense || []).flatMap((s) => s.partOfSpeech || []).find(covered);
      if (pos) {
        words.push([entry.key, pos]);
        break;
      }
    }
  }
}

const answers = {};
for (const [word, pos] of words) {
  for (const [form, how] of inflections(word, pos)) {
    if (answers[form] || existsSync(join(data, placeOf(form)))) continue;
    const found = await resolution.findWordsWithDeinflection(form, fetchStandIn);
    answers[form] = {
      from: word,
      pos,
      how,
      primary: found?.primary?.dictionaryForm ?? null,
      alternatives: (found?.alternatives || []).map((a) => a.dictionaryForm),
    };
  }
}

// What each field of the answers is made from (kiokun-oracle/NOTICE.md).
const provenance = {
  "from, primary, alternatives": "JMdict's written forms (EDRDG, CC BY-SA 4.0, through jmdict-simplified 3.6.1), as kiokun-data's builder files them",
  "the inflected forms, how": "the rewrite's own conjugator (inflect.mjs), from the grammar the deinflection ADR cites",
};
writeFileSync(
  out,
  JSON.stringify({ stride: step, read, words: words.length, asked: Object.keys(answers).length, answered: Object.keys(answers).length, named: {}, provenance, answers }, null, 1),
);
const none = Object.values(answers).filter((a) => a.primary === null).length;
console.log(`stride ${step}: read ${read} entries; ${words.length} words; ${Object.keys(answers).length} forms asked; ${none} found nothing`);

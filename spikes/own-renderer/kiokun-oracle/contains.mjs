// kiokun.com's own Contains and Appears in, run over a sample of kiokun's
// entries: the oracle the rewrite's two sections are held to (track
// `kiokun`, `just e14-kiokun-contains`; the integrator's ruling of
// 2026-10-09 on oracles).
//
// usage: node --experimental-strip-types contains.mjs LIB APP DATA STRIDE OUT [TRIM]
//   LIB     a directory holding copies of kiokun.com's `src/lib/contains-order.ts`,
//           `appears-in-order.ts`, `character-support.ts`,
//           `word-character-learning.ts`, `Contains.svelte` and
//           `routes/[word]/+page.svelte` (as `page.svelte`), copied by
//           the recipe from KIOKUN_APP's commit at HEAD each run
//           (`scripts/kiokun_app_source.py`; never committed here: the
//           owner's source stays in kiokun-data)
//   APP     a directory holding the app's `static/game_data/` component
//           glosses, taxonomy and uses, which kiokun.com's character
//           support reads, copied from the same commit
//   DATA    a directory of kiokun's entries in its build's layout: the whole
//           `output_dictionary`, or the repository's sample
//   STRIDE  every how manyth file of each subdirectory, in name order
//   OUT     where the JSON goes: { stride, read, answered, named, answers },
//           `answers` mapping each word to its Contains cards and its three
//           Appears in columns
//   TRIM    `HEAD,TAIL` for the CI fixture: each list kept as its length and
//           its first HEAD and last TAIL items, each with its place (`at`),
//           so the repository holds the head of each list, where its order
//           shows, and the tail, where unranked words and names go, without
//           the whole lists' dictionary text (the integrator's ruling of
//           2026-10-10). The output says so in `trimmed`.
//
// Only entries kiokun.com's loader shows as they are, as the head's oracle
// takes them: no stub, and no character with simplified variants or a form
// it is the simplified form of (whose page merges an equivalent form's
// data, and reads another entry's character data).
//
// The page's own functions are not modules: `sortContainsWords` and
// `buildContainsWordForms` are in the page's script, the cards' in
// `Contains.svelte`'s, and the support characters' are not exported. Each is
// taken from kiokun.com's source as written, by its name, into a module of
// its own; a name found other than once, or a module importing anything
// the harness does not copy, stops the run. Nothing is stubbed (the
// ruling's fifth point). `Contains.svelte`'s functions read the component's
// `charGlosses` prop, which the module holds and the harness sets for each
// word, as kiokun.com's page passes it.
//
// **Named differences**: none.
import { readFileSync, readdirSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { pathToFileURL } from "node:url";
import { inflateRawSync } from "node:zlib";

const [, , lib, app, data, stride, out, trim] = process.argv;
const [head, tail] = trim ? trim.split(",").map(Number) : [null, null];

// A list as the output holds it: whole, or trimmed to its length and its
// head and tail, each item with its place.
function kept(items) {
  if (head === null) return items;
  return {
    length: items.length,
    kept: items
      .map((item, at) => ({ at, ...item }))
      .filter(({ at }) => at < head || at >= items.length - tail),
  };
}

for (const file of ["contains-order.ts", "appears-in-order.ts", "character-support.ts"]) {
  const source = readFileSync(join(lib, file), "utf8");
  for (const [, spec] of source.matchAll(/^import\s+(?!type\b)[^'"]*['"]([^'"]+)['"]/gm)) {
    throw new Error(`${file} imports ${spec}, which the harness does not copy`);
  }
}

// The function `name` as `source` writes it: from `function name(` to the
// brace that closes its body, the body's brace the first at a line's end.
function taken(source, file, name) {
  const heads = [...source.matchAll(new RegExp(`(?:^|\\n)\\s*(?:export\\s+)?(?:async\\s+)?function ${name}\\s*[(<]`, "g"))];
  if (heads.length !== 1) throw new Error(`${file}: function ${name} found ${heads.length} times`);
  const start = source.indexOf("function", heads[0].index);
  const body = source.slice(start).search(/\{[ \t]*\r?\n/);
  if (body < 0) throw new Error(`${file}: function ${name} has no body`);
  let depth = 0;
  for (let i = start + body; i < source.length; i += 1) {
    if (source[i] === "{") depth += 1;
    if (source[i] === "}") {
      depth -= 1;
      if (depth === 0) return source.slice(start, i + 1);
    }
  }
  throw new Error(`${file}: function ${name} does not close`);
}

const page = readFileSync(join(lib, "page.svelte"), "utf8");
const contains = readFileSync(join(lib, "Contains.svelte"), "utf8");
const learning = readFileSync(join(lib, "word-character-learning.ts"), "utf8");
const parts = [
  ...["sortContainsWords", "buildContainsWordForms"].map((n) => taken(page, "+page.svelte", n)),
  ...["buildContainsCards", "normalizedForms", "unique", "displayText", "displayDefinition", "cleanGloss"].map((n) =>
    taken(contains, "Contains.svelte", n),
  ),
  ...[
    "collectSupportKeys",
    "collectMnemonicCards",
    "addGlyphs",
    "addSupportChar",
    "addUsageTarget",
    "addSupportComponent",
    "addComponents",
    "addMnemonicSupport",
    "parseIdsComponents",
    "componentChar",
  ].map((n) => taken(learning, "word-character-learning.ts", n)),
];
writeFileSync(
  join(lib, "taken.ts"),
  [
    "// Taken from kiokun.com's source by the harness, each function as written.",
    "let charGlosses: Record<string, string> = {};",
    "export function setGlosses(glosses: Record<string, string>): void { charGlosses = glosses; }",
    ...parts,
    "export { sortContainsWords, buildContainsWordForms, buildContainsCards, displayText, displayDefinition, collectSupportKeys, collectMnemonicCards };",
    "",
  ].join("\n\n"),
);

const load = (file) => import(pathToFileURL(join(lib, file)).href);
const { orderContainsWords } = await load("contains-order.ts");
const { rankAppearsIn } = await load("appears-in-order.ts");
const { buildCharacterSupportData } = await load("character-support.ts");
const taken_ = await load("taken.ts");

const game = (file) => JSON.parse(readFileSync(join(app, "static/game_data", file), "utf8"));
const source = {
  charGlosses: game("component_glosses.json"),
  charTaxonomy: game("char_taxonomy.json"),
  componentUses: game("component_uses.json"),
};
const step = Number(stride);

// Each card as the page writes it (`Contains.svelte:146-195`), every
// language shown.
function card(c) {
  return {
    text: taken_.displayText(c),
    p: c.p || "",
    ct: c.ct || "",
    jp: c.jp || "",
    jo: c.jo && c.jo !== c.jp ? c.jo : "",
    kr: c.kr || "",
    definition: taken_.displayDefinition(c),
  };
}

const answers = {};
let read = 0;
for (const sub of readdirSync(data).filter((d) => /^[0-9a-f]{2}$/.test(d)).sort()) {
  const files = readdirSync(join(data, sub)).filter((f) => f.endsWith(".json.deflate")).sort();
  for (let i = 0; i < files.length; i += step) {
    const entry = JSON.parse(inflateRawSync(readFileSync(join(data, sub, files[i]))).toString("utf8"));
    read += 1;
    const merges = entry.chinese_char?.simpVariants?.length || entry.simplified_form_of;
    if (entry.redirect || merges || !entry.key) continue;
    const word = entry.key;
    const cards = taken_.collectMnemonicCards(entry, null);
    const keys = taken_.collectSupportKeys(word, entry, null, null, cards);
    taken_.setGlosses(buildCharacterSupportData(source, keys).charGlosses);
    const forms = taken_.buildContainsWordForms(entry, word);
    const ordered = orderContainsWords(taken_.sortContainsWords(entry.contains || [], word), forms);
    answers[word] = {
      contains: kept(taken_.buildContainsCards(ordered, forms).map(card)),
      chinese: kept(rankAppearsIn(entry.contained_in_chinese, "chinese").map((p) => ({
        w: p.w,
        reading: p.p || "",
        ct: p.ct || "",
        d: p.d || "",
      }))),
      japanese: kept(rankAppearsIn(entry.contained_in_japanese, "japanese").map((p) => ({
        w: p.w,
        reading: p.jp || p.p || "",
        c: Boolean(p.c),
        d: p.d || "",
      }))),
      korean: kept(rankAppearsIn(entry.contained_in_korean, "korean").map((p) => ({
        w: p.w,
        reading: p.kr || p.p || "",
        d: p.d || "",
      }))),
    };
  }
}
const trimmed =
  head === null ? null : `each list's length, and its first ${head} and last ${tail} items, each with its place`;
writeFileSync(
  out,
  JSON.stringify({ stride: step, read, answered: Object.keys(answers).length, named: {}, trimmed, answers }, null, 1),
);
console.log(`stride ${step}: read ${read} entries; ${Object.keys(answers).length} answered`);

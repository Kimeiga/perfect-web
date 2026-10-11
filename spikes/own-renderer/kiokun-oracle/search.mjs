// kiokun.com's own search, run over its own index: the oracle the rewrite's
// search page is held to (track `kiokun`, `just e14-kiokun-search`; the
// integrator's rulings of 2026-10-09 on oracles and of 2026-10-10 on search).
//
// usage: node --experimental-strip-types search.mjs LIB ALIASES STRIDE OUT [QUERIES]
//   LIB      a directory holding copies of kiokun.com's
//            `src/routes/api/search/+server.ts` (as `server.ts`),
//            `src/lib/search-aliases.ts`, `src/lib/search-aliases-core.ts` and
//            `migrations/0005_search_index_with_jyutping.sql` (as
//            `schema.sql`), and kiokun-data's `output_search_index.csv` (as
//            `index.csv`, or the repository's sample of it), each copied by
//            the recipe from its commit at HEAD (`scripts/kiokun_app_source.py`;
//            never committed here)
//   ALIASES  the aliases `search-aliases.ts` imports: kiokun.com's own
//            (`src/lib/generated/search-aliases.json` at HEAD), or the
//            repository's hand-made ones (`examples/kiokun/data/search-aliases-sample.json`)
//   STRIDE   every how manyth row of the CSV gives the queries
//   OUT      where the JSON goes: { stride, rows, queries, named, failed,
//            loadings?, provenance, answers }, `answers` mapping each query to
//            kiokun.com's response for `/api/search?q=…&limit=60` (what its
//            search page asks), or to the error it answers, and `failed`
//            listing the queries answered with an error
//   QUERIES  a JSON array of more queries, asked after the sample's: the
//            hand-made cases' answers
//
// **The index** is migration 0005's table, filled with the CSV's rows, each
// field as text but `is_common` an integer, as `output_search_index.sql`
// writes it (the step's ADR names the assumption). The header is not a row.
// With KIOKUN_BOTH_LOADINGS set, the queries are asked again over the index
// with `is_common` as text, and `loadings` counts the answers that differ.
//
// **The queries**, taken from every STRIDE-th row in the CSV's order, each
// once: the row's word (a CJK query, or as typed); its romanization
// (`reading_search`); a Japanese row's kana reading; and the first word of
// three letters or more of its definition.
//
// **The handler** is kiokun.com's own, imported with its imports resolved to
// what the harness copied: `$lib/search-aliases`, `$lib/search-aliases-core`
// and the aliases. SvelteKit's `json` and `error` are a stated stand-in: `json`
// answers a JSON `Response`, `error` throws its status and message, which is
// all the handler asks of them. `platform.env.DB` is d1.mjs, D1's binding over
// `node:sqlite`, with a test of its own (d1.test.mjs). Any other import stops
// the run: nothing else is stubbed.
//
// **Named differences**: none here. A query kiokun.com answers with an error
// (FTS5 reading it as syntax) is kept as that error; the rewrite answers it
// (the search ADR's Difference 27), and its test counts each by name.
import { copyFileSync, readFileSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { pathToFileURL } from "node:url";
import { DatabaseSync } from "node:sqlite";
import { d1Of } from "./d1.mjs";

const [, , lib, aliases, stride, out, more] = process.argv;
const step = Number(stride);

copyFileSync(aliases, join(lib, "search-aliases.json"));
const resolved = {
  "@sveltejs/kit": "./kit.ts",
  "$lib/search-aliases": "./search-aliases.ts",
  "$lib/search-aliases-core": "./search-aliases-core.ts",
  "$lib/generated/search-aliases.json": "./search-aliases.json",
};
for (const file of ["server.ts", "search-aliases.ts", "search-aliases-core.ts"]) {
  let source = readFileSync(join(lib, file), "utf8");
  source = source.replace(/^(import\s+(?:type\s+)?[^'"]*from\s+)['"]([^'"]+)['"];?$/gm, (line, head, spec) => {
    const to = resolved[spec];
    if (!to) throw new Error(`${file} imports ${spec}, which the harness does not copy`);
    return to.endsWith(".json") ? `${head}'${to}' with { type: 'json' };` : `${head}'${to}';`;
  });
  writeFileSync(join(lib, file), source);
}
writeFileSync(
  join(lib, "kit.ts"),
  `export function json(data: unknown): Response {
  return new Response(JSON.stringify(data), { headers: { "content-type": "application/json" } });
}
export class HttpError {
  status: number;
  body: { message: string };
  constructor(status: number, body: { message: string }) { this.status = status; this.body = body; }
}
export function error(status: number, message: string): never { throw new HttpError(status, { message }); }
export type RequestEvent = unknown;
`,
);

// The builder's CSV records: a field holding a comma, a quote or a line
// break quoted, a quote inside doubled (its `escape_csv_field`).
function records(text) {
  const all = [];
  let row = [];
  let field = "";
  let quoted = false;
  for (let i = 0; i < text.length; i += 1) {
    const c = text[i];
    if (quoted) {
      if (c === '"' && text[i + 1] === '"') { field += '"'; i += 1; }
      else if (c === '"') quoted = false;
      else field += c;
    } else if (c === '"') quoted = true;
    else if (c === ",") { row.push(field); field = ""; }
    else if (c === "\n") { row.push(field); all.push(row); row = []; field = ""; }
    else if (c !== "\r") field += c;
  }
  if (field || row.length) { row.push(field); all.push(row); }
  return all;
}

const all = records(readFileSync(join(lib, "index.csv"), "utf8"));
const header = all.shift();
const common = header.indexOf("is_common");

// The index, `is_common` an integer (`asInteger`) or as the CSV's text, as
// the shell's `.import` stores it (the step's ADR, Found 2).
function loaded(asInteger) {
  const db = new DatabaseSync(":memory:");
  db.exec(readFileSync(join(lib, "schema.sql"), "utf8"));
  const insert = db.prepare(
    `INSERT INTO dictionary_search (${header.join(", ")}) VALUES (${header.map(() => "?").join(", ")})`,
  );
  db.exec("BEGIN");
  for (const r of all) {
    insert.run(...r.map((v, i) => (asInteger && i === common && /^-?\d+$/.test(v) ? Number(v) : v)));
  }
  db.exec("COMMIT");
  return db;
}
const db = loaded(true);

const at = Object.fromEntries(header.map((name, i) => [name, i]));
const queries = [];
const add = (q) => {
  if (q && q.trim() && !queries.includes(q)) queries.push(q);
};
for (let i = 0; i < all.length; i += step) {
  const r = all[i];
  add(r[at.word]);
  add(r[at.reading_search]);
  if (r[at.language] === "japanese") add(r[at.pronunciation]);
  add((r[at.definition].match(/[A-Za-z]{3,}/) || [])[0]);
}

for (const q of more ? JSON.parse(more) : []) add(q);
const { GET } = await import(pathToFileURL(join(lib, "server.ts")).href);
async function answered(over) {
  const platform = { env: { DB: d1Of(over) } };
  const answers = {};
  for (const q of queries) {
    const url = new URL(`http://kiokun/api/search?q=${encodeURIComponent(q)}&limit=60`);
    try {
      const response = await GET({ url, platform });
      answers[q] = { results: (await response.json()).results };
    } catch (e) {
      answers[q] = { error: e?.status ? `${e.status}: ${e.body?.message}` : String(e) };
    }
  }
  return answers;
}
const answers = await answered(db);
const failed = queries.filter((q) => answers[q].error);
console.log(`failed searches: ${failed.length}${failed.length ? ` (${failed.map((q) => JSON.stringify(q)).join(", ")})` : ""}`);

// **Both loadings, once** (the integrator's ruling of 2026-10-11, 3), where
// KIOKUN_BOTH_LOADINGS is set: the same queries over the index with
// `is_common` loaded as text, and how many answers differ from the
// integer loading's.
let loadings;
if (process.env.KIOKUN_BOTH_LOADINGS) {
  const text = await answered(loaded(false));
  const differ = queries.filter((q) => JSON.stringify(text[q]) !== JSON.stringify(answers[q]));
  const commonOf = (a) => (a.results || []).filter((r) => r.is_common).length;
  const results = (a) => (a.results || []).length;
  const sum = (as, f) => queries.reduce((n, q) => n + f(as[q]), 0);
  loadings = {
    differ: differ.length,
    of: queries.length,
    common: { integer: sum(answers, commonOf), text: sum(text, commonOf), results: sum(answers, results) },
    first: differ.slice(0, 5),
  };
  console.log(
    `is_common loaded as text: ${differ.length} of ${queries.length} answers differ from the integer loading's; ` +
      `hits marked common: ${loadings.common.integer} as integers, ${loadings.common.text} as text, of ${loadings.common.results}` +
      (differ.length ? `; first: ${differ.slice(0, 5).map((q) => JSON.stringify(q)).join(", ")}` : ""),
  );
}
// What each field of the answers is made from, which the fixture carries
// (kiokun-oracle/NOTICE.md; the integrator's ruling of 2026-10-10).
const provenance = {
  results: "each a group of the search index's rows: CC-CEDICT's and Unihan's definitions (through Dong Chinese's export, kiokun-data data/chinese_dictionary_word_2025-06-25.jsonl, items tagged cedict or unicode), JMdict's glosses (jmdict-simplified 3.6.1), with the builder's romanizations",
  "the index": "kiokun-data's output_search_index.csv at a47956fb, or the repository's sample of it (examples/kiokun/data/search-sample.csv), with the hand-made aliases where the sample is read",
};
writeFileSync(
  out,
  JSON.stringify(
    { stride: step, rows: all.length, queries: queries.length, answered: queries.length, named: {}, failed, ...(loadings ? { loadings } : {}), provenance, answers },
    null,
    1,
  ),
);
console.log(`stride ${step}: ${all.length} rows; ${queries.length} queries`);

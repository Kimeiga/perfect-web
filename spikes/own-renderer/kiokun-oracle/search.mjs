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
//   OUT      where the JSON goes: { stride, rows, queries, named, provenance,
//            answers }, `answers` mapping each query to kiokun.com's response
//            for `/api/search?q=…&limit=60` (what its search page asks), or to
//            the error it answers
//   QUERIES  a JSON array of more queries, asked after the sample's: the
//            hand-made cases' answers
//
// **The index** is migration 0005's table, filled with the CSV's rows, each
// field as text but `is_common` an integer, as `output_search_index.sql`
// writes it (the step's ADR names the assumption). The header is not a row.
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
// **Named differences**: none.
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

const db = new DatabaseSync(":memory:");
db.exec(readFileSync(join(lib, "schema.sql"), "utf8"));
const all = records(readFileSync(join(lib, "index.csv"), "utf8"));
const header = all.shift();
const common = header.indexOf("is_common");
const insert = db.prepare(
  `INSERT INTO dictionary_search (${header.join(", ")}) VALUES (${header.map(() => "?").join(", ")})`,
);
db.exec("BEGIN");
for (const r of all) {
  insert.run(...r.map((v, i) => (i === common && /^-?\d+$/.test(v) ? Number(v) : v)));
}
db.exec("COMMIT");

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
const platform = { env: { DB: d1Of(db) } };
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
// What each field of the answers is made from, which the fixture carries
// (kiokun-oracle/NOTICE.md; the integrator's ruling of 2026-10-10).
const provenance = {
  results: "each a group of the search index's rows: CC-CEDICT's and Unihan's definitions (through Dong Chinese's export, kiokun-data data/chinese_dictionary_word_2025-06-25.jsonl, items tagged cedict or unicode), JMdict's glosses (jmdict-simplified 3.6.1), with the builder's romanizations",
  "the index": "kiokun-data's output_search_index.csv at a47956fb, or the repository's sample of it (examples/kiokun/data/search-sample.csv), with the hand-made aliases where the sample is read",
};
writeFileSync(
  out,
  JSON.stringify({ stride: step, rows: all.length, queries: queries.length, answered: queries.length, named: {}, provenance, answers }, null, 1),
);
console.log(`stride ${step}: ${all.length} rows; ${queries.length} queries`);

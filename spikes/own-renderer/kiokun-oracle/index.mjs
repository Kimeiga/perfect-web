// kiokun.com's search index, as its oracles load it (track `kiokun`): migration
// 0005's table (LIB/schema.sql), filled with the rows of the builder's CSV
// (LIB/index.csv). Shared by search.mjs and homophones.mjs.
import { readFileSync } from "node:fs";
import { join } from "node:path";
import { DatabaseSync } from "node:sqlite";

// The builder's CSV records: a field holding a comma, a quote or a line
// break quoted, a quote inside doubled (its `escape_csv_field`).
export function records(text) {
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

// LIB's index: its rows (`all`, the header not among them), its `header`,
// and `loaded(asInteger)`, a database of them, `is_common` an integer
// (`asInteger`, as `output_search_index.sql` writes it) or as the CSV's
// text, as the shell's `.import` stores it (the search ADR, Found 2).
export function indexed(lib) {
  const all = records(readFileSync(join(lib, "index.csv"), "utf8"));
  const header = all.shift();
  const common = header.indexOf("is_common");
  const schema = readFileSync(join(lib, "schema.sql"), "utf8");
  function loaded(asInteger) {
    const db = new DatabaseSync(":memory:");
    db.exec(schema);
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
  return { all, header, loaded };
}

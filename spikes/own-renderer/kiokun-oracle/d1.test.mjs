// The D1 stand-in's own test (the integrator's ruling of 2026-10-10, B4):
// an adapter bug must not pass as the oracle agreeing with the rewrite.
// Run by `node --test` (scripts/tests/test_kiokun_d1_adapter.py).
import { test } from "node:test";
import assert from "node:assert/strict";
import { DatabaseSync } from "node:sqlite";
import { d1Of } from "./d1.mjs";

function db() {
  const d = new DatabaseSync(":memory:");
  d.exec("CREATE VIRTUAL TABLE t USING fts5(word, definition, n, tokenize = 'porter unicode61')");
  const insert = d.prepare("INSERT INTO t (word, definition, n) VALUES (?, ?, ?)");
  insert.run("人", "person", 1);
  insert.run("人々", "people", 0);
  insert.run("水", "water", 1);
  return d;
}

test("all() resolves to D1's shape: results keyed by column, success, meta", async () => {
  const r = await d1Of(db()).prepare("SELECT word, n FROM t ORDER BY rowid").all();
  assert.equal(r.success, true);
  assert.deepEqual(r.results, [
    { word: "人", n: 1 },
    { word: "人々", n: 0 },
    { word: "水", n: 1 },
  ]);
  assert.equal(typeof r.meta, "object");
});

test("bind() binds the placeholders in order and returns the statement", async () => {
  const statement = d1Of(db()).prepare("SELECT word FROM t WHERE word = ? OR definition = ? ORDER BY rowid");
  assert.equal(statement.bind("水", "people"), statement);
  const r = await statement.all();
  assert.deepEqual(r.results.map((x) => x.word), ["人々", "水"]);
});

test("a MATCH and a LIMIT bind as the handler binds them, and agree with the database read directly", async () => {
  const d = db();
  const sql = "SELECT word FROM t WHERE t MATCH ? ORDER BY rowid LIMIT ?";
  const r = await d1Of(d).prepare(sql).bind('word : "人"*', 5).all();
  assert.deepEqual(
    r.results,
    d.prepare(sql).all('word : "人"*', 5).map((x) => ({ ...x })),
  );
  assert.deepEqual(r.results.map((x) => x.word), ["人", "人々"]);
  // Porter stemming: "persons" finds "person".
  const stemmed = await d1Of(d).prepare("SELECT word FROM t WHERE t MATCH ?").bind("persons").all();
  assert.deepEqual(stemmed.results.map((x) => x.word), ["人"]);
});

test("a statement the database refuses rejects, as D1's does", async () => {
  await assert.rejects(d1Of(db()).prepare("SELECT word FROM t WHERE t MATCH ?").bind('"').all());
});

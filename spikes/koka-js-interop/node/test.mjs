// spike: koka-js-interop — the executable evidence.
//
// Run: node --test spikes/koka-js-interop/node/
// (or: just spike-koka)
//
// Each test corresponds to a claim in charter §14 Milestone 0 task 8, or to a
// finding recorded in this spike's README.

import test from "node:test";
import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import { fileURLToPath } from "node:url";
import path from "node:path";

import * as decode from "./decode.mjs";
import { parseInterface, effectNames, assertKokaVersion } from "./kki.mjs";

const here = path.dirname(fileURLToPath(import.meta.url));
const build = path.join(here, "..", "build");

// The generated Koka modules, imported as ordinary ES modules. This import
// succeeding IS charter claim 2 ("generated JavaScript can be imported from Node
// as an ES module") — no adapter, no bundler, no transpile step.
const kk = await import(path.join(build, "kk_store.mjs"));
const kkTypes = await import(path.join(build, "std_core_types.mjs"));

// ---------------------------------------------------------------------------
// Claim 1 — Koka ADTs and effect handlers compile and run on Apple Silicon
// ---------------------------------------------------------------------------

test("generated module exports the domain constructors and predicates", () => {
  for (const name of [
    "QOk", "QErr", "is_qok", "is_qerr",
    "Draft", "Confirmed", "is_draft", "is_confirmed",
    "Store", "Menu_item", "Money_usd",
    "load_store_for_js", "load_subtotal_for_js", "calculate_subtotal",
  ]) {
    assert.ok(name in kk, `missing export: ${name}`);
  }
});

test("effect handlers run: a database-read + trace computation produces a value", () => {
  // load_store_for_js installs both handlers internally, so JS receives a plain
  // value rather than a suspended computation.
  const found = kk.load_store_for_js("store_47");
  assert.ok(kk.is_qok(found), "store_47 should be found");

  const missing = kk.load_store_for_js("store_99");
  assert.ok(kk.is_qerr(missing), "store_99 should not be found");
});

test("pure computation over an ADT list produces the expected total", () => {
  // Espresso 350 + Cortado 475 available; Cold Brew 525 unavailable and filtered.
  const subtotal = kk.load_subtotal_for_js("store_47");
  assert.ok(kk.is_qok(subtotal));
  assert.equal(Number(subtotal.value), 350 + 475);
});

// ---------------------------------------------------------------------------
// Claim 3 — maybe/result cross the boundary WITHOUT unchecked guessing
// ---------------------------------------------------------------------------

test("qresult decodes to an explicit JS-side Result", () => {
  const r = decode.decodeQResult(
    kk, "load_store_for_js", kk.load_store_for_js("store_47"),
    decode.decodeStore, decode.decodeStoreError,
  );
  assert.ok(r.ok, JSON.stringify(r));
  assert.equal(r.value.kind, "Ok");
  assert.deepEqual(r.value.value, { id: "store_47", name: "Blue Bottle" });
});

test("qresult error side decodes to a typed domain error, not a string", () => {
  const r = decode.decodeQResult(
    kk, "load_store_for_js", kk.load_store_for_js("store_99"),
    decode.decodeStore, decode.decodeStoreError,
  );
  assert.ok(r.ok);
  assert.equal(r.value.kind, "Err");
  assert.deepEqual(r.value.error, { kind: "StoreNotFound", id: "store_99" });
});

test("every order-state variant decodes; none falls through to a default", () => {
  const states = decode.decodeList(
    "sample_order_states",
    kk.sample_order_states(),
    (p, u) => decode.decodeOrderState(kk, p, u),
  );
  assert.ok(states.ok, JSON.stringify(states));
  assert.deepEqual(states.value.map((s) => s.kind), [
    "Draft", "Pricing", "Confirmed", "Delivered", "Cancelled",
  ]);
});

test("decoder REJECTS malformed input instead of guessing", () => {
  for (const bad of [null, undefined, 42, "QOk", [], {}, { _tag: 99 }, { value: 1 }]) {
    const r = decode.decodeQResult(kk, "$", bad, decode.decodeStore, decode.decodeStoreError);
    assert.equal(r.ok, false, `should have rejected: ${JSON.stringify(bad)}`);
    assert.equal(r.error.kind, "DecodeError");
    assert.ok(r.error.path.length > 0);
  }
});

test("decoder rejects a field of the wrong runtime type rather than coercing", () => {
  // A store whose `name` is a number. Charter §7.1: no implicit coercion.
  const r = decode.decodeStore("$", { id: "store_47", name: 12345 });
  assert.equal(r.ok, false);
  assert.equal(r.error.path, "$.name");
  assert.equal(r.error.expected, "string");
});

test("finding F-5: Koka's is_* predicates are NOT validators", () => {
  // This test exists to pin the hazard in place. If a future Koka release makes
  // these safe, this test fails and the README finding gets revisited.
  assert.equal(kkTypes.is_just(42), true, "is_just accepts a bare number");
  assert.equal(kkTypes.is_just("nonsense"), true, "is_just accepts a string");
  assert.throws(() => kk.is_qok(null), TypeError, "is_qok throws on null");
});

test("finding F-7: Nothing and Nil are BOTH null — runtime discrimination is impossible", () => {
  assert.equal(kkTypes.Nothing, null);
  assert.equal(kkTypes.Nil, null);
  assert.equal(kkTypes.is_nothing(kkTypes.Nil), true, "is_nothing says the empty list is Nothing");
  assert.equal(kkTypes.is_nil(kkTypes.Nothing), true, "is_nil says Nothing is an empty list");

  // The decoder copes only because the CALLER supplies the static type.
  assert.deepEqual(decode.decodeMaybe("$", null, decode.decodeString).value, { kind: "None" });
  assert.deepEqual(decode.decodeList("$", null, decode.decodeString).value, []);
});

test("finding F-4: single-field value structs are ERASED to their payload", () => {
  // Money_usd(350) is literally 350. There is no nominal type at runtime.
  assert.equal(kk.Money_usd(350), 350);
  assert.equal(kk.Store_id("store_47"), "store_47");
  // Therefore a decoder cannot verify the currency; it records that it could not.
  const m = decode.decodeMoneyUsd("$", kk.Money_usd(350));
  assert.ok(m.ok);
  assert.equal(m.value._unverifiedNominalType, true);
});

// ---------------------------------------------------------------------------
// Milestone 1 look-ahead — machine-readable effects without a fork
// ---------------------------------------------------------------------------

test("koka version matches what the .kki reader was verified against", () => {
  const kokaBin = process.env.KOKA_BIN || "koka";
  const version = execFileSync(kokaBin, ["--version"], { encoding: "utf8" }).split("\n")[0];
  assertKokaVersion(version);
});

test(".kki exposes inferred effect rows: pure functions have an EMPTY row", () => {
  const decls = parseInterface(path.join(build, "kk_store.kki"));
  assert.deepEqual(effectNames(decls.get("calculate-subtotal").signature), []);
  assert.deepEqual(effectNames(decls.get("available-items").signature), []);
  assert.deepEqual(effectNames(decls.get("describe-order").signature), []);
});

test(".kki exposes inferred effect rows: effectful functions NAME their effects", () => {
  const decls = parseInterface(path.join(build, "kk_store.kki"));
  assert.deepEqual(effectNames(decls.get("load-store").signature), [
    "database-read", "trace-effect",
  ]);
  assert.deepEqual(effectNames(decls.get("load-store-subtotal").signature), [
    "database-read", "trace-effect",
  ]);
  assert.deepEqual(effectNames(decls.get("main").signature), ["console"]);
});

test(".kki shows handlers DISCHARGE effects — the JS-facing surface is pure", () => {
  // This is the property that makes the boundary sound: an unhandled effect is a
  // suspended computation and could not be handed to JS as a value at all.
  const decls = parseInterface(path.join(build, "kk_store.kki"));
  assert.deepEqual(effectNames(decls.get("load-store-for-js").signature), []);
  assert.deepEqual(effectNames(decls.get("load-subtotal-for-js").signature), []);
});

test(".kki carries source spans usable for mapping diagnostics back to .pw", () => {
  const decls = parseInterface(path.join(build, "kk_store.kki"));
  const span = decls.get("calculate-subtotal").span;
  assert.equal(span.length, 4, "span is [line, col, line, col]");
  assert.ok(span[0] > 0 && span[1] > 0);
});

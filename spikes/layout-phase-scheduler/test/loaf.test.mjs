import test from "node:test";
import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import {
  serializeLongAnimationFrame,
  summarizeLayoutAttribution,
  installLayoutObserver,
} from "../public/loaf.js";

const frame = (scripts = []) => ({ startTime: 1, duration: 80, scripts });
const summarize = (...entries) => summarizeLayoutAttribution(entries.map(serializeLongAnimationFrame));

test("reads per-script attribution, never a similarly named frame property", () => {
  const entry = { ...frame([{ forcedStyleAndLayoutDuration: 2 }, { forcedStyleAndLayoutDuration: 3 }]),
    forcedStyleAndLayoutDuration: 999 };
  const result = summarize(entry);
  assert.equal(result.reportedForcedStyleAndLayoutDuration, 5);
  assert.equal(result.reportedScripts, 2);
  assert.equal(result.state, "observed");
});

test("a measured zero remains different from missing evidence", () => {
  const measured = summarize(frame([{ forcedStyleAndLayoutDuration: 0 }]));
  const missing = summarize(frame([{}]));
  assert.equal(measured.state, "observed");
  assert.equal(measured.reportedForcedStyleAndLayoutDuration, 0);
  assert.equal(missing.state, "unobserved");
  assert.equal(missing.reportedForcedStyleAndLayoutDuration, null);
});

test("zero long frames do not establish zero layout or unsupported attribution", () => {
  assert.equal(summarize().state, "unobserved");
  assert.equal(summarize().reportedForcedStyleAndLayoutDuration, null);
});

test("a frame with no attributed scripts is unobserved", () => {
  assert.equal(summarize(frame()).state, "unobserved");
  assert.equal(summarize(frame()).framesWithoutScripts, 1);
});

test("missing script list is preserved as unavailable, not an observed zero", () => {
  const entry = serializeLongAnimationFrame({ duration: 80 });
  assert.equal(entry.scriptsAvailable, false);
  assert.deepEqual(entry.scripts, []);
  assert.equal(summarizeLayoutAttribution([entry]).state, "unobserved");
});

test("partial attribution retains its subtotal and coverage gap", () => {
  const result = summarize(frame([{ forcedStyleAndLayoutDuration: 4 }, {}]), frame());
  assert.equal(result.state, "partial");
  assert.equal(result.reportedForcedStyleAndLayoutDuration, 4);
  assert.equal(result.missingScripts, 1);
  assert.equal(result.framesWithoutScripts, 1);
});

test("later frames can supply evidence missing from the first", () => {
  const result = summarize(frame([{}]), frame([{ forcedStyleAndLayoutDuration: 6 }]));
  assert.equal(result.reportedScripts, 1);
  assert.equal(result.state, "partial");
  assert.equal(result.reportedForcedStyleAndLayoutDuration, 6);
});

for (const invalid of [undefined, null, NaN, Infinity, -1, "0"]) {
  test(`invalid duration ${String(invalid)} is not coerced into a measurement`, () => {
    const result = summarize(frame([{ forcedStyleAndLayoutDuration: invalid }]));
    assert.equal(result.state, "unobserved");
    assert.equal(result.reportedForcedStyleAndLayoutDuration, null);
  });
}

test("recording keeps attribution metadata without retaining a browser window", () => {
  const input = frame([{ duration: 75, forcedStyleAndLayoutDuration: 20,
    sourceURL: "https://fixture.invalid/app.js", sourceFunctionName: "run", window: {} }]);
  const output = serializeLongAnimationFrame(input);
  assert.equal(output.scripts[0].sourceFunctionName, "run");
  assert.equal(output.scripts[0].duration, 75);
  assert.equal("window" in output.scripts[0], false);
  assert.notEqual(output.scripts, input.scripts);
});

test("malformed entry and summary input fail explicitly", () => {
  assert.throws(() => serializeLongAnimationFrame(null), TypeError);
  assert.throws(() => summarizeLayoutAttribution(null), TypeError);
});

test("unsupported observer is distinct from an empty observing buffer", () => {
  const target = {};
  assert.equal(installLayoutObserver(target), null);
  assert.equal(target.__loafObservation, "unsupported");
  assert.equal(target.__loafSupported, false);
});

test("observer integration uses the same serializer as the unit tests", () => {
  let callback;
  let options;
  class Observer {
    static supportedEntryTypes = ["long-animation-frame"];
    constructor(fn) { callback = fn; }
    observe(value) { options = value; }
    disconnect() {}
  }
  const target = { PerformanceObserver: Observer };
  assert.ok(installLayoutObserver(target) instanceof Observer);
  assert.deepEqual(options, { type: "long-animation-frame", buffered: true });
  callback({ getEntries: () => [frame([{ forcedStyleAndLayoutDuration: 11 }])] });
  assert.equal(summarizeLayoutAttribution(target.__loaf).reportedForcedStyleAndLayoutDuration, 11);
  assert.equal(target.__loafObservation, "observing");
});

test("observer installation failure cannot be reported as unsupported or clean", () => {
  let disconnected = false;
  class Observer {
    static supportedEntryTypes = ["long-animation-frame"];
    observe() { throw new Error("fixture refusal"); }
    disconnect() { disconnected = true; }
  }
  const target = { PerformanceObserver: Observer };
  assert.throws(() => installLayoutObserver(target), /fixture refusal/);
  assert.equal(target.__loafObservation, "failed");
  assert.equal(target.__loafError, "fixture refusal");
  assert.equal(disconnected, true);
});

test("both actual spike pages install the shared, tested instrumentation", async () => {
  for (const name of ["thrash", "phased"]) {
    const source = await readFile(new URL(`../public/${name}.html`, import.meta.url), "utf8");
    assert.match(source, /import \{ installLayoutObserver \} from "\.\/loaf\.js"/);
    assert.match(source, /installLayoutObserver\(window\)/);
    assert.doesNotMatch(source, /e\.forcedStyleAndLayoutDuration/);
  }
});

test("harness closes its server when browser startup fails", async () => {
  const { execFile } = await import("node:child_process");
  const { promisify } = await import("node:util");
  const { fileURLToPath } = await import("node:url");
  const harness = fileURLToPath(new URL("../measure.mjs", import.meta.url));
  await assert.rejects(promisify(execFile)(process.execPath, [harness], {
    env: { ...process.env, CHROME_PATH: "/nonexistent-pleris-test-browser", PORT: "0" },
    timeout: 4000,
  }), (error) => error.code === 1 && !error.killed && error.stderr.includes("ENOENT"));
});

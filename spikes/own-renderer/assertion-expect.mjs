// Native matcher provenance for the device mutation producer (ADR-0366).
// Public Playwright expect/extend APIs only. Normal assertions keep their
// native comparator, error, stack and retry behavior; only a cause is added.
import { expect as playwrightExpect } from "@playwright/test";

function proven(error, name, isNot) {
  const result = error instanceof Error && error.matcherResult;
  if (result && result.name === name && typeof result.pass === "boolean"
      && result.pass === isNot && /^[a-f0-9]{64}$/.test(process.env.PW_ASSERTION_NONCE ?? "")) {
    // TestError publicly preserves Error.cause. The completed expect step has
    // already been serialized; the reporter pairs its message/stack with this
    // final error. Preserve any existing cause beneath the proof marker.
    const cause = new Error(`PW_ASSERTION:${process.env.PW_ASSERTION_NONCE}`, { cause: error.cause });
    try { Object.defineProperty(error, "cause", { value: cause, configurable: true, writable: true }); }
    catch { /* An immutable error cannot carry proof and earns no kill. */ }
  }
  throw error;
}

function matchers(target, isNot = false) {
  return new Proxy(target, {
    get(object, name) {
      if (name === "not") return matchers(object.not, !isNot);
      // These producers use normal assertions and .not. Do not silently add
      // soft, promise or other assertion modes to this proof protocol.
      if (name === "resolves" || name === "rejects") throw new Error("unsupported assertion proof mode");
      const matcher = Reflect.get(object, name);
      if (typeof matcher !== "function") return matcher;
      return (...args) => {
        try {
          const result = Reflect.apply(matcher, object, args);
          return result && typeof result.then === "function"
            ? result.catch(error => proven(error, name, isNot)) : result;
        } catch (error) { return proven(error, name, isNot); }
      };
    },
  });
}

export function expect(...args) { return matchers(playwrightExpect(...args)); }

// The sole poll witness is expect.poll(callback).toBe(expected). Native poll
// discards matcherResult and creates caught failing child expect steps. This
// scoped public custom matcher creates one native assertion result instead.
// Only positive toBe is exposed: Object.is, finite positive timeout/intervals,
// and a completed callback value. Callback errors/pending work are not evidence.
const pollingExpect = playwrightExpect.extend({
  async toBePolled(callback, expected, options) {
    const { timeout, intervals } = options;
    const deadline = performance.now() + timeout;
    let actual, observed = false, attempt = 0;
    for (;;) {
      const remaining = deadline - performance.now();
      if (remaining <= 0) break;
      let timer;
      try {
        actual = await Promise.race([
          Promise.resolve().then(callback).catch(cause => {
            throw new Error("poll callback failed", { cause });
          }),
          new Promise((_, reject) => {
            timer = setTimeout(() => reject(new Error("poll callback did not complete before its deadline")), remaining);
          }),
        ]);
      } finally { clearTimeout(timer); }
      if (performance.now() >= deadline) throw new Error("poll callback completed after its deadline");
      observed = true;
      if (Object.is(actual, expected)) break;
      const delay = Math.min(intervals[Math.min(attempt++, intervals.length - 1)], deadline - performance.now());
      if (delay > 0) await new Promise(resolve => setTimeout(resolve, delay));
    }
    if (!observed) throw new Error("poll observed no completed callback");
    return { name: "toBePolled", pass: Object.is(actual, expected), actual, expected,
      message: () => `Expected: ${this.utils.printExpected(expected)}\nReceived: ${this.utils.printReceived(actual)}` };
  },
});

expect.poll = (callback, options = {}) => {
  if (typeof callback !== "function" || !options || typeof options !== "object"
      || Array.isArray(options) || Object.keys(options).some(key => !["timeout", "intervals", "message"].includes(key))) {
    throw new Error("unsupported polling assertion arguments");
  }
  const timeout = options.timeout ?? 5000;
  const intervals = options.intervals ?? [100, 250, 500, 1000];
  if (!Number.isFinite(timeout) || timeout <= 0 || !Array.isArray(intervals) || !intervals.length
      || intervals.some(value => !Number.isFinite(value) || value <= 0)
      || (options.message !== undefined && typeof options.message !== "string")) {
    throw new Error("polling assertion requires a finite positive deadline and intervals");
  }
  return { toBe: expected => matchers(pollingExpect(callback, options.message)).toBePolled(expected, { timeout, intervals }) };
};

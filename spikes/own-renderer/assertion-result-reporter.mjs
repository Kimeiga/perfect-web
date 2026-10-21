import { existsSync, renameSync, writeFileSync } from "node:fs";

// Public Playwright 1.58 Reporter/TestResult/TestStep APIs. Like host-plan's
// collector, publish only at onExit: reporter exceptions are otherwise swallowed.
const signature = (error) => JSON.stringify([error?.message, error?.stack]);
export function bodyAssertions(result) {
  const assertions = [];
  let other = false;
  const visit = (steps, inHook = false) => {
    for (const step of steps) {
      const hook = inHook || step.category === "hook";
      if (step.error) {
        if (hook) other = true;
        else if (step.category === "expect") assertions.push(signature(step.error));
      }
      visit(step.steps ?? [], hook);
    }
  };
  visit(result.steps);
  const errors = result.errors.map(signature);
  return !other && errors.length > 0 && errors.every(e => assertions.includes(e))
    && assertions.every(e => errors.includes(e));
}

export default class AssertionResultReporter {
  constructor() {
    this.report = process.env.PW_ASSERTION_REPORT;
    this.nonce = process.env.PW_ASSERTION_NONCE;
    this.problem = !this.report || !/^[a-f0-9]{64}$/.test(this.nonce ?? "");
    this.begins = 0;
    this.ends = 0;
    this.expected = new Set();
    this.started = new Set();
    this.results = new Map();
  }
  printsToStdio() { return false; }
  onBegin(_config, suite) {
    this.begins++;
    const tests = suite.allTests();
    this.expected = new Set(tests.map(t => t.id));
    if (this.begins !== 1 || !tests.length || this.expected.size !== tests.length
        || tests.some(t => t.expectedStatus !== "passed")) this.problem = true;
  }
  onError() { this.problem = true; }
  onTestBegin(test, result) {
    if (!this.expected.has(test.id) || this.started.has(test.id) || result.retry !== 0) this.problem = true;
    this.started.add(test.id);
  }
  onTestEnd(test, result) {
    if (!this.started.has(test.id) || this.results.has(test.id)
        || test.expectedStatus !== "passed" || !["passed", "failed"].includes(result.status)) this.problem = true;
    if (result.status === "failed" && !bodyAssertions(result)) this.problem = true;
    if (result.status === "passed" && result.errors.length) this.problem = true;
    this.results.set(test.id, result.status);
  }
  onEnd(result) {
    this.ends++;
    const failed = [...this.results.values()].includes("failed");
    if (this.ends !== 1 || result.status !== (failed ? "failed" : "passed")
        || this.results.size !== this.expected.size || this.started.size !== this.expected.size) this.problem = true;
  }
  onExit() {
    try {
      if (this.problem || this.begins !== 1 || this.ends !== 1) return;
      if (existsSync(this.report)) throw new Error("receipt already exists");
      const receipt = { schema: 1, complete: true, nonce: this.nonce,
        tests: [...this.results].sort(([a], [b]) => a.localeCompare(b)),
        inventory: [...this.expected].sort() };
      writeFileSync(`${this.report}.part`, JSON.stringify(receipt), { flag: "wx", mode: 0o600 });
      renameSync(`${this.report}.part`, this.report);
    } catch (error) {
      this.problem = true;
      console.error(`assertion receipt refused: ${error.message}`);
    }
  }
}

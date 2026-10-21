// Public Reporter callback contract fixture, no application/browser required.
import Reporter from "./assertion-result-reporter.mjs";
const mode = process.argv[2];
const test = { id: "one", expectedStatus: "passed" };
const error = { message: "expect(1).toBe(2)", stack: "assertion fixture" };
error.cause = { message: `Error: PW_ASSERTION:${process.env.PW_ASSERTION_NONCE}` };
const assertion = { category: "expect", error, steps: [] };
const result = { retry: 0, status: "passed", errors: [], steps: [] };
const reporter = new Reporter();
reporter.onBegin({}, { allTests: () => [test] });
if (mode === "missing") { reporter.onEnd({ status: "passed" }); reporter.onExit(); process.exit(); }
if (["assertion", "hook", "runtime", "timeout", "retry", "nested", "unproven", "staleProof", "preservedCause"].includes(mode)) {
  result.status = "failed"; result.errors = [error]; result.steps = [assertion];
}
if (mode === "unproven") delete error.cause;
if (mode === "staleProof") error.cause.message = `Error: PW_ASSERTION:${"b".repeat(64)}`;
if (mode === "preservedCause") error.cause.cause = { message: "original cause" };
if (mode === "hook") result.steps = [{ category: "hook", error, steps: [assertion] }];
if (mode === "runtime") result.steps = [];
if (mode === "timeout") result.status = "timedOut";
if (mode === "retry") result.retry = 1;
if (mode === "nested") result.steps = [{ category: "test.step", error, steps: [assertion] }];
if (mode === "skipped") result.status = "skipped";
if (mode === "unexpected") test.expectedStatus = "failed";
reporter.onTestBegin(test, result);
reporter.onTestEnd(test, result);
if (mode === "duplicate") reporter.onTestEnd(test, result);
if (mode === "global") reporter.onError(error);
if (mode === "extraError") { result.errors = [error, {message: "runtime"}]; result.status = "failed"; reporter.onError(result.errors[1]); }
if (mode !== "noEnd") reporter.onEnd({ status: ["passed", "skipped"].includes(result.status) ? "passed" : "failed" });
if (mode === "doubleEnd") reporter.onEnd({ status: "passed" });
reporter.onExit();

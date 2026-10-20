import { existsSync, renameSync, writeFileSync } from "node:fs";
import { PLAN_SCHEMA, collectionRequest, selectionFromSuite } from "./host-plan.mjs";

// Playwright swallows reporter exceptions. Invalid callbacks must therefore
// prevent publication, and the parent also requires a successful child exit.
export default class HostPlanReporter {
  constructor() {
    this.request = collectionRequest();
    this.problem = !this.request;
    this.begins = 0;
    this.ends = 0;
    this.selection = null;
  }
  printsToStdio() { return true; }
  onBegin(config, suite) {
    this.begins++;
    try {
      if (this.begins !== 1 || !this.request) throw new Error("invalid collection callback sequence");
      this.selection = selectionFromSuite(config, suite, this.request);
    } catch (error) {
      this.problem = true;
      console.error(`browser host collection refused: ${error.message}`);
    }
  }
  onError(error) {
    this.problem = true;
    console.error(error.message ?? error);
  }
  onTestBegin() { this.problem = true; }
  onEnd(result) {
    this.ends++;
    if (this.ends !== 1 || result.status !== "passed") this.problem = true;
  }
  onExit() {
    try {
      if (this.problem || this.begins !== 1 || this.ends !== 1 || !this.selection) return;
      if (existsSync(this.request.report)) throw new Error("receipt already exists");
      const receipt = {
        schema: PLAN_SCHEMA, complete: true, nonce: this.request.nonce,
        issuerPid: this.request.issuerPid, collectorPid: process.pid,
        cwd: this.request.cwd, root: this.request.root, argv: this.request.argv,
        collectionArgv: this.request.collectionArgv, forbidOnly: true, ...this.selection,
      };
      const temporary = `${this.request.report}.part`;
      writeFileSync(temporary, JSON.stringify(receipt), { flag: "wx", mode: 0o600 });
      renameSync(temporary, this.request.report);
    } catch (error) {
      this.problem = true;
      console.error(`browser host collection receipt refused: ${error.message}`);
    }
  }
}
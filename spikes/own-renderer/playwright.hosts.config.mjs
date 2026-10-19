// Track `second-host` (ADR-XXXX): the browser suite of hosts that share one
// database, apart from the main suite (`playwright.config.mjs`). Its spec
// starts its own hosts, two per engine, each a process of its own that it
// stops and starts again, on a PostgreSQL schema of the engine's own: the
// main suite's hosts keep their data in memory (ADR-0257), and Playwright's
// `webServer` cannot stop one host while a test runs. Needs the feed's build
// (`feed.sh`), the development server (`cargo build -p pw-dev-server`) and
// PW_FEED_DATABASE_URL, a throwaway database; `just e14-second-host` runs it.
import { createHash } from "node:crypto";
import { existsSync, readFileSync } from "node:fs";
import { defineConfig, devices } from "@playwright/test";

// The feed's build, held as the main suite holds it (ADR-0161, ADR-0166):
// built, with the runtime the repository has, from the sources it records.
// A stale build is refused here rather than served: this suite has no other.
const at = (path) => new URL(path, import.meta.url);
if (!existsSync(at("./dist-feed/build"))) {
  throw new Error("dist-feed is not built: run feed.sh");
}
if (
  readFileSync(at("./dist-feed/pw-runtime.mjs"), "utf8") !==
  readFileSync(at("./public/pw-runtime.mjs"), "utf8")
) {
  throw new Error("dist-feed was built with another pw-runtime.mjs: run feed.sh again");
}
const { files } = JSON.parse(readFileSync(at("./dist-feed/sources.json"), "utf8"));
const digest = (path) => createHash("sha256").update(readFileSync(path)).digest("hex");
const changed = files.find(({ path, sha256 }) => existsSync(path) && digest(path) !== sha256);
if (changed) {
  throw new Error(`${changed.path} changed since dist-feed was built: run feed.sh again`);
}

export default defineConfig({
  testDir: "./e2e-hosts",
  // One worker per engine: each engine's two hosts, and its tests in order.
  workers: 3,
  fullyParallel: false,
  reporter: [["list"]],
  // A test that stops a host and starts it again waits for the new process
  // to be ready, as a deployment's restart does.
  timeout: 90_000,
  // A test that fails on CI keeps its trace, as the main suite's does. Not a
  // retry: a test that fails once is a finding.
  use: {
    trace: process.env.CI ? "retain-on-failure" : "off",
  },
  projects: [
    { name: "chromium", use: { ...devices["Desktop Chrome"] } },
    { name: "firefox", use: { ...devices["Desktop Firefox"] } },
    { name: "webkit", use: { ...devices["Desktop Safari"] } },
  ],
});

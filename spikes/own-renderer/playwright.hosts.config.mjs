// Track `second-host` (ADR-XXXX): the browser suite of hosts that share one
// database, apart from the main suite (`playwright.config.mjs`). Its spec
// starts its own hosts, two per engine, each a process of its own that it
// stops and starts again, on a PostgreSQL schema of the engine's own: the
// main suite's hosts keep their data in memory (ADR-0257), and Playwright's
// `webServer` cannot stop one host while a test runs. Needs the feed's build
// (`feed.sh`), the development server (`cargo build -p pw-dev-server`) and
// PW_FEED_DATABASE_URL, a throwaway database; `just e14-second-host` runs it.
import { defineConfig, devices } from "@playwright/test";

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

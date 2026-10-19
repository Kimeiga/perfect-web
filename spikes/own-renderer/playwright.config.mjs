// Native Playwright collection selects the role/engine dependency closure.
// Upstream webServer still owns readiness, process groups and teardown.
import { defineConfig, devices } from "@playwright/test";
import { ENGINES, PORT } from "./host-metadata.mjs";
import { selectedTests, serversForSelection } from "./host-plan.mjs";

const selection = await selectedTests();
const device = { chromium: "Desktop Chrome", firefox: "Desktop Firefox", webkit: "Desktop Safari" };

export default defineConfig({
  testDir: "./e2e",
  // Native list does not apply focused-test filtering. Both phases refuse it.
  forbidOnly: true,
  testIgnore: process.env.PW_PERFORMANCE ? [] : ["**/performance.spec.mjs"],
  fullyParallel: true,
  reporter: [["list"]],
  use: {
    baseURL: `http://127.0.0.1:${PORT}`,
    trace: process.env.CI ? "retain-on-failure" : "off",
  },
  projects: ENGINES.map((name) => ({ name, use: { ...devices[device[name]] } })),
  webServer: serversForSelection(selection),
});

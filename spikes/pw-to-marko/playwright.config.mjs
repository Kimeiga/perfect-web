// E3 browser tests — charter §14 M3 tasks 11 and 13.
//
// Three engines, because "works in Chrome" is not the claim: the gate says all
// three pass. The server is the BUILT output, not a dev server — a dev server
// would measure Vite's behaviour rather than what a user receives.
import { defineConfig, devices } from "@playwright/test";

const PORT = 3122;

export default defineConfig({
  testDir: "./e2e",
  fullyParallel: true,
  forbidOnly: !!process.env.CI,
  retries: 0,
  reporter: [["list"]],
  use: { baseURL: `http://127.0.0.1:${PORT}`, trace: "off" },
  projects: [
    { name: "chromium", use: { ...devices["Desktop Chrome"] } },
    { name: "firefox", use: { ...devices["Desktop Firefox"] } },
    { name: "webkit", use: { ...devices["Desktop Safari"] } },
  ],
  webServer: {
    command: `node dist/index.mjs`,
    env: { PORT: String(PORT) },
    url: `http://127.0.0.1:${PORT}/static`,
    reuseExistingServer: false,
    timeout: 30_000,
  },
});

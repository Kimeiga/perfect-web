// The own renderer's pages are static files, so the "server" is a file server
// and nothing else. That is the point of gate 1: no framework runs at request
// time because there is nothing to run.
import { defineConfig, devices } from "@playwright/test";

const PORT = Number(process.env.PORT ?? 3141);

export default defineConfig({
  testDir: "./e2e",
  fullyParallel: true,
  reporter: [["list"]],
  use: { baseURL: `http://127.0.0.1:${PORT}`, trace: "off" },
  projects: [
    { name: "chromium", use: { ...devices["Desktop Chrome"] } },
    { name: "firefox", use: { ...devices["Desktop Firefox"] } },
    { name: "webkit", use: { ...devices["Desktop Safari"] } },
  ],
  webServer: {
    command: `npx --yes http-server dist -p ${PORT} -s --no-dotfiles`,
    port: PORT,
    reuseExistingServer: false,
    timeout: 60_000,
  },
});

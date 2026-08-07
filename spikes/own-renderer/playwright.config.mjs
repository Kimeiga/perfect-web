// The own renderer's pages come from a Rust development server that owns
// `pw-materialize`, `pw-resource`, `pw-render` and `pw-protocol`. The browser
// sees only the protocol.
//
// Not E8's production host: its deletion condition is that E8 replaces it with
// the capability-constrained one while preserving the `pw-protocol` boundary.
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
    command: `../../target/debug/pw-dev-server dist`,
    env: { PORT: String(PORT) },
    port: PORT,
    reuseExistingServer: false,
    timeout: 60_000,
  },
});

// The own renderer's pages come from a Rust development server that owns
// `pw-materialize`, `pw-resource`, `pw-render` and `pw-protocol`. The browser
// sees only the protocol.
//
// Not E8's production host: its deletion condition is that E8 replaces it with
// the capability-constrained one while preserving the `pw-protocol` boundary.
import { defineConfig, devices } from "@playwright/test";

const PORT = Number(process.env.PORT ?? 3141);

// One host per engine, for the tests that MUTATE the shared menu.
//
// The menu is public data: one list, broadcast to every subscriber, which is
// the property `keyed-list.spec.mjs` exists to prove. That makes it hostile to
// a parallel suite — a reorder committed by one run is visible to every other,
// and `store.spec.mjs` reasonably assumes three items in a known order.
//
// Isolation in the HARNESS rather than in the server: a per-session menu would
// delete the property under test. One host per engine rather than one shared
// mutable host, because the three engine projects run concurrently and a
// single extra host merely moved the interference somewhere less obvious —
// where it showed up as one engine's reorder failing for another engine's
// reasons.
export const MUTABLE_PORTS = {
  chromium: PORT + 1,
  firefox: PORT + 2,
  webkit: PORT + 3,
};

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
  webServer: [
    {
      command: `../../target/debug/pw-dev-server dist`,
      env: { PORT: String(PORT) },
      port: PORT,
      reuseExistingServer: !!process.env.PW_REUSE,
      timeout: 60_000,
    },
    ...Object.values(MUTABLE_PORTS).map((port) => ({
      command: `../../target/debug/pw-dev-server dist`,
      env: { PORT: String(port) },
      port,
      reuseExistingServer: !!process.env.PW_REUSE,
      timeout: 60_000,
    })),
  ],
});

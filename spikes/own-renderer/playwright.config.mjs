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
const ENGINES = ["chromium", "firefox", "webkit"];

/// The suites that mutate shared state, each with a host per engine.
///
/// Per SUITE as well as per engine: two mutating files on one host interfere
/// exactly as two engines did, and the second time it presented as frames
/// arriving in a different order rather than as a wrong list — which is much
/// harder to read as interference.
const MUTATING = ["keyed-list", "transport", "performance"];

export const MUTABLE_PORTS = Object.fromEntries(
  MUTATING.map((suite, s) => [
    suite,
    Object.fromEntries(ENGINES.map((e, i) => [e, PORT + 1 + s * ENGINES.length + i])),
  ]),
);

// The performance run needs ONE host, not eleven.
//
// Playwright starts every declared `webServer` before the first test, and
// eleven processes coming up while the first navigation happens is itself the
// load a long-animation-frame measurement is trying not to see. It failed
// exactly that way: gate 8 reported an interaction long frame on a cold start
// and none on a warm one.
const HOSTS = process.env.PW_PERFORMANCE
  ? [MUTABLE_PORTS.performance.chromium]
  : [PORT, ...MUTATING.flatMap((suite) => Object.values(MUTABLE_PORTS[suite]))];

export default defineConfig({
  testDir: "./e2e",
  // The performance gate is excluded from the parallel suite and run alone by
  // `just e7-performance`.
  //
  // Not a convenience: three engine families times several workers saturates
  // the machine, and a long-animation-frame measurement taken under that load
  // measures the load. It failed exactly that way — the clean run reported a
  // long frame that the same page, alone, does not produce.
  testIgnore: process.env.PW_PERFORMANCE ? [] : ["**/performance.spec.mjs"],
  fullyParallel: true,
  reporter: [["list"]],
  use: { baseURL: `http://127.0.0.1:${PORT}`, trace: "off" },
  projects: [
    { name: "chromium", use: { ...devices["Desktop Chrome"] } },
    { name: "firefox", use: { ...devices["Desktop Firefox"] } },
    { name: "webkit", use: { ...devices["Desktop Safari"] } },
  ],
  webServer: [
    ...HOSTS.map((port) => ({
      command: `../../target/debug/pw-dev-server dist`,
      env: { PORT: String(port) },
      port,
      reuseExistingServer: !!process.env.PW_REUSE,
      timeout: 60_000,
    })),
  ],
});

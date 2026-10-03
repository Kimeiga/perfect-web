// The own renderer's pages come from a Rust development server that owns
// `pw-materialize`, `pw-resource`, `pw-render` and `pw-protocol`. The browser
// sees only the protocol.
///
// Not E8's production host: its deletion condition is that E8 replaces it with
// the capability-constrained one while preserving the `pw-protocol` boundary.
import { existsSync, readFileSync } from "node:fs";
import { defineConfig, devices } from "@playwright/test";

const PORT = Number(process.env.PORT ?? 3141);

// One host per engine, for the tests that MUTATE the shared menu.
///
// The menu is public data: one list, broadcast to every subscriber, which is
// the property `keyed-list.spec.mjs` exists to prove. That makes it hostile to
// a parallel suite — a reorder committed by one run is visible to every other,
// and `store.spec.mjs` reasonably assumes three items in a known order.
///
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
///
/// `public-fragment` since 2026-09-25. It renames a menu item and back, which
/// broadcasts a patch to every page on its host, and it had been running on the
/// shared one. A store page there that had just updated its cart received
/// another test's rename as its latest batch, and `store.spec.mjs`'s "only
/// cart-related part ids update" read the rename. That happened once in fifteen
/// three-engine runs, and was then reproduced on demand by renaming from a
/// second context. Appended, so the other suites keep their ports.
///
/// `stream` since 2026-10-03 (ADR-0148): its recommender is one per server,
/// and a test that makes it fail would fail another engine's page.
///
/// `availability` since 2026-10-03 (ADR-0157): an item sold out is sold out
/// for every page on the host.
const MUTATING = [
  "keyed-list",
  "transport",
  "performance",
  "public-fragment",
  "stream",
  // ADR-0157: an item sold out is one per server.
  "availability",
];

export const MUTABLE_PORTS = Object.fromEntries(
  MUTATING.map((suite, s) => [
    suite,
    Object.fromEntries(ENGINES.map((e, i) => [e, PORT + 1 + s * ENGINES.length + i])),
  ]),
);

// The performance run needs ONE host, not eleven.
///
// Playwright starts every declared `webServer` before the first test, and
// eleven processes coming up while the first navigation happens is itself the
// load a long-animation-frame measurement is trying not to see. It failed
// exactly that way: gate 8 reported an interaction long frame on a cold start
// and none on a warm one.
// The kiokun slice's host (E10): a second application, its own server, and
// read-only, so every engine shares it.
export const KIOKUN_PORT = PORT + 40;

// The keyed store's hosts (ADR-0152): the store with T07's category tabs,
// built into `dist-keyed` by `keyed-store.sh`. One per engine, because which
// category is slow is one per server. Served only when it is built.
export const KEYED_PORTS = Object.fromEntries(ENGINES.map((e, i) => [e, PORT + 50 + i]));
// A build serves the runtime it was built with. One built before the runtime
// changed runs the old runtime against the new server, and fails for a reason
// that is no test's: `dist-keyed` did, once each page's subscription named
// its document (ADR-0161).
// - `dist` is refused: `run.sh` builds it before every suite, and every
//   mutation control's run.
// - A stale `dist-keyed` is not served, and the keyed suite not run, with a
//   warning that says what to run. Refusing it would fail every mutation
//   control that changes the runtime and runs another spec, for no test's
//   reason; the recipes that run the keyed suite build it first.
const RUNTIME = readFileSync(new URL("./public/pw-runtime.mjs", import.meta.url), "utf8");
const builtWith = (dist) => {
  const built = new URL(`./${dist}/pw-runtime.mjs`, import.meta.url);
  return existsSync(built) ? readFileSync(built, "utf8") : null;
};
if (builtWith("dist") !== null && builtWith("dist") !== RUNTIME) {
  throw new Error("dist was built with another pw-runtime.mjs: run run.sh again");
}
const KEYED_STALE =
  existsSync(new URL("./dist-keyed/build", import.meta.url)) && builtWith("dist-keyed") !== RUNTIME;
if (KEYED_STALE) {
  console.warn(
    "dist-keyed was built with another pw-runtime.mjs: e2e/keyed.spec.mjs is not run; " +
      "run keyed-store.sh again",
  );
}
const KEYED_BUILT = existsSync(new URL("./dist-keyed/build", import.meta.url)) && !KEYED_STALE;

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
  testIgnore: [
    ...(process.env.PW_PERFORMANCE ? [] : ["**/performance.spec.mjs"]),
    // The keyed store's suite needs its build (`keyed-store.sh`).
    ...(KEYED_BUILT ? [] : ["**/keyed.spec.mjs"]),
  ],
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
    ...(KEYED_BUILT && !process.env.PW_PERFORMANCE
      ? Object.values(KEYED_PORTS).map((port) => ({
          command: `../../target/debug/pw-dev-server dist-keyed`,
          env: { PORT: String(port) },
          port,
          reuseExistingServer: !!process.env.PW_REUSE,
          timeout: 60_000,
        }))
      : []),
    ...(process.env.PW_PERFORMANCE
      ? []
      : [
          {
            command: `../../target/debug/kiokun-server`,
            env: { PORT: String(KIOKUN_PORT) },
            port: KIOKUN_PORT,
            reuseExistingServer: !!process.env.PW_REUSE,
            timeout: 60_000,
          },
        ]),
  ],
});

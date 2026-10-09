// The own renderer's pages come from a Rust development server that owns
// `pw-materialize`, `pw-resource`, `pw-render` and `pw-protocol`. The browser
// sees only the protocol.
///
// Not E8's production host: its deletion condition is that E8 replaces it with
// the capability-constrained one while preserving the `pw-protocol` boundary.
import { createHash } from "node:crypto";
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
  // ADR-0162: store 47's menu, which a test renames, is one per server.
  "stores",
  // ADR-0165: the recommender, which a test slows, is one per server.
  "slots",
  // ADR-0172: an item sold out, which a cart's test refuses, is one per
  // server.
  "cart",
  // ADR-0174: the store's delay, which a test sets, is one per server.
  "controls",
  // ADR-0182: an item sold out, and the menu's changes, which the
  // accessibility audit reads the page after, are one per server.
  "accessibility",
  // ADR-0277: the menu a test adds to and takes from, which its count and
  // line are made again from, is one per server.
  "materialized",
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
// The feed reference app's hosts (ADR-0220): a second program on the same
// server, built into `dist-feed` by `feed.sh`. One per engine, because a
// post is every reader's. Served only when it is built.
export const FEED_PORTS = Object.fromEntries(ENGINES.map((e, i) => [e, PORT + 60 + i]));
// The identity track's hosts (ADR-0258): the feed's build again, its
// sessions signed in through the development identity provider
// (`PW_IDENTITY=dev-accounts`). One per engine, because an account and a post
// are every reader's on a host. Served only when the feed is built.
export const IDENTITY_PORTS = Object.fromEntries(ENGINES.map((e, i) => [e, PORT + 70 + i]));
// The uploads track's hosts (ADR-0260): the feed's build again, each with a
// blob storage of its own (`PW_BLOB_DIR`), so one engine's images are never
// another's. Served only when the feed is built.
export const UPLOADS_PORTS = Object.fromEntries(ENGINES.map((e, i) => [e, PORT + 80 + i]));
// The notifications track's hosts (ADR-0274): the feed's build again, its
// sessions signed in through the development identity provider, so one user
// reads in two sessions and another beside them. One per engine, because a
// notification is its user's on a host. Served only when the feed is built.
export const NOTIFICATIONS_PORTS = Object.fromEntries(
  ENGINES.map((e, i) => [e, PORT + 90 + i]),
);
// The messages track's hosts (ADR-0279): the feed's build again, its
// sessions signed in through the development identity provider, so two users
// message each other, one of them in two sessions, and a third reads beside
// them. One per engine, because a message is its users' on a host. Served
// only when the feed is built.
export const MESSAGES_PORTS = Object.fromEntries(ENGINES.map((e, i) => [e, PORT + 100 + i]));
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
// And the sources it was built from (ADR-0166), each by its digest, as
// `run.sh` records them. On 2026-10-03 the suite ran on a build of an
// experiment whose source had since been put back, and passed a page the
// sources no longer described. A source that is gone is a copy a build made
// for itself (`keyed-store.sh`'s), and is not held to.
const changedSince = (dist) => {
  const recorded = new URL(`./${dist}/sources.json`, import.meta.url);
  if (!existsSync(recorded)) return `${dist} records no sources`;
  const { files } = JSON.parse(readFileSync(recorded, "utf8"));
  const digest = (path) => createHash("sha256").update(readFileSync(path)).digest("hex");
  const changed = files.find(({ path, sha256 }) => existsSync(path) && digest(path) !== sha256);
  return changed ? `${changed.path} changed since ${dist} was built` : null;
};
if (builtWith("dist") !== null && changedSince("dist")) {
  throw new Error(`${changedSince("dist")}: run run.sh again`);
}
const KEYED_WHY = !existsSync(new URL("./dist-keyed/build", import.meta.url))
  ? null
  : builtWith("dist-keyed") !== RUNTIME
    ? "dist-keyed was built with another pw-runtime.mjs"
    : changedSince("dist-keyed");
const KEYED_STALE = KEYED_WHY !== null;
if (KEYED_STALE) {
  console.warn(`${KEYED_WHY}: e2e/keyed.spec.mjs is not run; run keyed-store.sh again`);
}
const KEYED_BUILT = existsSync(new URL("./dist-keyed/build", import.meta.url)) && !KEYED_STALE;
// The feed's build, held to the same: its runtime and its sources.
const FEED_WHY = !existsSync(new URL("./dist-feed/build", import.meta.url))
  ? null
  : builtWith("dist-feed") !== RUNTIME
    ? "dist-feed was built with another pw-runtime.mjs"
    : changedSince("dist-feed");
if (FEED_WHY !== null) {
  console.warn(`${FEED_WHY}: e2e/feed.spec.mjs is not run; run feed.sh again`);
}
const FEED_BUILT = existsSync(new URL("./dist-feed/build", import.meta.url)) && FEED_WHY === null;

const HOSTS = process.env.PW_PERFORMANCE
  ? [MUTABLE_PORTS.performance.chromium]
  : [PORT, ...MUTATING.flatMap((suite) => Object.values(MUTABLE_PORTS[suite]))];

// **The hosts keep their data in memory** (ADR-0257), whatever the caller's
// environment says: Playwright gives a server the caller's environment with
// its own `env` over it, and a `PW_FEED_DATABASE_URL` left there put every
// engine's feed host on one database, and each run's posts on the last's.
// A suite on PostgreSQL is the server's tests' (`e14-feed-postgres`).
const IN_MEMORY = { PW_FEED_DATABASE_URL: "" };

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
    // The feed's needs its (`feed.sh`).
    ...(FEED_BUILT && !process.env.PW_PERFORMANCE ? [] : ["**/feed.spec.mjs"]),
    // The identity track's, the feed's build with accounts.
    ...(FEED_BUILT && !process.env.PW_PERFORMANCE ? [] : ["**/identity.spec.mjs"]),
    // The uploads track's, the feed's build with its images.
    ...(FEED_BUILT && !process.env.PW_PERFORMANCE ? [] : ["**/uploads.spec.mjs"]),
    // The notifications track's, the feed's build with accounts.
    ...(FEED_BUILT && !process.env.PW_PERFORMANCE ? [] : ["**/notifications.spec.mjs"]),
    // The messages track's, the feed's build with accounts.
    ...(FEED_BUILT && !process.env.PW_PERFORMANCE ? [] : ["**/messages.spec.mjs"]),
  ],
  fullyParallel: true,
  reporter: [["list"]],
  // A test that fails on CI keeps its trace, the network's frames among it,
  // in the browser job's `test-results` artifact: a failure seen once and not
  // here (WebKit's "Load more", run 37663299969) left only a page snapshot.
  // Not a retry: a test that fails once is a finding, and not to be run again
  // until it passes.
  use: {
    baseURL: `http://127.0.0.1:${PORT}`,
    trace: process.env.CI ? "retain-on-failure" : "off",
  },
  projects: [
    { name: "chromium", use: { ...devices["Desktop Chrome"] } },
    { name: "firefox", use: { ...devices["Desktop Firefox"] } },
    { name: "webkit", use: { ...devices["Desktop Safari"] } },
  ],
  webServer: [
    ...HOSTS.map((port) => ({
      command: `../../target/debug/pw-dev-server dist`,
      env: { PORT: String(port), ...IN_MEMORY },
      port,
      reuseExistingServer: !!process.env.PW_REUSE,
      timeout: 60_000,
    })),
    ...(KEYED_BUILT && !process.env.PW_PERFORMANCE
      ? Object.values(KEYED_PORTS).map((port) => ({
          command: `../../target/debug/pw-dev-server dist-keyed`,
          env: { PORT: String(port), ...IN_MEMORY },
          port,
          reuseExistingServer: !!process.env.PW_REUSE,
          timeout: 60_000,
        }))
      : []),
    ...(FEED_BUILT && !process.env.PW_PERFORMANCE
      ? Object.values(FEED_PORTS).map((port) => ({
          command: `../../target/debug/pw-dev-server dist-feed`,
          env: { PORT: String(port), ...IN_MEMORY },
          port,
          reuseExistingServer: !!process.env.PW_REUSE,
          timeout: 60_000,
        }))
      : []),
    ...(FEED_BUILT && !process.env.PW_PERFORMANCE
      ? Object.values(IDENTITY_PORTS).map((port) => ({
          command: `../../target/debug/pw-dev-server dist-feed`,
          env: { PORT: String(port), PW_IDENTITY: "dev-accounts", ...IN_MEMORY },
          port,
          reuseExistingServer: !!process.env.PW_REUSE,
          timeout: 60_000,
        }))
      : []),
    ...(FEED_BUILT && !process.env.PW_PERFORMANCE
      ? Object.values(UPLOADS_PORTS).map((port) => ({
          command: `../../target/debug/pw-dev-server dist-feed`,
          env: { PORT: String(port), PW_BLOB_DIR: `dist-feed/blobs-${port}`, ...IN_MEMORY },
          port,
          reuseExistingServer: !!process.env.PW_REUSE,
          timeout: 60_000,
        }))
      : []),
    ...(FEED_BUILT && !process.env.PW_PERFORMANCE
      ? Object.values(NOTIFICATIONS_PORTS).map((port) => ({
          command: `../../target/debug/pw-dev-server dist-feed`,
          env: { PORT: String(port), PW_IDENTITY: "dev-accounts", ...IN_MEMORY },
          port,
          reuseExistingServer: !!process.env.PW_REUSE,
          timeout: 60_000,
        }))
      : []),
    ...(FEED_BUILT && !process.env.PW_PERFORMANCE
      ? Object.values(MESSAGES_PORTS).map((port) => ({
          command: `../../target/debug/pw-dev-server dist-feed`,
          env: { PORT: String(port), PW_IDENTITY: "dev-accounts", ...IN_MEMORY },
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

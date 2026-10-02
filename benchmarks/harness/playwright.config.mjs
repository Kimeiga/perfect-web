// E14-A: one behavioural contract, three stores (ADR-0120).
//
// Each project is a stack: the Pleris store served by the own-renderer
// development server, and the same store written in Next.js and in SvelteKit.
// The tests do not know which stack they run against beyond `storePath` and
// `ready`, which is the point: a grading test that branches on the stack
// would grade three different things.
//
// Built artifacts only (`next start`, `node build`, the compiled Pleris
// page). A development server's hot-reload socket and on-demand compilation
// are not the store.
import { defineConfig, devices } from "@playwright/test";

const BASE = Number(process.env.BENCH_PORT ?? 3200);
export const STACKS = {
  pleris: { port: BASE + 1, storePath: "/StorePage.html" },
  "next-react": { port: BASE + 2, storePath: "/stores/blue-bottle" },
  sveltekit: { port: BASE + 3, storePath: "/stores/blue-bottle" },
};

const only = process.env.BENCH_STACK;
const stacks = Object.entries(STACKS).filter(([name]) => !only || name === only);

const command = {
  pleris: "../../target/debug/pw-dev-server dist",
  "next-react": "pnpm start",
  sveltekit: "pnpm start",
};
const cwd = {
  pleris: "../../spikes/own-renderer",
  "next-react": "../baselines/next-react",
  sveltekit: "../baselines/sveltekit",
};

export default defineConfig({
  testDir: "./contract",
  fullyParallel: true,
  reporter: [["list"]],
  projects: stacks.map(([name, s]) => ({
    name,
    use: {
      ...devices["Desktop Chrome"],
      baseURL: `http://127.0.0.1:${s.port}`,
      storePath: s.storePath,
      stack: name,
    },
  })),
  webServer: stacks.map(([name, s]) => ({
    command: command[name],
    cwd: cwd[name],
    env: { PORT: String(s.port), HOST: "127.0.0.1", ORIGIN: `http://127.0.0.1:${s.port}` },
    port: s.port,
    reuseExistingServer: !!process.env.BENCH_REUSE,
    timeout: 60_000,
  })),
});

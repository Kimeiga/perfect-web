// The grading run: one stack, one already-running server, the shared
// contract and one task's hidden tests, copied by the harness into
// BENCH_SPECS. No web server is started here; the harness owns the server,
// because it built it from the sandbox.
import { defineConfig, devices } from "@playwright/test";

export default defineConfig({
  testDir: process.env.BENCH_SPECS,
  fullyParallel: true,
  workers: 2,
  retries: 0,
  reporter: [["json", { outputFile: process.env.BENCH_REPORT }]],
  projects: [
    {
      name: process.env.BENCH_STACK,
      use: {
        ...devices["Desktop Chrome"],
        baseURL: process.env.BENCH_BASE_URL,
        storePath: process.env.BENCH_STORE_PATH,
        stack: process.env.BENCH_STACK,
      },
    },
  ],
});

import { defineConfig } from "vite";
import marko from "@marko/run/vite";
import adapter from "@marko/run-adapter-node";

// spike: pw-to-marko — E3.
//
// Every route under src/routes is GENERATED from `.pw` by `pw emit-marko`
// (ADR-0017). Nothing here is authored by hand except this config and the
// package manifest. Same pins as the E0 spike, deliberately: E3 must not
// silently move the toolchain E0's measurements were taken against.
export default defineConfig({
  plugins: [marko({ adapter: adapter() })],
  build: {
    // Deterministic asset names so the measurement script can attribute bytes to
    // routes rather than to content hashes.
    minify: true,
  },
});

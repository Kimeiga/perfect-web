import { defineConfig } from "vite";
import marko from "@marko/run/vite";
import adapter from "@marko/run-adapter-node";

// spike: marko-stream-resume
// Charter §14 Milestone 0 task 9. Pinned: marko 6.3.32, @marko/run 0.11.8,
// @marko/run-adapter-node 2.0.6, vite 8.2.0 (verified on npm 2026-08-05).
export default defineConfig({
  plugins: [marko({ adapter: adapter() })],
  build: {
    // Deterministic asset names so the measurement script can attribute bytes to
    // routes rather than to content hashes.
    minify: true,
  },
});

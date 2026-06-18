import { defineConfig } from "vitest/config";

// Standalone Vitest config — intentionally does NOT load the SvelteKit plugin.
// The units under test (e.g. the render-envelope parser) are pure TypeScript
// with no DOM or Svelte runtime dependency, so a plain Node environment is
// sufficient and avoids the async `defineConfig` + `sveltekit()` setup in
// `vite.config.js`.
export default defineConfig({
  test: {
    environment: "node",
    include: ["src/**/*.{test,spec}.ts"],
  },
});

import { configDefaults, defineConfig } from "vitest/config";
import vue from "@vitejs/plugin-vue";

/**
 * Test-runner config — deliberately a separate file, not a `test` block inside
 * `vite.config.ts`.
 *
 * `vite.config.ts` carries the Tauri dev-server contract (port 1420, strict
 * port, `src-tauri` watch ignore) and the `safari15` build target; it is the
 * app's build recipe and nothing about tests belongs in it. Vitest reads
 * `vitest.config.ts` in preference to `vite.config.ts` when it exists, so the
 * two never merge and `pnpm build` / `pnpm tauri dev` see exactly what they
 * saw before.
 *
 * No build step: vitest transforms on demand through Vite (the vue plugin is
 * here so `.vue` component tests can be added later without touching config).
 */
export default defineConfig({
  plugins: [vue()],
  test: {
    // Components read `window.localStorage` (locale) and `document` (theme,
    // `lang` attribute), so the suite needs a DOM.
    environment: "jsdom",
    include: [
      "src/**/*.test.ts",
      "src/**/*.test.tsx",
      "src/**/*.spec.ts",
      "src/**/*.spec.tsx",
    ],
    // Spread the defaults (node_modules, dist, …) rather than replacing them;
    // `src-tauri` is a Rust crate — its Cargo test suite must never be picked
    // up by vitest.
    exclude: [...configDefaults.exclude, "src-tauri/**"],
  },
});

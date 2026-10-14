import { defineConfig } from "vite";
import vue from "@vitejs/plugin-vue";

// Tauri owns the terminal for its CLI output, and the dev server must sit on
// the fixed port tauri.conf.json points at (devUrl → 1420).
export default defineConfig({
  plugins: [vue()],
  clearScreen: false,
  server: {
    port: 1420,
    strictPort: true,
    watch: {
      ignored: ["**/src-tauri/**", "**/docs/**"],
    },
  },
  build: {
    // The app runs in WKWebView (macOS), not in a consumer browser.
    target: "safari15",
    sourcemap: true,
  },
});

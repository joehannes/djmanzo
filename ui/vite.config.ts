import { defineConfig } from "vite";
import { svelte } from "@sveltejs/vite-plugin-svelte";

export default defineConfig({
  plugins: [svelte()],
  // Tauri serves the dev build from a fixed port and fails if it moves.
  clearScreen: false,
  server: {
    port: 5173,
    strictPort: true,
  },
  build: {
    // Match what the webviews actually are: WKWebView on macOS, WebKitGTK on
    // Linux. No point shipping transpiled output for browsers we never target.
    target: "es2021",
    sourcemap: true,
  },
  test: {
    // `e2e/` is Playwright's, and its specs import `@playwright/test` -- which
    // vitest cannot run. Two runners in one project need one line saying which
    // files are whose, or the first one to see a file claims it.
    exclude: ["node_modules/**", "e2e/**"],
  },
});

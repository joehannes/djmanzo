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
});

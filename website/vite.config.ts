import { defineConfig } from "vite";
import { svelte } from "@sveltejs/vite-plugin-svelte";

export default defineConfig({
  plugins: [svelte()],
  build: { target: "es2022", chunkSizeWarningLimit: 900 },
  server: { proxy: { "/api": "http://127.0.0.1:8787", "/auth": "http://127.0.0.1:8787" } },
});

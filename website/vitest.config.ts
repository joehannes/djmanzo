import { defineConfig } from "vitest/config";
import { svelte } from "@sveltejs/vite-plugin-svelte";

export default defineConfig({
  // Svelte's compiler, so `.svelte.ts` modules (the island's state) can be tested.
  plugins: [svelte()],
  test: { include: ["test/**/*.test.ts", "src/**/*.test.ts"], exclude: ["e2e/**", "node_modules/**"], environment: "node" },
});

import tailwindcss from "@tailwindcss/vite";
import react from "@vitejs/plugin-react";
import type { Plugin } from "vite";
import { defineConfig } from "vitest/config";

/** The dev-server origin is allowed only while developing; release builds never trust it. */
function productionCsp(): Plugin {
  return {
    name: "wisp-production-csp",
    apply: "build",
    transformIndexHtml: (html) => html.replace(/ ws:\/\/127\.0\.0\.1:5173 http:\/\/127\.0\.0\.1:5173/, ""),
  };
}

export default defineConfig({
  plugins: [react(), tailwindcss(), productionCsp()],
  // Served from the wisp:// scheme inside the app; relative asset URLs keep it portable.
  base: "./",
  server: { port: 5173, strictPort: true, host: "127.0.0.1" },
  build: {
    outDir: "dist",
    target: "safari18",
    sourcemap: true,
  },
  test: {
    environment: "jsdom",
    restoreMocks: true,
    testTimeout: 20_000,
    setupFiles: ["./tests/setup.ts"],
  },
});

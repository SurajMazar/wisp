import tailwindcss from "@tailwindcss/vite";
import react from "@vitejs/plugin-react";
import { defineConfig } from "vitest/config";

export default defineConfig({
  plugins: [react(), tailwindcss()],
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

// Vite configuration for the Unhoard UI: React, a fixed dev port for Tauri, and Vitest.
import react from "@vitejs/plugin-react";
import { defineConfig } from "vitest/config";

export default defineConfig({
  plugins: [react()],
  clearScreen: false,
  server: { port: 5173, strictPort: true },
  build: { target: "es2022", outDir: "dist", sourcemap: false },
  test: {
    environment: "jsdom",
    include: ["src/**/*.test.{ts,tsx}"],
    benchmark: { include: ["src/**/*.bench.ts"] },
  },
});

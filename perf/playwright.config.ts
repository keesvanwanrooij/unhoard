// Playwright configuration for the interaction timings (T-P3): the UI with the fake core,
// in Chromium, with the CPU slowed down four times to stand in for a mid-range PC.
import { defineConfig } from "@playwright/test";

export default defineConfig({
  testDir: ".",
  testMatch: "ui.spec.ts",
  timeout: 180_000,
  reporter: "list",
  use: { baseURL: "http://localhost:1425", viewport: { width: 1440, height: 900 } },
  webServer: {
    // A production build: the React development build is several times slower.
    command: "npx vite build --mode fake --outDir dist-fake --emptyOutDir && npx vite preview --mode fake --outDir dist-fake --port 1425 --strictPort",
    url: "http://localhost:1425",
    reuseExistingServer: false,
    cwd: "..",
    timeout: 120_000,
  },
  projects: [{ name: "chromium", use: { browserName: "chromium" } }],
});

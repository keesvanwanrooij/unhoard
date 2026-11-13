// Vitest configuration for the layout timings (T-P2); kept apart from the unit tests,
// because timings belong to `npm run perf`, not to `npm test`.
import { defineConfig } from "vitest/config";

export default defineConfig({
  test: { include: ["perf/**/*.perf.ts"], environment: "node", testTimeout: 120_000 },
});

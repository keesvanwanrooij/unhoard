// Layout timings (T-P2): one relaxation step, the treemap, the morph and the grouping with
// 50 and 300 cells. Writes the 95th percentiles to perf/results/layout.json.
import { mkdirSync, writeFileSync } from "node:fs";

import { expect, it } from "vitest";

import { lerpPoly, rectPoly } from "../src/map/geometry";
import { groupSmall } from "../src/map/grouping";
import { makeLevel, relax, type Entry } from "../src/map/powerLayout";
import { squarify } from "../src/map/treemap";

function p95(samples: number[]): number {
  const s = [...samples].sort((a, b) => a - b);
  return s[Math.min(s.length - 1, Math.floor(s.length * 0.95))] ?? 0;
}

function time(runs: number, f: () => void): number {
  for (let i = 0; i < 5; i++) f();
  const out: number[] = [];
  for (let i = 0; i < runs; i++) {
    const t = performance.now();
    f();
    out.push(performance.now() - t);
  }
  return p95(out);
}

it("tp2_layout_timings", () => {
  const stage = rectPoly(0, 0, 1200, 760);
  const results: Record<string, number> = {};
  for (const n of [50, 300]) {
    const e: Entry[] = Array.from({ length: n }, (_, i) => ({ id: `n${String(i)}`, value: 1000 / (i + 1) + 5 }));
    const level = makeLevel(stage, `p${String(n)}`);
    relax(level, e, 60);
    results[`layout.relaxStep.${String(n)}`] = time(60, () => {
      relax(level, e, 1);
    });
    results[`layout.treemap.${String(n)}`] = time(60, () => squarify(e, 0, 0, 1200, 760));
    results[`layout.group.${String(n)}`] = time(60, () => groupSmall(e, 1e6));
    const a = level.sites.get("n0")?.cell ?? stage;
    results[`layout.morphFrame.${String(n)}`] = time(30, () => {
      for (let i = 0; i < n; i++) lerpPoly(a, stage, 0.5);
    });
  }
  mkdirSync("perf/results", { recursive: true });
  writeFileSync("perf/results/layout.json", JSON.stringify(results, null, 2));
  expect(Object.keys(results).length).toBe(8);
});

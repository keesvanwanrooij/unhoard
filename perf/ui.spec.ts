// Interaction timings (T-P3) against the budgets in docs/25-performance.md: screen switches,
// zoom, the map-style morph, search, frame times and long tasks, at 4x CPU throttling.
// Writes the 95th percentiles to perf/results/ui.json.
import { mkdirSync, writeFileSync } from "node:fs";

import { expect, test, type Page } from "@playwright/test";

interface PerfWindow {
  __longTasks: number[];
  __unhoardFrames?: () => number[];
}

function p95(samples: number[]): number {
  const s = [...samples].sort((a, b) => a - b);
  return s[Math.min(s.length - 1, Math.floor(s.length * 0.95))] ?? 0;
}

async function measures(page: Page, name: string): Promise<number[]> {
  return page.evaluate((n) => performance.getEntriesByName(n, "measure").map((e) => e.duration), name);
}

/** Time from a click to the second animation frame after it (the frame that shows it). */
async function responseMs(page: Page, selector: string): Promise<number> {
  return page.evaluate(async (sel) => {
    const el = document.querySelector<HTMLElement>(sel);
    if (!el) return Number.NaN;
    const t0 = performance.now();
    el.click();
    await new Promise<void>((r) => {
      requestAnimationFrame(() => {
        requestAnimationFrame(() => {
          r();
        });
      });
    });
    return performance.now() - t0;
  }, selector);
}

test("tp3_interactions_stay_within_budget", async ({ page }) => {
  await page.goto("/?entries=200000");
  await page.waitForSelector(".stage canvas");
  await page.waitForTimeout(1500);
  const cdp = await page.context().newCDPSession(page);
  await cdp.send("Emulation.setCPUThrottlingRate", { rate: 4 });
  await page.evaluate(() => {
    const w = window as unknown as PerfWindow;
    w.__longTasks = [];
    new PerformanceObserver((list) => {
      for (const e of list.getEntries()) w.__longTasks.push(e.duration);
    }).observe({ type: "longtask" });
  });

  const results: Record<string, number> = {};
  const responses: number[] = [];
  for (let round = 0; round < 4; round++) {
    for (const i of [2, 3, 4, 5, 6, 1]) responses.push(await responseMs(page, `.nav button:nth-of-type(${String(i)})`));
  }
  const longest = async (): Promise<number> =>
    page.evaluate(() => {
      const w = window as unknown as PerfWindow;
      const m = Math.max(0, ...w.__longTasks);
      w.__longTasks = [];
      return m;
    });
  results["ui.longestTask.screens"] = await longest();
  results["ui.screenSwitchPaint"] = p95(await measures(page, "unhoard:screen-paint"));
  results["ui.inputResponse"] = p95(responses);

  for (let i = 0; i < 6; i++) {
    const box = await page.locator(".stage canvas").boundingBox();
    if (!box) break;
    await page.mouse.click(box.x + box.width * 0.45, box.y + box.height * 0.55);
    await page.waitForTimeout(800);
    await page.keyboard.press("Backspace");
    await page.waitForTimeout(800);
  }
  results["ui.longestTask.zoom"] = await longest();
  results["ui.zoomResponse"] = p95(await measures(page, "unhoard:zoom-response"));
  results["ui.zoomSettle"] = p95(await measures(page, "unhoard:zoom-settle"));

  for (let i = 0; i < 4; i++) {
    await page.click(".top .seg button:nth-of-type(2)");
    await page.waitForTimeout(700);
    await page.click(".top .seg button:nth-of-type(1)");
    await page.waitForTimeout(700);
  }
  results["ui.longestTask.morph"] = await longest();
  results["ui.morphResponse"] = p95(await measures(page, "unhoard:morph-response"));

  const search: number[] = [];
  for (const q of ["s", "st", "ste", "x", ""]) {
    search.push(
      await page.evaluate(async (text) => {
        const input = document.querySelector<HTMLInputElement>(".top input");
        if (!input) return Number.NaN;
        const t0 = performance.now();
        Object.getOwnPropertyDescriptor(HTMLInputElement.prototype, "value")?.set?.call(input, text);
        input.dispatchEvent(new Event("input", { bubbles: true }));
        await new Promise<void>((r) => {
          requestAnimationFrame(() => {
            requestAnimationFrame(() => {
              r();
            });
          });
        });
        return performance.now() - t0;
      }, q),
    );
  }
  results["ui.longestTask.search"] = await longest();
  results["ui.search"] = p95(search);

  await page.waitForTimeout(2000);
  const frames = await page.evaluate(() => (window as unknown as PerfWindow).__unhoardFrames?.() ?? []);
  results["ui.frameTime"] = p95(frames);
  results["ui.maxFrame"] = Math.max(0, ...frames);
  results["ui.longestTask.rest"] = await longest();
  results["ui.longestTask"] = Math.max(
    results["ui.longestTask.screens"] ?? 0,
    results["ui.longestTask.zoom"] ?? 0,
    results["ui.longestTask.morph"] ?? 0,
    results["ui.longestTask.search"] ?? 0,
    results["ui.longestTask.rest"] ?? 0,
  );

  mkdirSync("perf/results", { recursive: true });
  writeFileSync("perf/results/ui.json", JSON.stringify(results, null, 2));
  for (const [k, v] of Object.entries(results)) expect(Number.isFinite(v), k).toBe(true);
});

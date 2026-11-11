// F2 squishy cell map: a dragged cell follows the pointer, its neighbours give way and
// flow back, and the areas are accurate again once the map has come to rest.
import { describe, expect, it } from "vitest";

import { area, centroid, inside, rectPoly, stretch, type Point } from "./geometry";
import { makeLevel, relax, type Entry } from "./powerLayout";

const sizes = [214, 135, 94, 70, 58, 49, 40, 38, 38, 36, 28.8, 24, 22, 14, 7.2];
const entries: Entry[] = sizes.map((v, i) => ({ id: `n${i}`, value: v }));
const stage = rectPoly(0, 0, 1200, 760);

describe("squishy cells", () => {
  it("f2_a_dragged_cell_follows_the_pointer", () => {
    const level = makeLevel(stage, "root");
    relax(level, entries, 200);
    const start = level.sites.get("n3");
    if (!start) throw new Error("no site");
    const from: Point = [start.x, start.y];
    const to: Point = [from[0] < 600 ? from[0] + 380 : from[0] - 380, from[1] < 380 ? from[1] + 220 : from[1] - 220];
    // The pointer moves in twenty steps, one per frame.
    for (let k = 1; k <= 20; k++) {
      const pin = { id: "n3", x: from[0] + ((to[0] - from[0]) * k) / 20, y: from[1] + ((to[1] - from[1]) * k) / 20 };
      relax(level, entries, 1, { pin, glide: true });
    }
    const site = level.sites.get("n3");
    expect(site?.x).toBeCloseTo(to[0]);
    expect(site?.y).toBeCloseTo(to[1]);
    // The cell came along: the pointer is inside it, and it kept a real size.
    expect(site?.cell && inside(site.cell, to[0], to[1])).toBe(true);
    expect(site?.area).toBeGreaterThan(1000);
    // Every other cell is still there.
    for (const e of entries) expect(level.sites.get(e.id)?.cell, e.id).not.toBeNull();
  });

  it("f2_the_map_flows_back_and_comes_to_rest_with_accurate_areas", () => {
    const level = makeLevel(stage, "root");
    relax(level, entries, 200);
    for (let k = 0; k < 30; k++) relax(level, entries, 1, { pin: { id: "n0", x: 100 + k * 30, y: 100 + k * 15 }, glide: true });
    // Let go: the cells flow to their new places and slow down.
    const first = relax(level, entries, 1, { glide: true });
    expect(first).toBeGreaterThan(0.5);
    let speed = first;
    let frames = 0;
    while (speed > 0.05 && frames < 600) {
      speed = relax(level, entries, 1, { glide: true });
      frames++;
    }
    expect(frames).toBeLessThan(600);
    relax(level, entries, 200);
    const total = sizes.reduce((s, v) => s + v, 0);
    for (const e of entries) {
      const cell = level.sites.get(e.id)?.cell;
      const share = Math.abs(area(cell ?? [])) / Math.abs(area(stage));
      expect(Math.abs(share / (e.value / total) - 1), e.id).toBeLessThan(0.03);
    }
  });

  it("f2_a_calm_drag_moves_the_other_cells_in_whole_steps", () => {
    const level = makeLevel(stage, "root");
    relax(level, entries, 200);
    for (let k = 0; k < 10; k++) relax(level, entries, 1, { pin: { id: "n0", x: 300 + k * 20, y: 300 }, glide: false });
    level.sites.forEach((s) => {
      if (s.id !== "n0") expect(Math.hypot(s.vx, s.vy)).toBe(0);
    });
  });

  it("f2_stretch_keeps_the_area_and_the_centre", () => {
    const square = rectPoly(100, 100, 80, 80);
    const c = centroid(square);
    const out = stretch(square, c, 1, 0, 0.2);
    expect(Math.abs(area(out))).toBeCloseTo(Math.abs(area(square)), 6);
    const c2 = centroid(out);
    expect(c2[0]).toBeCloseTo(c[0]);
    expect(c2[1]).toBeCloseTo(c[1]);
    const xs = out.map((p) => p[0]);
    expect(Math.max(...xs) - Math.min(...xs)).toBeCloseTo(96);
  });
});

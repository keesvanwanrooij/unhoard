// F2 layout tests: cell map and treemap accuracy, grouping of small items, determinism
// (T-2.1 to T-2.4).
import { describe, expect, it } from "vitest";

import { area, rectPoly } from "./geometry";
import { groupSmall } from "./grouping";
import { makeLevel, relax, type Entry } from "./powerLayout";
import { squarify } from "./treemap";

const sizes = [214, 135, 94, 70, 58, 49, 40, 38, 38, 36, 28.8, 24, 22, 14, 7.2];
const entries: Entry[] = sizes.map((v, i) => ({ id: `n${i}`, value: v }));
const stage = rectPoly(0, 0, 1200, 760);

function settle(): ReturnType<typeof makeLevel> {
  const level = makeLevel(stage, "root");
  relax(level, entries, 400);
  return level;
}

describe("cell map", () => {
  it("f2_cell_areas_are_within_3_percent_after_settling", () => {
    const level = settle();
    const total = sizes.reduce((s, v) => s + v, 0);
    for (const e of entries) {
      const cell = level.sites.get(e.id)?.cell;
      expect(cell, e.id).toBeDefined();
      const share = Math.abs(area(cell ?? [])) / Math.abs(area(stage));
      expect(Math.abs(share / (e.value / total) - 1)).toBeLessThan(0.03);
    }
  });

  it("f2_layout_is_deterministic", () => {
    const a = settle();
    const b = settle();
    for (const e of entries) {
      expect(a.sites.get(e.id)?.cell).toEqual(b.sites.get(e.id)?.cell);
    }
  });
});

describe("treemap", () => {
  it("f2_treemap_areas_are_within_3_percent", () => {
    const rects = squarify(entries, 0, 0, 1200, 760);
    const total = sizes.reduce((s, v) => s + v, 0);
    for (const e of entries) {
      const r = rects.get(e.id);
      expect(r).toBeDefined();
      const share = Math.abs(area(r ?? [])) / (1200 * 760);
      expect(Math.abs(share / (e.value / total) - 1)).toBeLessThan(0.03);
    }
  });
});

describe("grouping", () => {
  it("f2_items_below_half_a_percent_are_grouped", () => {
    const items = [
      { id: "big", value: 1000 },
      { id: "mid", value: 10 },
      { id: "tiny1", value: 4 },
      { id: "tiny2", value: 1 },
    ];
    const out = groupSmall(items, 1015);
    expect(out.map((e) => e.id)).toEqual(["big", "mid", "smaller"]);
    expect(out[2]).toMatchObject({ value: 5, count: 2 });
  });

  it("f2_nothing_is_grouped_when_everything_is_large_enough", () => {
    expect(groupSmall([{ id: "a", value: 50 }, { id: "b", value: 50 }], 100)).toHaveLength(2);
  });
});

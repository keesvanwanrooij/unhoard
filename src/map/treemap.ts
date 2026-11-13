// The simple treemap: a squarified layout of rectangles.
import { rectPoly, type Poly } from "./geometry";
import type { Entry } from "./powerLayout";

interface Item {
  id: string;
  a: number;
}

function worst(row: Item[], side: number): number {
  if (row.length === 0) return Infinity;
  const s = row.reduce((t, i) => t + i.a, 0);
  let max = 0;
  let min = Infinity;
  for (const i of row) {
    max = Math.max(max, i.a);
    min = Math.min(min, i.a);
  }
  return Math.max((side * side * max) / (s * s), (s * s) / (side * side * min));
}

/** Lays out `entries` as squarified rectangles in the box (x, y, w, h). */
export function squarify(entries: Entry[], x: number, y: number, w: number, h: number): Map<string, Poly> {
  const total = entries.reduce((s, e) => s + e.value, 0) || 1;
  const k = (w * h) / total;
  const items: Item[] = entries.map((e) => ({ id: e.id, a: e.value * k })).sort((p, q) => q.a - p.a);
  const out = new Map<string, Poly>();
  const R = { x, y, w, h };
  let row: Item[] = [];
  const lay = (r: Item[]): void => {
    const s = r.reduce((t, i) => t + i.a, 0);
    if (R.w >= R.h) {
      const cw = s / R.h;
      let yy = R.y;
      for (const i of r) {
        const hh = i.a / cw;
        out.set(i.id, rectPoly(R.x, yy, cw, hh));
        yy += hh;
      }
      R.x += cw;
      R.w -= cw;
    } else {
      const rh = s / R.w;
      let xx = R.x;
      for (const i of r) {
        const ww = i.a / rh;
        out.set(i.id, rectPoly(xx, R.y, ww, rh));
        xx += ww;
      }
      R.y += rh;
      R.h -= rh;
    }
  };
  while (items.length > 0) {
    const side = Math.min(R.w, R.h);
    const next = items[0] as Item;
    if (worst(row.concat([next]), side) <= worst(row, side)) {
      row.push(next);
      items.shift();
    } else {
      lay(row);
      row = [];
    }
  }
  if (row.length > 0) lay(row);
  return out;
}

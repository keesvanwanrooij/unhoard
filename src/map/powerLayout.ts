// The living cell map: a weighted Voronoi treemap computed as a power diagram.
//
// Each cell is the parent polygon clipped by one half-plane per other cell. Every
// relaxation step moves each site to its cell's centroid and adjusts its weight with a
// damped Newton step towards the target area. Running one step per frame from the previous
// state makes the map grow smoothly during a scan and breathe at rest.
import { area, centroid, clip, inside, perimeter, type Point, type Poly, bbox } from "./geometry";

/** One item to lay out: a stable ID and its size. */
export interface Entry {
  id: string;
  value: number;
}

/** The state of one site. */
export interface Site {
  id: string;
  x: number;
  y: number;
  w: number;
  cell: Poly | null;
  area: number;
}

/** One level of the map inside a container polygon. */
export interface Level {
  poly: Poly;
  key: string;
  sites: Map<string, Site>;
  rnd: () => number;
  settled: boolean;
  /** Relaxation steps done since the data last changed a lot. */
  steps: number;
}

/** A seeded pseudo-random generator (mulberry32). */
export function rng(seed: number): () => number {
  let a = seed >>> 0;
  return () => {
    a = (a + 0x6d2b79f5) | 0;
    let t = Math.imul(a ^ (a >>> 15), 1 | a);
    t = (t + Math.imul(t ^ (t >>> 7), 61 | t)) ^ t;
    return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
  };
}

/** FNV-1a hash of a string. */
export function hash(s: string): number {
  let h = 2166136261;
  for (let i = 0; i < s.length; i++) {
    h ^= s.charCodeAt(i);
    h = Math.imul(h, 16777619);
  }
  return h >>> 0;
}

function randomIn(p: Poly, r: () => number): Point {
  const b = bbox(p);
  for (let k = 0; k < 40; k++) {
    const x = b.x0 + r() * (b.x1 - b.x0);
    const y = b.y0 + r() * (b.y1 - b.y0);
    if (inside(p, x, y)) return [x, y];
  }
  return centroid(p);
}

/** Creates a level inside `poly`. Seeds come from `key`, so layouts are reproducible. */
export function makeLevel(poly: Poly, key: string): Level {
  return { poly, key, sites: new Map(), rnd: rng(hash(key)), settled: false, steps: 0 };
}

function sync(level: Level, entries: Entry[]): void {
  const ids = new Set(entries.map((e) => e.id));
  for (const id of [...level.sites.keys()]) if (!ids.has(id)) level.sites.delete(id);
  let wMin = Infinity;
  level.sites.forEach((s) => (wMin = Math.min(wMin, s.w)));
  const w0 = Number.isFinite(wMin) ? wMin : 0;
  for (const e of entries) {
    if (!level.sites.has(e.id)) {
      const [x, y] = randomIn(level.poly, rng(hash(level.key + e.id)));
      level.sites.set(e.id, { id: e.id, x, y, w: w0, cell: null, area: 0 });
    }
  }
}

function cells(level: Level, list: Site[]): void {
  for (const s of list) {
    let c: Poly = level.poly;
    for (const o of list) {
      if (o === s) continue;
      c = clip(c, 2 * (o.x - s.x), 2 * (o.y - s.y), o.x * o.x + o.y * o.y - s.x * s.x - s.y * s.y - o.w + s.w);
      if (c.length < 3) break;
    }
    s.cell = c.length >= 3 ? c : null;
    s.area = s.cell ? Math.abs(area(s.cell)) : 0;
  }
}

/** Runs `iters` relaxation steps towards the target areas of `entries`. */
export function relax(level: Level, entries: Entry[], iters: number): void {
  sync(level, entries);
  const list = entries.map((e) => level.sites.get(e.id)).filter((s): s is Site => s !== undefined);
  if (list.length === 0) return;
  const A = Math.abs(area(level.poly));
  const total = entries.reduce((s, e) => s + e.value, 0) || 1;
  if (list.length === 1) {
    const only = list[0] as Site;
    only.cell = level.poly;
    only.area = A;
    return;
  }
  // Each step starts from the cells of the previous step, so one step costs one cell
  // computation; that is what runs every frame.
  if (list.some((s) => s.cell === null)) cells(level, list);
  for (let it = 0; it < iters; it++) {
    for (const s of list) {
      if (s.cell) {
        const c = centroid(s.cell);
        s.x = c[0];
        s.y = c[1];
      } else {
        const p = randomIn(level.poly, level.rnd);
        s.x = p[0];
        s.y = p[1];
      }
    }
    cells(level, list);
    let wMax = -Infinity;
    for (const s of list) wMax = Math.max(wMax, s.w);
    list.forEach((s, i) => {
      const target = (A * (entries[i]?.value ?? 0)) / total;
      // Raising a weight by d moves each edge by about d / (2 · neighbour distance).
      if (s.area > 1e-6 && s.cell) s.w += (0.5 * (target - s.area) * 4 * Math.sqrt(s.area / Math.PI)) / perimeter(s.cell);
      else s.w = wMax;
    });
    for (const a of list) {
      for (const b of list) {
        if (a === b) continue;
        const d2 = (a.x - b.x) ** 2 + (a.y - b.y) ** 2;
        const over = a.w - b.w - d2 * 0.98;
        if (over > 0) a.w -= over;
      }
    }
  }
  // A bulk solve ends with cells that match the final weights.
  if (iters > 1) cells(level, list);
}

/** Moves every site and weight along when the container is resized. */
export function rescale(level: Level, poly: Poly): void {
  const a = bbox(level.poly);
  const b = bbox(poly);
  const sx = (b.x1 - b.x0) / (a.x1 - a.x0 || 1);
  const sy = (b.y1 - b.y0) / (a.y1 - a.y0 || 1);
  level.sites.forEach((s) => {
    s.x = b.x0 + (s.x - a.x0) * sx;
    s.y = b.y0 + (s.y - a.y0) * sy;
    s.w *= sx * sy;
  });
  level.poly = poly;
  level.settled = false;
}

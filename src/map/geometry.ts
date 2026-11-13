// Polygon geometry for the storage map: area, centroid, half-plane clipping, insets,
// hit testing and resampling for the cells-to-treemap morph.

/** A point `[x, y]` in canvas pixels. */
export type Point = [number, number];
/** A polygon as a list of points with positive (counter-clockwise in math terms) orientation. */
export type Poly = Point[];

/** Signed area of a polygon. */
export function area(p: Poly): number {
  let s = 0;
  for (let i = 0; i < p.length; i++) {
    const a = p[i] as Point;
    const b = p[(i + 1) % p.length] as Point;
    s += a[0] * b[1] - b[0] * a[1];
  }
  return s / 2;
}

/** Centroid of a polygon; the vertex mean for degenerate polygons. */
export function centroid(p: Poly): Point {
  let cx = 0;
  let cy = 0;
  let s = 0;
  for (let i = 0; i < p.length; i++) {
    const a = p[i] as Point;
    const b = p[(i + 1) % p.length] as Point;
    const f = a[0] * b[1] - b[0] * a[1];
    cx += (a[0] + b[0]) * f;
    cy += (a[1] + b[1]) * f;
    s += f;
  }
  if (Math.abs(s) < 1e-9) {
    const n = Math.max(p.length, 1);
    return [p.reduce((q, v) => q + v[0], 0) / n, p.reduce((q, v) => q + v[1], 0) / n];
  }
  return [cx / (3 * s), cy / (3 * s)];
}

/** Perimeter of a polygon. */
export function perimeter(p: Poly): number {
  let s = 0;
  for (let i = 0; i < p.length; i++) {
    const a = p[i] as Point;
    const b = p[(i + 1) % p.length] as Point;
    s += Math.hypot(b[0] - a[0], b[1] - a[1]);
  }
  return s;
}

/** Sutherland–Hodgman clip of a convex polygon by the half-plane a·x + b·y ≤ c. */
export function clip(p: Poly, a: number, b: number, c: number): Poly {
  // Most half-planes of a power diagram do not touch a given cell: return it unchanged
  // without allocating, which keeps a relaxation step fast with hundreds of cells.
  let inside = true;
  for (const P of p) {
    if (a * P[0] + b * P[1] - c > 0) {
      inside = false;
      break;
    }
  }
  if (inside) return p;
  const out: Poly = [];
  for (let i = 0; i < p.length; i++) {
    const P = p[i] as Point;
    const Q = p[(i + 1) % p.length] as Point;
    const dp = a * P[0] + b * P[1] - c;
    const dq = a * Q[0] + b * Q[1] - c;
    if (dp <= 0) out.push(P);
    if (dp <= 0 !== dq <= 0) {
      const t = dp / (dp - dq);
      out.push([P[0] + t * (Q[0] - P[0]), P[1] + t * (Q[1] - P[1])]);
    }
  }
  return out;
}

/** Shrinks a convex polygon by moving every edge inwards by `g`; null if nothing is left. */
export function inset(p: Poly, g: number): Poly | null {
  let q = p;
  for (let i = 0; i < p.length && q.length >= 3; i++) {
    const A = p[i] as Point;
    const B = p[(i + 1) % p.length] as Point;
    const dx = B[0] - A[0];
    const dy = B[1] - A[1];
    const L = Math.hypot(dx, dy);
    if (L < 1e-6) continue;
    q = clip(q, dy, -dx, dy * A[0] - dx * A[1] - g * L);
  }
  return q.length >= 3 ? q : null;
}

/** True if the point lies inside the polygon. */
export function inside(p: Poly, x: number, y: number): boolean {
  let c = false;
  for (let i = 0, j = p.length - 1; i < p.length; j = i++) {
    const a = p[i] as Point;
    const b = p[j] as Point;
    if (a[1] > y !== b[1] > y && x < ((b[0] - a[0]) * (y - a[1])) / (b[1] - a[1]) + a[0]) c = !c;
  }
  return c;
}

/** Bounding box of a polygon. */
export function bbox(p: Poly): { x0: number; y0: number; x1: number; y1: number } {
  let x0 = Infinity;
  let y0 = Infinity;
  let x1 = -Infinity;
  let y1 = -Infinity;
  for (const [x, y] of p) {
    x0 = Math.min(x0, x);
    y0 = Math.min(y0, y);
    x1 = Math.max(x1, x);
    y1 = Math.max(y1, y);
  }
  return { x0, y0, x1, y1 };
}

/** A rectangle as a polygon with positive orientation. */
export function rectPoly(x: number, y: number, w: number, h: number): Poly {
  return [
    [x, y],
    [x + w, y],
    [x + w, y + h],
    [x, y + h],
  ];
}

/** Resamples a polygon to `n` points evenly along its outline, starting at its top-left. */
export function resample(p: Poly, n: number): Poly {
  if (p.length === 0) return [];
  let s = 0;
  p.forEach((v, i) => {
    const best = p[s] as Point;
    if (v[0] + v[1] < best[0] + best[1]) s = i;
  });
  const q = p.slice(s).concat(p.slice(0, s));
  const L = perimeter(q);
  const out: Poly = [];
  let seg = 0;
  let acc = 0;
  for (let k = 0; k < n; k++) {
    const d = (k / n) * L;
    while (seg < q.length) {
      const a = q[seg] as Point;
      const b = q[(seg + 1) % q.length] as Point;
      const l = Math.hypot(b[0] - a[0], b[1] - a[1]);
      if (acc + l >= d || seg === q.length - 1) {
        const t = l > 0 ? (d - acc) / l : 0;
        out.push([a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t]);
        break;
      }
      acc += l;
      seg++;
    }
  }
  return out;
}

/** Points per polygon during a morph. */
export const MORPH_POINTS = 56;

/** Resampled outlines by polygon: a morph draws the same start and end shapes every frame,
 * so each is resampled once (P10, P11). */
const resampled = new WeakMap<Poly, Poly>();

function morphPoints(p: Poly): Poly {
  let r = resampled.get(p);
  if (!r) {
    r = resample(p, MORPH_POINTS);
    resampled.set(p, r);
  }
  return r;
}

/** Interpolates between two polygons (resampled to the same point count). */
export function lerpPoly(a: Poly, b: Poly, t: number): Poly {
  const ra = morphPoints(a);
  const rb = morphPoints(b);
  return ra.map((v, i) => {
    const w = rb[i] ?? v;
    return [v[0] + (w[0] - v[0]) * t, v[1] + (w[1] - v[1]) * t];
  });
}

/** Stretches `p` by `1 + s` along the direction (`dx`, `dy`) and squeezes it by the same
 * factor across, around `c`, so the area stays the same: the squash of a moving blob. */
export function stretch(p: Poly, c: Point, dx: number, dy: number, s: number): Poly {
  const len = Math.hypot(dx, dy);
  if (len === 0 || s === 0) return p;
  const ux = dx / len;
  const uy = dy / len;
  const along = 1 + s;
  const across = 1 / along;
  return p.map(([x, y]): Point => {
    const rx = x - c[0];
    const ry = y - c[1];
    const a = (rx * ux + ry * uy) * along;
    const b = (-rx * uy + ry * ux) * across;
    return [c[0] + a * ux - b * uy, c[1] + a * uy + b * ux];
  });
}

/** Ease in and out (cubic). */
export function ease(t: number): number {
  return t < 0.5 ? 4 * t * t * t : 1 - Math.pow(-2 * t + 2, 3) / 2;
}

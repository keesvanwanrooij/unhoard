// The stage renderer: draws the living cell map or the treemap on a canvas.
//
// One relaxation step per frame keeps the cells alive: they grow while a scan streams in
// and breathe (±2%) at rest. With the "calm" setting or reduced motion the layout is
// solved once and the canvas only redraws on interaction. Transitions (zoom, map style)
// morph each cell from its previous outline. Colours come from CSS variables, so the
// light and dark themes both work.
import { bbox, centroid, ease, inset, inside, lerpPoly, perimeter, rectPoly, area, type Poly, type Point } from "./geometry";
import type { Spot } from "./navigate";
import { hash, makeLevel, relax, rescale, type Level } from "./powerLayout";
import { squarify } from "./treemap";
import { mark, measureSince } from "../perf/marks";

/** One cell to draw. */
export interface Cell {
  id: string;
  nodeId: number | null;
  kind: "node" | "hidden" | "smaller";
  name: string;
  value: number;
  /** Size text shown on the label. */
  sizeText: string;
  /** Share text shown on the label. */
  shareText: string;
  category: string;
  hasChildren: boolean;
  /** A hint of the cell's own children (second level), if loaded. */
  sub?: { id: string; value: number; category: string }[];
}

/** What the renderer reports back. */
export interface RendererEvents {
  hover: (cell: Cell | null, x: number, y: number) => void;
}

interface Theme {
  colors: Record<string, string>;
  label: string;
  sub: string;
  glow: number;
  fillInner: number;
  fillOuter: number;
  hatch: string;
  hidden: string;
  select: string;
  overlay: string;
}

function readTheme(): Theme {
  const s = getComputedStyle(document.documentElement);
  const v = (n: string, d: string): string => s.getPropertyValue(n).trim() || d;
  const cats = ["media", "apps", "downloads", "system", "documents", "developer", "cloud", "hidden", "other"];
  return {
    colors: Object.fromEntries(cats.map((c) => [c, v(`--c-${c}`, "#4f8fa8")])),
    label: v("--map-label", "#fff"),
    sub: v("--map-sub", "rgba(255,255,255,.7)"),
    glow: Number(v("--map-glow", "12")),
    fillInner: Number(v("--map-fill-inner", "0.6")),
    fillOuter: Number(v("--map-fill-outer", "0.2")),
    hatch: v("--map-hatch", "rgba(255,255,255,.2)"),
    hidden: v("--map-hidden", "#0b1a26"),
    select: v("--map-select", "#fff"),
    overlay: v("--map-overlay", "#5fe7ff"),
  };
}

function hexRgb(h: string): [number, number, number] {
  const m = /^#([0-9a-f]{6})$/i.exec(h);
  if (!m?.[1]) return [128, 128, 128];
  const n = parseInt(m[1], 16);
  return [(n >> 16) & 255, (n >> 8) & 255, n & 255];
}
function rgba(h: string, a: number): string {
  const [r, g, b] = hexRgb(h);
  return `rgba(${r},${g},${b},${a})`;
}
function lighten(h: string, f: number, a: number): string {
  const [r, g, b] = hexRgb(h);
  return `rgba(${Math.round(r + (255 - r) * f)},${Math.round(g + (255 - g) * f)},${Math.round(b + (255 - b) * f)},${a})`;
}

const GAP = 3.5;
/** Steps of the fast warm-up after new data. */
const WARM_STEPS = 30;
/** Steps after which a calm map stops relaxing. */
const SETTLE_STEPS = 60;

/** The canvas renderer of the stage. */
export class Renderer {
  private ctx: CanvasRenderingContext2D;
  private W = 0;
  private H = 0;
  private stagePoly: Poly = rectPoly(0, 0, 1, 1);
  private levels = new Map<string, Level>();
  private subLevels = new Map<string, Level>();
  private cells: Cell[] = [];
  private levelKey = "root";
  private mode: "cells" | "treemap" = "cells";
  private living = true;
  private scanning = false;
  private hovered: string | null = null;
  private selected: string | null = null;
  private highlighted: string | null = null;
  private query = "";
  private theme: Theme;
  private hatch: CanvasPattern | null = null;
  private trans: { from: Map<string, Poly>; start: number; dur: number; last: string | null } | null = null;
  private raf = 0;
  private dirty = true;
  private drawn: { cell: Cell; poly: Poly }[] = [];
  private subPolys = new Map<string, Map<string, Poly>>();
  private frameTimes: number[] = [];
  private frameCount = 0;
  private active = true;
  private newSubs = 0;
  private expanding: { poly: Poly; cell: Cell; start: number } | null = null;

  constructor(
    private canvas: HTMLCanvasElement,
    private events: RendererEvents,
  ) {
    const ctx = canvas.getContext("2d");
    if (!ctx) throw new Error("canvas 2d unavailable");
    this.ctx = ctx;
    this.theme = readTheme();
    this.makeHatch();
    this.raf = requestAnimationFrame(this.frame);
  }

  /** Stops the animation loop. */
  destroy(): void {
    cancelAnimationFrame(this.raf);
  }

  /** Re-reads the theme colours (after a light or dark switch). */
  refreshTheme(): void {
    this.theme = readTheme();
    this.makeHatch();
    this.dirty = true;
  }

  private makeHatch(): void {
    const c = document.createElement("canvas");
    c.width = c.height = 10;
    const g = c.getContext("2d");
    if (!g) return;
    g.strokeStyle = this.theme.hatch;
    g.lineWidth = 2;
    g.beginPath();
    g.moveTo(-2, 12);
    g.lineTo(12, -2);
    g.moveTo(8, 12);
    g.lineTo(12, 8);
    g.moveTo(-2, 2);
    g.lineTo(2, -2);
    g.stroke();
    this.hatch = this.ctx.createPattern(c, "repeat");
  }

  /** Sets the canvas size in CSS pixels. */
  resize(w: number, h: number): void {
    const dpr = window.devicePixelRatio || 1;
    this.W = Math.max(100, w);
    this.H = Math.max(100, h);
    this.canvas.width = Math.round(this.W * dpr);
    this.canvas.height = Math.round(this.H * dpr);
    this.canvas.style.width = `${this.W}px`;
    this.canvas.style.height = `${this.H}px`;
    this.ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
    const poly = rectPoly(12, 8, this.W - 24, this.H - 16);
    this.levels.forEach((l) => {
      rescale(l, poly);
    });
    this.stagePoly = poly;
    this.dirty = true;
  }

  /** Shows `cells` as the level `key` (the focused node). */
  setCells(key: string, cells: Cell[]): void {
    this.levelKey = key;
    this.cells = cells;
    const level = this.levels.get(key);
    if (level) {
      level.settled = false;
      level.steps = Math.min(level.steps, WARM_STEPS);
    }
    this.dirty = true;
  }

  /** Animation and state flags. */
  setFlags(f: { living: boolean; scanning: boolean; selected: string | null; query: string; active: boolean }): void {
    this.active = f.active;
    this.living = f.living && !window.matchMedia("(prefers-reduced-motion: reduce)").matches;
    this.scanning = f.scanning;
    this.selected = f.selected;
    this.query = f.query.toLowerCase();
    this.dirty = true;
  }

  /** Switches between the cell map and the treemap with a morph. */
  setMode(mode: "cells" | "treemap"): void {
    if (mode === this.mode) return;
    const from = new Map(this.drawn.map((d) => [d.cell.id, d.poly]));
    this.mode = mode;
    this.trans = { from, start: performance.now(), dur: this.living ? 520 : 180, last: null };
    mark("unhoard:morph-start");
  }

  /** The second-level outlines of a cell, used as the start of a zoom-in. */
  subOutlines(cellId: string): Map<string, Poly> {
    return this.subPolys.get(cellId) ?? new Map<string, Poly>();
  }

  /** Starts the visible zoom at once on a click: the clicked cell expands towards the
   * stage while the next level loads, so the response never waits for data. */
  beginZoom(cellId: string): void {
    const d = this.drawn.find((x) => x.cell.id === cellId);
    if (d) this.expanding = { poly: d.poly, cell: d.cell, start: performance.now() };
  }

  /** Starts a zoom: cells morph from `from` (cells not in it grow from their centre). */
  zoom(from: Map<string, Poly>, last: string | null): void {
    this.expanding = null;
    mark("unhoard:zoom-start");
    this.trans = { from, start: performance.now(), dur: this.living ? 520 : 180, last };
  }

  /** The full stage outline, the start of a zoom-out. */
  stageOutline(): Poly {
    return this.stagePoly;
  }

  /** The cell at a point. */
  hit(x: number, y: number): Cell | null {
    for (const d of this.drawn) if (inside(d.poly, x, y)) return d.cell;
    return null;
  }

  /** The centres of the drawn cells that can be selected, largest first, for the keyboard. */
  spots(): (Spot & { cell: Cell })[] {
    return this.drawn.filter((d) => d.cell.nodeId !== null).map((d) => ({ id: d.cell.id, at: centroid(d.poly), cell: d.cell }));
  }

  /** Marks the cell of the item under the pointer in the list, drawn like a selection. */
  setHighlight(id: string | null): void {
    if (id !== this.highlighted) {
      this.highlighted = id;
      this.dirty = true;
    }
  }

  /** Marks a hovered cell. */
  setHover(id: string | null): void {
    if (id !== this.hovered) {
      this.hovered = id;
      this.dirty = true;
    }
  }

  /** Recent frame durations in milliseconds, for the performance checks. */
  frameDurations(): number[] {
    return this.frameTimes.slice();
  }

  private level(): Level {
    let l = this.levels.get(this.levelKey);
    if (!l) {
      l = makeLevel(this.stagePoly, this.levelKey);
      this.levels.set(this.levelKey, l);
    }
    return l;
  }

  private entries(t: number): { id: string; value: number }[] {
    const breathe = this.living && !this.scanning && this.mode === "cells";
    return this.cells
      .filter((c) => c.value > 0)
      .map((c) => ({
        id: c.id,
        value: breathe ? c.value * (1 + 0.02 * Math.sin(t * 1.1 + (hash(c.id) % 1000) / 159)) : c.value,
      }));
  }

  private frame = (now: number): void => {
    this.raf = requestAnimationFrame(this.frame);
    if (!this.active) return;
    const animating = this.living || this.scanning || this.trans !== null || this.expanding !== null;
    if (!animating && !this.dirty) return;
    const started = performance.now();
    this.dirty = false;
    this.draw(now);
    this.frameTimes.push(performance.now() - started);
    if (this.frameTimes.length > 240) this.frameTimes.shift();
  };

  private draw(now: number): void {
    const t = now / 1000;
    const ctx = this.ctx;
    ctx.clearRect(0, 0, this.W, this.H);
    this.overlay(t, true);
    const entries = this.entries(t);
    const level = this.level();
    // With many cells, relax every other frame: effects give way before frames drop (AC2.9).
    this.frameCount++;
    // Convergence is spread over frames (a few steps each) so no frame becomes a long task.
    const skip = entries.length > 150 && level.steps >= WARM_STEPS && this.frameCount % 2 === 1;
    if (!level.settled && !skip) {
      // While a scan streams in, one step per frame lets cells glide to their new size
      // instead of jumping at every update.
      const n = this.scanning ? 1 : level.steps < WARM_STEPS ? 3 : 1;
      relax(level, entries, n);
      level.steps += n;
      if (!this.living && !this.scanning && level.steps >= SETTLE_STEPS) level.settled = true;
    }
    if (this.living || this.scanning) level.settled = false;
    this.dirty = this.dirty || !level.settled;
    const rects = this.mode === "treemap" ? this.treemapRects(entries) : null;
    let tr: number | null = null;
    if (this.trans) {
      const k = (now - this.trans.start) / this.trans.dur;
      if (k >= 1) {
        this.trans = null;
        measureSince("unhoard:zoom-settle", "unhoard:zoom-start");
      } else {
        tr = ease(Math.max(0, k));
        measureSince("unhoard:zoom-response", "unhoard:zoom-click");
        measureSince("unhoard:morph-response", "unhoard:morph-start");
      }
    }
    const byId = new Map(this.cells.map((c) => [c.id, c]));
    const order = this.trans?.last
      ? entries.filter((e) => e.id !== this.trans?.last).concat(entries.filter((e) => e.id === this.trans?.last))
      : entries;
    const drawn: { cell: Cell; poly: Poly }[] = [];
    for (const e of order) {
      const cell = byId.get(e.id);
      if (!cell) continue;
      const target = this.mode === "cells" ? (level.sites.get(e.id)?.cell ?? null) : (rects?.get(e.id) ?? null);
      if (!target) continue;
      if (tr !== null && this.trans) {
        const src = this.trans.from.get(e.id) ?? pointPoly(centroid(target));
        this.drawCell(lerpPoly(src, target, tr), cell, "morph", 0, this.trans.from.has(e.id) ? 1 : Math.min(1, tr * 1.6));
        continue;
      }
      drawn.push({ cell, poly: target });
    }
    if (tr === null) {
      this.newSubs = 0;
      this.subPolys.clear();
      for (const d of drawn) {
        const info = this.drawCell(d.poly, d.cell, this.mode === "cells" ? "cells" : "rect", 0, 1);
        if (info && d.cell.sub && d.cell.sub.length > 1 && info.area > this.W * this.H * 0.03) this.drawSub(d, t);
      }
      for (const d of drawn) this.drawLabel(d.poly, d.cell);
      this.drawn = drawn;
      if (this.expanding) {
        const k = Math.min(1, (now - this.expanding.start) / 300);
        this.drawCell(lerpPoly(this.expanding.poly, this.stagePoly, ease(k) * 0.35), this.expanding.cell, "morph", 0, 0.9);
        measureSince("unhoard:zoom-response", "unhoard:zoom-click");
        if (k >= 1) this.expanding = null;
      }
    } else {
      this.drawn = [];
    }
    this.overlay(t, false);
  }

  private treemapRects(entries: { id: string; value: number }[]): Map<string, Poly> {
    const b = bbox(this.stagePoly);
    return squarify(entries, b.x0, b.y0, b.x1 - b.x0, b.y1 - b.y0);
  }

  private drawSub(d: { cell: Cell; poly: Poly }, t: number): void {
    const sub = d.cell.sub ?? [];
    const entries = sub.map((s) => ({
      id: s.id,
      value: this.living && !this.scanning ? s.value * (1 + 0.02 * Math.sin(t * 1.3 + (hash(s.id) % 997) / 151)) : s.value,
    }));
    const out = new Map<string, Poly>();
    if (this.mode === "cells") {
      const container = inset(d.poly, GAP * 2.6);
      if (!container) return;
      let l = this.subLevels.get(d.cell.id);
      if (!l) {
        // At most two new sub-levels per frame, so a zoom never builds them all at once.
        if (this.newSubs >= 2) return;
        this.newSubs++;
        l = makeLevel(container, `sub${d.cell.id}`);
        this.subLevels.set(d.cell.id, l);
        relax(l, entries, 6);
      }
      l.poly = container;
      relax(l, entries, this.living || this.scanning ? 1 : 0);
      for (const s of sub) {
        const p = l.sites.get(s.id)?.cell;
        if (p) {
          this.drawCell(p, { ...d.cell, id: s.id, category: s.category, sub: [] }, "cells", 1, 0.55);
          out.set(s.id, p);
        }
      }
    } else {
      const b = bbox(d.poly);
      if (b.y1 - b.y0 < 70) return;
      const rr = squarify(entries, b.x0 + 10, b.y0 + 34, b.x1 - b.x0 - 20, b.y1 - b.y0 - 44);
      for (const s of sub) {
        const p = rr.get(s.id);
        if (p) {
          this.drawCell(p, { ...d.cell, id: s.id, category: s.category, sub: [] }, "rect", 1, 0.5);
          out.set(s.id, p);
        }
      }
    }
    this.subPolys.set(d.cell.id, out);
  }

  private path(shape: Poly, kind: "cells" | "rect" | "morph"): void {
    const ctx = this.ctx;
    ctx.beginPath();
    if (kind === "cells") {
      const n = shape.length;
      const mid = (a: Point, b: Point): Point => [(a[0] + b[0]) / 2, (a[1] + b[1]) / 2];
      const first = mid(shape[n - 1] as Point, shape[0] as Point);
      ctx.moveTo(first[0], first[1]);
      for (let i = 0; i < n; i++) {
        const p = shape[i] as Point;
        const q = mid(p, shape[(i + 1) % n] as Point);
        ctx.quadraticCurveTo(p[0], p[1], q[0], q[1]);
      }
    } else if (kind === "rect") {
      const b = bbox(shape);
      ctx.roundRect(b.x0, b.y0, b.x1 - b.x0, b.y1 - b.y0, 3);
    } else {
      shape.forEach(([x, y], i) => {
        if (i === 0) ctx.moveTo(x, y);
        else ctx.lineTo(x, y);
      });
    }
    ctx.closePath();
  }

  private drawCell(poly: Poly, cell: Cell, kind: "cells" | "rect" | "morph", depth: number, alpha: number): { area: number } | null {
    const gap = depth ? GAP * 0.7 : GAP;
    let shape: Poly | null = poly;
    if (kind === "cells") shape = inset(poly, gap);
    if (kind === "rect") {
      const b = bbox(poly);
      if (b.x1 - b.x0 < gap * 2 + 1 || b.y1 - b.y0 < gap * 2 + 1) return null;
      shape = rectPoly(b.x0 + gap, b.y0 + gap, b.x1 - b.x0 - gap * 2, b.y1 - b.y0 - gap * 2);
    }
    if (!shape) return null;
    const ctx = this.ctx;
    const th = this.theme;
    const col = th.colors[cell.category] ?? th.colors["other"] ?? "#4f8fa8";
    const c = centroid(shape);
    const A = Math.abs(area(shape));
    const r = Math.sqrt(A) * 0.75;
    const sel = depth === 0 && (this.selected === cell.id || this.highlighted === cell.id);
    const hov = depth === 0 && this.hovered === cell.id;
    const dim = this.query !== "" && depth === 0 && !cell.name.toLowerCase().includes(this.query);
    ctx.save();
    ctx.globalAlpha = alpha * (dim ? 0.22 : 1);
    this.path(shape, kind);
    if (cell.kind === "hidden" && depth === 0) {
      ctx.fillStyle = th.hidden;
      ctx.fill();
      if (this.hatch) {
        ctx.fillStyle = this.hatch;
        ctx.fill();
      }
    } else if (depth > 0) {
      // Second-level cells are many and small: a flat fill keeps frames cheap.
      ctx.fillStyle = rgba(col, (th.fillInner + th.fillOuter) / 2);
      ctx.fill();
    } else {
      const g = ctx.createRadialGradient(c[0] - r * 0.25, c[1] - r * 0.3, 0, c[0], c[1], r * 1.25);
      g.addColorStop(0, lighten(col, 0.45, th.fillInner));
      g.addColorStop(0.55, rgba(col, (th.fillInner + th.fillOuter) / 2));
      g.addColorStop(1, rgba(col, th.fillOuter));
      ctx.fillStyle = g;
      ctx.fill();
    }
    ctx.lineWidth = sel ? 2.4 : hov ? 2 : depth ? 0.9 : 1.3;
    ctx.strokeStyle = sel ? th.select : hov ? lighten(col, 0.6, 1) : rgba(col, 0.95);
    if (depth === 0 && th.glow > 0 && (hov || sel || this.cells.length <= 60)) {
      ctx.shadowColor = rgba(col, hov || sel ? 0.95 : 0.6);
      ctx.shadowBlur = hov || sel ? th.glow * 1.8 : th.glow * 0.6;
    }
    ctx.stroke();
    ctx.restore();
    return { area: A };
  }

  private drawLabel(poly: Poly, cell: Cell): void {
    const shape = this.mode === "cells" ? inset(poly, GAP) : poly;
    if (!shape) return;
    const A = Math.abs(area(shape));
    const rIn = (2 * A) / perimeter(shape);
    if (rIn < 22) return;
    const ctx = this.ctx;
    ctx.save();
    ctx.globalAlpha = this.query !== "" && !cell.name.toLowerCase().includes(this.query) ? 0.3 : 1;
    ctx.fillStyle = this.theme.label;
    ctx.font = "700 14px Rajdhani, 'Segoe UI', sans-serif";
    ctx.shadowColor = "rgba(0,0,0,.45)";
    ctx.shadowBlur = 6;
    let x: number;
    let y: number;
    let maxW: number;
    if (this.mode === "treemap") {
      const b = bbox(shape);
      x = b.x0 + 10;
      y = b.y0 + 20;
      maxW = b.x1 - b.x0 - 18;
      ctx.textAlign = "left";
    } else {
      [x, y] = centroid(shape);
      maxW = rIn * 2.1;
      ctx.textAlign = "center";
    }
    let name = cell.name;
    while (name.length > 3 && ctx.measureText(name).width > maxW) name = `${name.slice(0, -2)}…`;
    if (ctx.measureText(name).width <= maxW) ctx.fillText(name, x, this.mode === "treemap" ? y : y - 2);
    if (rIn > 30) {
      ctx.font = "400 11px 'JetBrains Mono', Consolas, monospace";
      ctx.fillStyle = this.theme.sub;
      ctx.fillText(`${cell.sizeText} · ${cell.shareText}`, x, this.mode === "treemap" ? y + 15 : y + 14);
    }
    ctx.restore();
  }

  /** HUD rings and a radar sweep, fast during a scan and slow at rest. */
  private overlay(t: number, under: boolean): void {
    if (under) return;
    const ctx = this.ctx;
    const col = this.theme.overlay;
    const { W, H } = this;
    const cx = W / 2;
    const cy = H / 2;
    const R = Math.hypot(W, H) / 2;
    ctx.save();
    ctx.globalCompositeOperation = "lighter";
    ctx.strokeStyle = rgba(col, 0.06);
    ctx.lineWidth = 1;
    for (let r = 80; r < R; r += 90) {
      ctx.beginPath();
      ctx.arc(cx, cy, r, 0, Math.PI * 2);
      ctx.stroke();
    }
    const moving = this.living || this.scanning;
    const ang = moving ? t * (this.scanning ? 2.4 : 0.35) : -Math.PI / 2;
    if (this.scanning) {
      // The sweep's trailing glow is a full-canvas fill, so it only runs during a scan.
      const g = ctx.createConicGradient(ang - 0.9, cx, cy);
      g.addColorStop(0, rgba(col, 0));
      g.addColorStop(0.14, rgba(col, 0.16));
      g.addColorStop(0.145, rgba(col, 0));
      ctx.fillStyle = g;
      ctx.fillRect(0, 0, W, H);
    }
    ctx.strokeStyle = rgba(col, this.scanning ? 0.5 : 0.15);
    ctx.beginPath();
    ctx.moveTo(cx, cy);
    ctx.lineTo(cx + Math.cos(ang) * R, cy + Math.sin(ang) * R);
    ctx.stroke();
    for (let k = 0; k < 72; k++) {
      const a = (k / 72) * Math.PI * 2;
      const r0 = Math.min(W, H) * 0.49;
      const r1 = r0 - (k % 6 ? 4 : 10);
      ctx.beginPath();
      ctx.moveTo(cx + Math.cos(a) * r0, cy + Math.sin(a) * r0);
      ctx.lineTo(cx + Math.cos(a) * r1, cy + Math.sin(a) * r1);
      ctx.strokeStyle = rgba(col, 0.1);
      ctx.stroke();
    }
    ctx.restore();
  }
}

function pointPoly(c: Point): Poly {
  return Array.from({ length: 8 }, (): Point => [c[0], c[1]]);
}

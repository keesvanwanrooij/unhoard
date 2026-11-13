// The map on the page: hands the canvas to the map's own thread (mapWorker.ts) and keeps a
// copy of what was drawn, so hovering, clicking, the keyboard and zoom starts are answered
// here without waiting for the worker. Where a browser cannot move a canvas to a worker,
// the same renderer runs on the page instead.
import { centroid, inside, rectPoly, type Poly } from "./geometry";
import type { FromRenderer } from "./mapWorker";
import type { Spot } from "./navigate";
import { Renderer, type Cell, type DrawnState, type RendererEvents, type Theme } from "./renderer";
import { mark, measureSince } from "../perf/marks";

/** The flags the page sets. */
export interface MapFlags {
  living: boolean;
  scanning: boolean;
  selected: string | null;
  query: string;
  active: boolean;
  reduced: boolean;
}

/** Messages the page sends to the worker. */
export type ToRenderer =
  | { type: "init"; canvas: OffscreenCanvas; theme: Theme }
  | { type: "destroy" }
  | { type: "theme"; theme: Theme }
  | { type: "resize"; w: number; h: number; dpr: number }
  | { type: "cells"; key: string; cells: Cell[] }
  | { type: "flags"; flags: MapFlags }
  | { type: "mode"; mode: "cells" | "treemap" }
  | { type: "beginZoom"; id: string }
  | { type: "zoom"; from: [string, Poly][]; last: string | null }
  | { type: "highlight"; id: string | null }
  | { type: "hover"; id: string | null }
  | { type: "poke"; id: string; strength: number }
  | { type: "grab"; id: string; x: number; y: number }
  | { type: "dragTo"; x: number; y: number }
  | { type: "drop" };

/** The map's colours from the page's CSS variables. */
export function readTheme(): Theme {
  const s = getComputedStyle(document.documentElement);
  const v = (n: string, d: string): string => s.getPropertyValue(n).trim() || d;
  const cats = ["media", "apps", "downloads", "system", "documents", "developer", "cloud", "hidden", "other", "growHigh", "grow", "same", "shrink", "nodata"];
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

/** The map: the renderer in its worker, or on the page where that is not possible. */
export class MapView {
  private worker: Worker | null = null;
  private local: Renderer | null = null;
  private cells = new Map<string, Cell>();
  private state: DrawnState = { drawn: [], subs: [], stage: rectPoly(0, 0, 1, 1) };
  private subs = new Map<string, Map<string, Poly>>();
  private frames: number[] = [];
  private mode: "cells" | "treemap" = "cells";
  private size = { w: 0, h: 0 };

  constructor(private canvas: HTMLCanvasElement) {
    const events: RendererEvents = {
      drawn: (s) => {
        this.receive(s);
      },
      frame: (ms) => {
        this.frames.push(ms);
        if (this.frames.length > 240) this.frames.shift();
      },
      measure: (name, start) => {
        measureSince(name, start);
      },
    };
    const theme = readTheme();
    if (typeof Worker === "function" && "transferControlToOffscreen" in canvas) {
      const offscreen = canvas.transferControlToOffscreen();
      this.worker = new Worker(new URL("./mapWorker.ts", import.meta.url), { type: "module" });
      this.worker.onmessage = (e: MessageEvent<FromRenderer>) => {
        const m = e.data;
        if (m.type === "drawn") events.drawn(m.state);
        else if (m.type === "frame") events.frame(m.ms);
        else events.measure(m.name, m.start);
      };
      this.post({ type: "init", canvas: offscreen, theme }, [offscreen]);
    } else {
      this.local = new Renderer(canvas, events, theme);
    }
  }

  private post(m: ToRenderer, transfer: Transferable[] = []): void {
    this.worker?.postMessage(m, transfer);
  }

  private receive(s: DrawnState): void {
    this.state = s;
    this.subs = new Map(s.subs.map(([id, list]) => [id, new Map(list)]));
  }

  /** Stops the map and its worker. */
  destroy(): void {
    this.post({ type: "destroy" });
    this.worker?.terminate();
    this.local?.destroy();
  }

  /** Reads the theme colours again (after a light or dark switch). */
  refreshTheme(): void {
    const theme = readTheme();
    this.post({ type: "theme", theme });
    this.local?.setTheme(theme);
  }

  /** Sets the size in CSS pixels. */
  resize(w: number, h: number): void {
    if (w === this.size.w && h === this.size.h) return;
    this.size = { w, h };
    const dpr = window.devicePixelRatio || 1;
    this.canvas.style.width = `${String(Math.max(100, w))}px`;
    this.canvas.style.height = `${String(Math.max(100, h))}px`;
    this.post({ type: "resize", w, h, dpr });
    this.local?.resize(w, h, dpr);
  }

  /** Shows `cells` as the level `key`. */
  setCells(key: string, cells: Cell[]): void {
    this.cells = new Map(cells.map((c) => [c.id, c]));
    this.post({ type: "cells", key, cells });
    this.local?.setCells(key, cells);
  }

  /** Animation and state flags. */
  setFlags(flags: MapFlags): void {
    this.post({ type: "flags", flags });
    this.local?.setFlags(flags);
  }

  /** Switches between the cell map and the treemap with a morph. */
  setMode(mode: "cells" | "treemap"): void {
    if (mode === this.mode) return;
    this.mode = mode;
    mark("unhoard:morph-start");
    this.post({ type: "mode", mode });
    this.local?.setMode(mode);
  }

  /** The second-level outlines of a cell, the start of a zoom-in. */
  subOutlines(cellId: string): Map<string, Poly> {
    return this.subs.get(cellId) ?? new Map<string, Poly>();
  }

  /** Starts the visible zoom at once on a click. */
  beginZoom(cellId: string): void {
    this.post({ type: "beginZoom", id: cellId });
    this.local?.beginZoom(cellId);
  }

  /** Starts a zoom from the outlines `from`. */
  zoom(from: Map<string, Poly>, last: string | null): void {
    mark("unhoard:zoom-start");
    this.post({ type: "zoom", from: [...from], last });
    this.local?.zoom(from, last);
  }

  /** The full stage outline, the start of a zoom-out. */
  stageOutline(): Poly {
    return this.state.stage;
  }

  /** The cell at a point, from the last frame drawn. */
  hit(x: number, y: number): Cell | null {
    for (const d of this.state.drawn) if (inside(d.poly, x, y)) return this.cells.get(d.id) ?? null;
    return null;
  }

  /** The centres of the drawn cells that can be selected, for the keyboard. */
  spots(): (Spot & { cell: Cell })[] {
    return this.state.drawn.flatMap((d) => {
      const cell = this.cells.get(d.id);
      return cell && cell.nodeId !== null ? [{ id: d.id, at: centroid(d.poly), cell }] : [];
    });
  }

  /** Marks the item under the pointer in the list. */
  setHighlight(id: string | null): void {
    this.post({ type: "highlight", id });
    this.local?.setHighlight(id);
  }

  /** Marks a hovered cell. */
  setHover(id: string | null): void {
    this.post({ type: "hover", id });
    this.local?.setHover(id);
  }

  /** Pokes a cell. */
  poke(id: string, strength = 1): void {
    this.post({ type: "poke", id, strength });
    this.local?.poke(id, strength);
  }

  /** Takes hold of a cell in the cell map; false where it cannot be held. */
  grab(id: string, x: number, y: number): boolean {
    if (this.mode !== "cells" || !this.state.drawn.some((d) => d.id === id)) return false;
    this.post({ type: "grab", id, x, y });
    return this.local ? this.local.grab(id, x, y) : true;
  }

  /** Moves the held cell. */
  dragTo(x: number, y: number): void {
    this.post({ type: "dragTo", x, y });
    this.local?.dragTo(x, y);
  }

  /** Lets the held cell go. */
  drop(): void {
    this.post({ type: "drop" });
    this.local?.drop();
  }

  /** Recent frame durations in milliseconds, for the performance checks. */
  frameDurations(): number[] {
    return this.frames.slice();
  }
}

/** Map views by canvas: a canvas can be handed to a worker only once, so a view outlives an
 * unmount that is followed at once by a mount of the same canvas (React's development
 * check does that). */
const views = new WeakMap<HTMLCanvasElement, { view: MapView; timer: number | null }>();

/** The view of `canvas`, made on first use. */
export function mapViewFor(canvas: HTMLCanvasElement): MapView {
  const entry = views.get(canvas);
  if (entry) {
    if (entry.timer !== null) clearTimeout(entry.timer);
    entry.timer = null;
    return entry.view;
  }
  const view = new MapView(canvas);
  views.set(canvas, { view, timer: null });
  return view;
}

/** Releases the view of `canvas` unless the canvas is mounted again right away. */
export function releaseMapView(canvas: HTMLCanvasElement): void {
  const entry = views.get(canvas);
  if (!entry) return;
  entry.timer = window.setTimeout(() => {
    entry.view.destroy();
    views.delete(canvas);
  }, 0);
}

// The stage: the living cell map or treemap of the focused folder, its breadcrumb, live
// readouts, tooltip, legend and the companion. It asks the core for one level at a time.
import { useEffect, useMemo, useRef, useState, type JSX } from "react";
import { useShallow } from "zustand/react/shallow";
import { useTranslation } from "react-i18next";

import { api } from "../api/ipc";
import type { ViewNode, ViewResponse } from "../api/types";
import { explainShort } from "../explain";
import { fmtBytes, fmtInt, fmtShare } from "../format";
import { groupSmall } from "../map/grouping";
import { nearestInDirection, type Direction } from "../map/navigate";
import { Renderer, type Cell } from "../map/renderer";
import { mark } from "../perf/marks";
import { useApp } from "../store";
import { Companion } from "./Companion";
import { Matrix } from "./Matrix";
import { ScanBar } from "./ScanBar";

declare global {
  interface Window {
    /** Frame durations of the stage, exposed to the performance scenarios. */
    __unhoardFrames?: () => number[];
  }
}

/** How many children a view asks for. */
const TOP = 120;
/** The arrow keys of the map. */
const ARROWS: Partial<Record<string, Direction>> = { ArrowLeft: "left", ArrowRight: "right", ArrowUp: "up", ArrowDown: "down" };
/** Category order of the legend. */
const LEGEND = ["media", "apps", "downloads", "system", "documents", "developer", "cloud", "hidden", "other"] as const;

/** How often the map and the details reload while a scan runs: often enough to watch the
 * drive fill in, calm enough that the cells glide instead of jump. */
export const SCAN_TICK_MS = 1500;

/** Loads the view of the focused node, again every tick while the scan runs. */
export function useView(): ViewResponse | null {
  const { scan, focus, measure, treeVersion } = useApp(useShallow((s) => ({ scan: s.scan, focus: s.focus, measure: s.settings.measure, treeVersion: s.treeVersion })));
  const [resp, setResp] = useState<ViewResponse | null>(null);
  const generation = scan?.generation;
  const scanning = scan ? !scan.done : false;
  useEffect(() => {
    if (generation === undefined) return;
    let alive = true;
    const load = (): void => {
      api
        .view(generation, focus, TOP, measure === "logical")
        .then((r) => {
          if (alive) setResp(r);
        })
        .catch(() => {
          // A stale or vanished node: fall back to the root of the current scan.
          if (alive) useApp.getState().setFocus(0);
        });
    };
    load();
    const timer = scanning ? window.setInterval(load, SCAN_TICK_MS) : undefined;
    return () => {
      alive = false;
      if (timer !== undefined) window.clearInterval(timer);
    };
  }, [generation, focus, measure, treeVersion, scanning]);
  return resp;
}

function valueOf(n: ViewNode, logical: boolean): number {
  return logical ? n.logical : n.onDisk;
}

/** Pixels the pointer moves while pressed before a press becomes a drag. */
const DRAG_START = 5;

/** The stage. */
export function Stage({ resp, active }: { resp: ViewResponse | null; active: boolean }): JSX.Element {
  const { t } = useTranslation();
  const { lang, settings, scan, selected, select, setFocus, query, focus, hovered } = useApp(useShallow((s) => ({ lang: s.lang, settings: s.settings, scan: s.scan, selected: s.selected, select: s.select, setFocus: s.setFocus, query: s.query, focus: s.focus, hovered: s.hovered })));
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const boxRef = useRef<HTMLDivElement>(null);
  const renderer = useRef<Renderer | null>(null);
  const prevFocus = useRef<{ id: number; cell: string } | null>(null);
  const [tip, setTip] = useState<{ cell: Cell; x: number; y: number } | null>(null);
  const press = useRef<{ id: string; x: number; y: number; dragging: boolean } | null>(null);
  // Set when a drag ends, so the click that follows it is ignored.
  const dragged = useRef(false);
  const [dragging, setDragging] = useState(false);
  const [subs, setSubs] = useState<Map<number, ViewNode[]>>(new Map());
  const logical = settings.measure === "logical";
  const scanning = scan ? !scan.done : false;

  useEffect(() => {
    const canvas = canvasRef.current;
    const box = boxRef.current;
    if (!canvas || !box) return;
    const r = new Renderer(canvas, {
      hover: () => undefined,
    });
    renderer.current = r;
    // The performance scenarios read frame times in the fake-core build only.
    if (import.meta.env.MODE === "fake") window.__unhoardFrames = () => r.frameDurations();
    const ro = new ResizeObserver(() => {
      const b = box.getBoundingClientRect();
      r.resize(b.width, b.height);
    });
    ro.observe(box);
    const mq = window.matchMedia("(prefers-color-scheme: dark)");
    const onTheme = (): void => {
      r.refreshTheme();
    };
    mq.addEventListener("change", onTheme);
    return () => {
      ro.disconnect();
      mq.removeEventListener("change", onTheme);
      r.destroy();
      renderer.current = null;
    };
  }, []);

  // Second level: a hint of the children of the largest folders, once the scan is done.
  useEffect(() => {
    if (!resp || scanning) return;
    const total = resp.view.node.onDisk || 1;
    const big = resp.view.children.filter((c) => c.hasChildren && valueOf(c, logical) / total > 0.03).slice(0, 10);
    let alive = true;
    Promise.all(big.map((c) => api.view(resp.generation, c.id, 16, logical).then((v) => [c.id, v.view.children] as const)))
      .then((pairs) => {
        if (alive) setSubs(new Map(pairs));
      })
      .catch(() => {
        if (alive) setSubs(new Map());
      });
    return () => {
      alive = false;
    };
  }, [resp, scanning, logical]);

  const cells = useMemo((): Cell[] => {
    if (!resp) return [];
    const v = resp.view;
    const hidden = resp.hiddenBytes ?? 0;
    const total = valueOf(v.node, logical) + hidden || 1;
    const items = v.children.filter((c) => !c.unscanned).map((c) => ({ id: `n${c.id}`, value: valueOf(c, logical), node: c }));
    const othersValue = logical ? v.others.logical : v.others.onDisk;
    const grouped = groupSmall(items, total);
    const out: Cell[] = [];
    let smallerCount = v.others.count;
    let smallerValue = othersValue;
    for (const g of grouped) {
      if ("node" in g) {
        const n = (g as { node: ViewNode }).node;
        out.push({
          id: g.id,
          nodeId: n.id,
          kind: "node",
          name: n.name,
          value: g.value,
          sizeText: fmtBytes(g.value, lang),
          shareText: fmtShare(g.value / total, lang),
          hint: explainShort(n, t),
          category: n.category,
          hasChildren: n.hasChildren,
          sub: (subs.get(n.id) ?? []).map((s) => ({ id: `s${s.id}`, value: valueOf(s, logical), category: s.category })).filter((s) => s.value > 0),
        });
      } else {
        smallerCount += g.count ?? 0;
        smallerValue += g.value;
      }
    }
    if (smallerValue > 0) {
      out.push({
        id: "smaller",
        nodeId: null,
        kind: "smaller",
        name: t("map.smaller", { count: smallerCount }),
        value: smallerValue,
        sizeText: fmtBytes(smallerValue, lang),
        shareText: fmtShare(smallerValue / total, lang),
        category: "other",
        hasChildren: false,
      });
    }
    if (hidden > 0) {
      out.push({
        id: "hidden",
        nodeId: -1,
        kind: "hidden",
        name: t("map.hiddenSpace"),
        value: hidden,
        sizeText: fmtBytes(hidden, lang),
        shareText: fmtShare(hidden / total, lang),
        category: "hidden",
        hasChildren: false,
      });
    }
    return out;
  }, [resp, logical, lang, t, subs]);

  useEffect(() => {
    const r = renderer.current;
    if (!r || !resp) return;
    const key = `g${resp.generation}-${resp.view.node.id}`;
    const prev = prevFocus.current;
    if (prev && prev.id !== resp.view.node.id) {
      const goingDown = resp.view.breadcrumb.some((c) => c.id === prev.id);
      if (goingDown) {
        // Zoom in: the child cells grow out of the outlines they had inside their parent.
        const from = new Map([...r.subOutlines(`n${resp.view.node.id}`)].map(([k, p]) => [k.replace(/^s/, "n"), p]));
        r.zoom(from, null);
      } else {
        // Zoom out: the folder we leave shrinks from the full stage into its place.
        r.zoom(new Map([[`n${prev.id}`, r.stageOutline()]]), `n${prev.id}`);
      }
    }
    prevFocus.current = { id: resp.view.node.id, cell: key };
    r.setCells(key, cells);
  }, [cells, resp]);

  const matrix = settings.mapStyle === "matrix";
  useEffect(() => {
    renderer.current?.setFlags({
      living: settings.motion === "living",
      scanning,
      selected: selected === null ? null : selected === -1 ? "hidden" : `n${selected}`,
      query,
      // The map draws nothing while the matrix shows; it keeps its layout for coming back.
      active: active && !matrix,
    });
  }, [settings.motion, scanning, selected, query, active, matrix]);

  useEffect(() => {
    if (settings.mapStyle !== "matrix") renderer.current?.setMode(settings.mapStyle);
  }, [settings.mapStyle]);

  useEffect(() => {
    renderer.current?.setHighlight(hovered === null ? null : hovered === -1 ? "hidden" : `n${String(hovered)}`);
  }, [hovered]);

  const onMove = (e: React.MouseEvent<HTMLCanvasElement>): void => {
    const r = renderer.current;
    if (!r || press.current?.dragging) return;
    const b = e.currentTarget.getBoundingClientRect();
    const x = e.clientX - b.left;
    const y = e.clientY - b.top;
    const cell = r.hit(x, y);
    // Touching a new cell with the pointer makes it jiggle a little.
    if (cell && tip?.cell.id !== cell.id) r.poke(cell.id, 0.35);
    r.setHover(cell?.id ?? null);
    setTip(cell ? { cell, x: Math.min(x + 16, b.width - 250), y: Math.max(8, y - 12) } : null);
  };

  /** Zooms into a folder cell, or returns false when the cell cannot be zoomed into. */
  const zoomInto = (cell: Cell): boolean => {
    const r = renderer.current;
    if (!r || cell.kind !== "node" || !cell.hasChildren || scanning || cell.nodeId === null) return false;
    mark("unhoard:zoom-click");
    r.beginZoom(cell.id);
    // Paint the start of the zoom first; the next level loads after that frame.
    const target = cell.nodeId;
    requestAnimationFrame(() => {
      setFocus(target);
    });
    return true;
  };

  // A press pokes the cell; moving the pointer while pressed drags it. A drag is not a
  // click, so letting go neither selects nor zooms.
  const onPointerDown = (e: React.PointerEvent<HTMLCanvasElement>): void => {
    const r = renderer.current;
    if (!r || e.button !== 0) return;
    const b = e.currentTarget.getBoundingClientRect();
    const cell = r.hit(e.clientX - b.left, e.clientY - b.top);
    if (!cell) return;
    r.poke(cell.id);
    press.current = { id: cell.id, x: e.clientX, y: e.clientY, dragging: false };
  };
  const onPointerMove = (e: React.PointerEvent<HTMLCanvasElement>): void => {
    const r = renderer.current;
    const p = press.current;
    if (!r || !p) return;
    const b = e.currentTarget.getBoundingClientRect();
    const x = e.clientX - b.left;
    const y = e.clientY - b.top;
    if (!p.dragging) {
      if (Math.hypot(e.clientX - p.x, e.clientY - p.y) < DRAG_START || !r.grab(p.id, x, y)) return;
      p.dragging = true;
      e.currentTarget.setPointerCapture(e.pointerId);
      r.setHover(p.id);
      setTip(null);
      setDragging(true);
    }
    r.dragTo(x, y);
  };
  const onPointerUp = (): void => {
    const p = press.current;
    press.current = null;
    if (!p?.dragging) return;
    renderer.current?.drop();
    dragged.current = true;
    setDragging(false);
  };

  const onClick = (e: React.MouseEvent<HTMLCanvasElement>): void => {
    const r = renderer.current;
    if (dragged.current) {
      dragged.current = false;
      return;
    }
    if (!r) return;
    const b = e.currentTarget.getBoundingClientRect();
    const cell = r.hit(e.clientX - b.left, e.clientY - b.top);
    if (!cell || cell.nodeId === null) return;
    if (!zoomInto(cell)) select(cell.nodeId);
  };

  // Arrow keys select the nearest cell in that direction, Enter zooms in, and the Menu
  // key or Shift+F10 moves focus to the actions of the selected item (AC2.8). Backspace
  // (one level up) is handled for the whole overview.
  const onKey = (e: React.KeyboardEvent<HTMLCanvasElement>): void => {
    const r = renderer.current;
    if (!r) return;
    const spots = r.spots();
    const current = selected === null ? null : selected === -1 ? "hidden" : `n${String(selected)}`;
    const dir = ARROWS[e.key];
    if (dir) {
      e.preventDefault();
      const target = nearestInDirection(spots, current, dir);
      const nodeId = spots.find((s) => s.id === target)?.cell.nodeId;
      if (typeof nodeId === "number") select(nodeId);
    } else if (e.key === "Enter") {
      const cell = spots.find((s) => s.id === current)?.cell;
      if (cell) zoomInto(cell);
    } else if (e.key === "ContextMenu" || (e.key === "F10" && e.shiftKey)) {
      e.preventDefault();
      document.querySelector<HTMLButtonElement>(".details .btns button")?.focus();
    }
  };

  const crumbs = resp?.view.breadcrumb ?? [];
  const files = scan?.files ?? 0;
  return (
    <div className="stage-wrap">
      <ScanBar />
      <nav className="crumbs" aria-label={t("details.path")}>
        {crumbs.map((c, i) =>
          i < crumbs.length - 1 ? (
            <span key={c.id}>
              <button
                onClick={() => {
                  setFocus(c.id);
                }}
              >
                {c.name}
              </button>
              <span aria-hidden> › </span>
            </span>
          ) : (
            <span key={c.id}>{c.name}</span>
          ),
        )}
        {scan?.partial ? <span className="partial">{t("top.partial")}</span> : null}
      </nav>
      <div className="readouts" aria-live="off">
        <div>
          {t("map.readouts.files")}
          <b>{fmtInt(files, lang)}</b>
        </div>
        <div>
          {t("map.readouts.scanned")}
          <b>{fmtBytes(scan?.onDisk ?? 0, lang)}</b>
        </div>
        <div>
          {t("map.readouts.hidden")}
          <b>{resp?.hiddenBytes !== null && resp?.hiddenBytes !== undefined && focus === 0 ? fmtBytes(resp.hiddenBytes, lang) : "–"}</b>
        </div>
      </div>
      <div className="stage" ref={boxRef}>
        <canvas
          ref={canvasRef}
          onMouseMove={onMove}
          onPointerDown={onPointerDown}
          onPointerMove={onPointerMove}
          onPointerUp={onPointerUp}
          onPointerCancel={onPointerUp}
          onMouseLeave={() => {
            renderer.current?.setHover(null);
            setTip(null);
          }}
          onClick={onClick}
          onKeyDown={onKey}
          tabIndex={0}
          role="application"
          aria-label={t("map.keys")}
          style={{ cursor: dragging ? "grabbing" : tip ? "pointer" : "default", visibility: matrix ? "hidden" : "visible" }}
        />
        {matrix ? <Matrix active={active} /> : null}
        {tip && !matrix ? (
          <div className="tooltip" style={{ left: tip.x, top: tip.y }}>
            <b>{tip.cell.name}</b>
            <span>
              {tip.cell.sizeText} · {tip.cell.shareText}
            </span>
            {tip.cell.hint ? <span>{tip.cell.hint}</span> : null}
          </div>
        ) : null}
        {!scan ? <div className="empty-stage">{t("map.empty")}</div> : null}
      </div>
      <div className="legend">
        {LEGEND.map((c) => (
          <span key={c}>
            <i style={{ background: `var(--c-${c})` }} />
            {t(`category.${c}`)}
          </span>
        ))}
      </div>
      <Companion />
    </div>
  );
}

// The priority matrix (AC2.10): every suggestion as a bubble. x = days since last use (log),
// y = size on disk (log), bubble size = size on disk (log), colour = kind of content.
// Top right holds big items unused for long: the first to look at. Zoom with the wheel or
// the buttons and drag to pan, so crowded bubbles come apart; bubbles keep their size.
// Hover and click work as on the map: the item is outlined and selected for the details.
import { useEffect, useMemo, useRef, useState, type JSX } from "react";
import { useShallow } from "zustand/react/shallow";
import { useTranslation } from "react-i18next";

import { api } from "../api/ipc";
import type { MatrixPoint } from "../api/types";
import { fmtAgo, fmtBytes } from "../format";
import { AGE_TICKS, MIN_BYTES, ageScale, ageTick, jitterOf, sizeRadius, sizeScale, zoomAt, type Zoom } from "../map/matrix";
import { useApp } from "../store";

/** Room for the axis labels around the plot, and the strip left of it for items of unknown age. */
const PAD = { left: 70, right: 24, top: 38, bottom: 40 };
const UNKNOWN_STRIP = 60;
const STRIP_GAP = 8;
/** Bubbles in the unknown strip stay this small, so they fit its width. */
const UNKNOWN_R = 12;
/** Margin inside the plot, so the biggest bubbles at the edges show whole. */
const INSET = 24;
/** Size ticks on the y axis. */
const TICKS = [2 ** 20, 10 * 2 ** 20, 100 * 2 ** 20, 2 ** 30, 10 * 2 ** 30, 100 * 2 ** 30, 2 ** 40];
const FIT: Zoom = { k: 1, tx: 0, ty: 0 };
/** Zoom factor of one wheel notch or button press. */
const STEP = 1.5;

/** The matrix, filling the stage. */
export function Matrix({ active }: { active: boolean }): JSX.Element {
  const { t } = useTranslation();
  const { lang, scan, treeVersion, selected, hovered, select, hover } = useApp(
    useShallow((s) => ({ lang: s.lang, scan: s.scan, treeVersion: s.treeVersion, selected: s.selected, hovered: s.hovered, select: s.select, hover: s.hover })),
  );
  const boxRef = useRef<HTMLDivElement>(null);
  const drag = useRef<{ x: number; y: number; z: Zoom } | null>(null);
  const [size, setSize] = useState({ w: 800, h: 500 });
  // `now` is taken when the data arrives, so ages stay put between renders.
  const [loaded, setLoaded] = useState<{ key: string; points: MatrixPoint[]; now: number } | null>(null);
  const [zoom, setZoom] = useState<Zoom>(FIT);
  const [tip, setTip] = useState<{ p: MatrixPoint; x: number; y: number } | null>(null);
  const done = scan?.done ?? false;
  const key = `${String(scan?.generation)}-${String(treeVersion)}`;

  useEffect(() => {
    const box = boxRef.current;
    if (!box) return;
    const ro = new ResizeObserver(() => {
      const b = box.getBoundingClientRect();
      setSize({ w: Math.max(320, b.width), h: Math.max(240, b.height) });
    });
    ro.observe(box);
    return () => {
      ro.disconnect();
    };
  }, []);

  useEffect(() => {
    if (!active || !done) return;
    let alive = true;
    api
      .matrix(300)
      .then(([, points]) => {
        if (alive) setLoaded({ key, points, now: Date.now() / 1000 });
      })
      .catch(() => {
        if (alive) setLoaded({ key, points: [], now: 0 });
      });
    return () => {
      alive = false;
    };
  }, [active, done, key]);

  const points = loaded?.key === key ? loaded.points : null;
  const now = loaded?.now ?? 0;
  const plotX = PAD.left + UNKNOWN_STRIP + STRIP_GAP;
  const plotW = size.w - plotX - PAD.right;
  const plotH = size.h - PAD.top - PAD.bottom;
  const maxBytes = points?.reduce((m, p) => Math.max(m, p.onDisk), MIN_BYTES) ?? MIN_BYTES;
  const sx = (fx: number): number => plotX + (INSET + fx * (plotW - 2 * INSET)) * zoom.k + zoom.tx;
  const sy = (fy: number): number => PAD.top + (INSET + (1 - fy) * (plotH - 2 * INSET)) * zoom.k + zoom.ty;
  const placed = useMemo(
    () =>
      (points ?? [])
        .map((p) => ({
          p,
          fx: ageScale(p.lastUse?.at ?? null, now),
          fy: sizeScale(p.onDisk, maxBytes),
          // Items of unknown age sit in their own strip left of the plot, spread by ID.
          ux: PAD.left + UNKNOWN_R + jitterOf(p.id) * (UNKNOWN_STRIP - 2 * UNKNOWN_R),
          r: sizeRadius(p.onDisk, maxBytes),
        }))
        // Large bubbles first, so small ones stay on top and clickable.
        .sort((a, b) => b.r - a.r),
    [points, maxBytes, now],
  );

  const zoomBy = (factor: number, x = plotW / 2, y = plotH / 2): void => {
    setZoom((z) => {
      const n = zoomAt(z, factor, x, y);
      return n.k === 1 ? FIT : n;
    });
  };
  const label = (p: MatrixPoint): string => p.label ?? (p.ruleId ? t(`knownLocations.${p.ruleId}.title`) : p.name);
  const use = (p: MatrixPoint): string =>
    p.lastUse ? t(`matrix.use.${p.lastUse.source}`, { when: fmtAgo(p.lastUse.at, now, lang) }) : t("matrix.use.unknown");

  return (
    <div className="matrix" ref={boxRef}>
      {!done ? <div className="empty-stage">{t("matrix.waiting")}</div> : null}
      {done && points?.length === 0 ? <div className="empty-stage">{t("matrix.empty")}</div> : null}
      <svg
        width={size.w}
        height={size.h}
        role="img"
        aria-label={t("matrix.title")}
        onWheel={(e) => {
          const b = e.currentTarget.getBoundingClientRect();
          zoomBy(e.deltaY < 0 ? STEP : 1 / STEP, e.clientX - b.left - plotX, e.clientY - b.top - PAD.top);
        }}
      >
        <defs>
          <clipPath id="matrix-plot">
            <rect x={plotX} y={PAD.top} width={plotW} height={plotH} />
          </clipPath>
          <clipPath id="matrix-unknown">
            <rect x={PAD.left} y={PAD.top} width={UNKNOWN_STRIP} height={plotH} />
          </clipPath>
        </defs>
        <rect
          x={plotX}
          y={PAD.top}
          width={plotW}
          height={plotH}
          className={`plot${zoom.k > 1 ? " pannable" : ""}`}
          onPointerDown={(e) => {
            if (zoom.k <= 1) return;
            drag.current = { x: e.clientX, y: e.clientY, z: zoom };
            e.currentTarget.setPointerCapture(e.pointerId);
          }}
          onPointerMove={(e) => {
            const d = drag.current;
            if (!d) return;
            setZoom({ ...d.z, tx: d.z.tx + e.clientX - d.x, ty: d.z.ty + e.clientY - d.y });
          }}
          onPointerUp={() => {
            drag.current = null;
          }}
        />
        <rect x={PAD.left} y={PAD.top} width={UNKNOWN_STRIP} height={plotH} className="unknown-strip" />
        <text x={PAD.left + UNKNOWN_STRIP / 2} y={PAD.top + plotH + 16} className="axis-label" textAnchor="middle">
          {t("matrix.ageUnknown")}
        </text>
        {AGE_TICKS.map((days) => {
          const x = sx(ageTick(days));
          if (x < plotX - 1 || x > plotX + plotW + 1) return null;
          return (
            <g key={days}>
              <line x1={x} x2={x} y1={PAD.top} y2={PAD.top + plotH} className="tick" />
              {/* A label near the left edge starts at its tick, so it clears the unknown strip's label. */}
              <text x={x} y={PAD.top + plotH + 16} className="axis-label" textAnchor={x < plotX + 24 ? "start" : "middle"}>
                {t(`matrix.ages.${String(days)}`)}
              </text>
            </g>
          );
        })}
        {TICKS.filter((b) => b <= maxBytes * 1.01).map((b) => {
          const y = sy(sizeScale(b, maxBytes));
          if (y < PAD.top - 1 || y > PAD.top + plotH + 1) return null;
          return (
            <g key={b}>
              <line x1={PAD.left} x2={plotX + plotW} y1={y} y2={y} className="tick" />
              <text x={PAD.left - 8} y={y} className="axis-label" textAnchor="end" dominantBaseline="middle">
                {fmtBytes(b, lang)}
              </text>
            </g>
          );
        })}
        <text x={PAD.left} y={22} className="axis-title">
          {t("matrix.axisY")}
        </text>
        <text x={plotX + plotW} y={size.h - 6} className="axis-title" textAnchor="end">
          {t("matrix.axisX")}
        </text>
        <text x={plotX + plotW - 8} y={PAD.top + 16} className="quadrant win" textAnchor="end">
          {t("matrix.quadrant.bigOld")}
        </text>
        <text x={plotX + 8} y={PAD.top + 16} className="quadrant">
          {t("matrix.quadrant.bigRecent")}
        </text>
        {placed.map(({ p, fx, fy, ux, r }) => {
          const on = selected === p.id || hovered === p.id;
          const cx = fx === null ? ux : sx(fx);
          const cy = sy(fy);
          return (
            <g key={p.id} clipPath={fx === null ? "url(#matrix-unknown)" : "url(#matrix-plot)"}>
              <circle
                cx={cx}
                cy={cy}
                r={fx === null ? Math.min(r, UNKNOWN_R) : r}
                className={`bubble${on ? " on" : ""}${fx === null ? " unknown" : ""}`}
                style={{ fill: `var(--c-${p.category})`, stroke: on ? undefined : `var(--c-${p.category})` }}
                onMouseEnter={() => {
                  hover(p.id);
                  setTip({ p, x: Math.min(cx + r + 8, size.w - 260), y: Math.max(8, cy - 20) });
                }}
                onMouseLeave={() => {
                  hover(null);
                  setTip(null);
                }}
                onClick={() => {
                  select(p.id);
                }}
              />
            </g>
          );
        })}
      </svg>
      <div className="matrix-zoom" role="group" aria-label={t("matrix.zoom")}>
        <button
          className="btn small"
          aria-label={t("matrix.zoomIn")}
          onClick={() => {
            zoomBy(STEP);
          }}
        >
          +
        </button>
        <button
          className="btn small"
          aria-label={t("matrix.zoomOut")}
          onClick={() => {
            zoomBy(1 / STEP);
          }}
        >
          −
        </button>
        <button
          className="btn small"
          disabled={zoom.k === 1}
          onClick={() => {
            setZoom(FIT);
          }}
        >
          {t("matrix.zoomFit")}
        </button>
      </div>
      {tip ? (
        <div className="tooltip" style={{ left: tip.x, top: tip.y }}>
          <b>{label(tip.p)}</b>
          <span>{fmtBytes(tip.p.onDisk, lang)}</span>
          <span>{use(tip.p)}</span>
        </div>
      ) : null}
    </div>
  );
}

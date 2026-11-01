// The priority matrix (AC2.10): every suggestion as a bubble. x = size on disk (log), y =
// effort in four bands, bubble size = days since last use (log), colour = kind of content.
// Bottom right holds the quick wins, top right the big jobs. Hover and click work as on the
// map: the item is outlined and selected for the details panel.
import { useEffect, useMemo, useRef, useState, type JSX } from "react";
import { useShallow } from "zustand/react/shallow";
import { useTranslation } from "react-i18next";

import { api } from "../api/ipc";
import type { MatrixPoint } from "../api/types";
import { fmtAgo, fmtBytes } from "../format";
import { MIN_BYTES, ageRadius, effortY, jitterOf, sizeX } from "../map/matrix";
import { useApp } from "../store";

/** Room for the axis labels around the plot. */
const PAD = { left: 132, right: 24, top: 18, bottom: 42 };
/** The radius of a bubble whose last use is unknown. */
const R_UNKNOWN = 9;
/** Size ticks on the x axis. */
const TICKS = [2 ** 20, 10 * 2 ** 20, 100 * 2 ** 20, 2 ** 30, 10 * 2 ** 30, 100 * 2 ** 30, 2 ** 40];

/** The matrix, filling the stage. */
export function Matrix({ active }: { active: boolean }): JSX.Element {
  const { t } = useTranslation();
  const { lang, scan, treeVersion, selected, hovered, select, hover } = useApp(
    useShallow((s) => ({ lang: s.lang, scan: s.scan, treeVersion: s.treeVersion, selected: s.selected, hovered: s.hovered, select: s.select, hover: s.hover })),
  );
  const boxRef = useRef<HTMLDivElement>(null);
  const [size, setSize] = useState({ w: 800, h: 500 });
  // `now` is taken when the data arrives, so ages stay put between renders.
  const [loaded, setLoaded] = useState<{ key: string; points: MatrixPoint[]; now: number } | null>(null);
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
  const plotW = size.w - PAD.left - PAD.right;
  const plotH = size.h - PAD.top - PAD.bottom;
  const maxBytes = points?.reduce((m, p) => Math.max(m, p.onDisk), MIN_BYTES) ?? MIN_BYTES;
  const placed = useMemo(
    () =>
      (points ?? [])
        .map((p) => ({
          p,
          cx: PAD.left + sizeX(p.onDisk, maxBytes) * plotW,
          cy: PAD.top + (1 - effortY(p.effort, jitterOf(p.id))) * plotH,
          r: ageRadius(p.lastUse?.at ?? null, now) ?? R_UNKNOWN,
        }))
        // Large bubbles first, so small ones stay on top and clickable.
        .sort((a, b) => b.r - a.r),
    [points, maxBytes, plotW, plotH, now],
  );

  const label = (p: MatrixPoint): string => p.label ?? (p.ruleId ? t(`knownLocations.${p.ruleId}.title`) : p.name);
  const use = (p: MatrixPoint): string =>
    p.lastUse ? t(`matrix.use.${p.lastUse.source}`, { when: fmtAgo(p.lastUse.at, now, lang) }) : t("matrix.use.unknown");

  return (
    <div className="matrix" ref={boxRef}>
      {!done ? <div className="empty-stage">{t("matrix.waiting")}</div> : null}
      {done && points?.length === 0 ? <div className="empty-stage">{t("matrix.empty")}</div> : null}
      <svg width={size.w} height={size.h} role="img" aria-label={t("matrix.title")}>
        {[1, 2, 3, 4].map((e) => {
          const y0 = PAD.top + (1 - e / 4) * plotH;
          return (
            <g key={e}>
              <rect x={PAD.left} y={y0} width={plotW} height={plotH / 4} className={e % 2 ? "band" : "band alt"} />
              <text x={PAD.left - 10} y={y0 + plotH / 8} className="axis-label" textAnchor="end" dominantBaseline="middle">
                {t(`matrix.effort.${String(e)}`)}
              </text>
            </g>
          );
        })}
        {TICKS.filter((b) => b <= maxBytes * 1.01).map((b) => {
          const x = PAD.left + sizeX(b, maxBytes) * plotW;
          return (
            <g key={b}>
              <line x1={x} x2={x} y1={PAD.top} y2={PAD.top + plotH} className="tick" />
              <text x={x} y={PAD.top + plotH + 16} className="axis-label" textAnchor="middle">
                {fmtBytes(b, lang)}
              </text>
            </g>
          );
        })}
        <text x={PAD.left + plotW} y={size.h - 6} className="axis-title" textAnchor="end">
          {t("matrix.axisX")}
        </text>
        <text x={PAD.left + plotW - 8} y={PAD.top + plotH - 8} className="quadrant win" textAnchor="end">
          {t("matrix.quadrant.quickWins")}
        </text>
        <text x={PAD.left + plotW - 8} y={PAD.top + 16} className="quadrant" textAnchor="end">
          {t("matrix.quadrant.bigJobs")}
        </text>
        {placed.map(({ p, cx, cy, r }) => {
          const on = selected === p.id || hovered === p.id;
          return (
            <circle
              key={p.id}
              cx={cx}
              cy={cy}
              r={r}
              className={`bubble${on ? " on" : ""}${p.lastUse ? "" : " unknown"}`}
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
          );
        })}
      </svg>
      {tip ? (
        <div className="tooltip" style={{ left: tip.x, top: tip.y }}>
          <b>{label(tip.p)}</b>
          <span>
            {fmtBytes(tip.p.onDisk, lang)} · {t(`matrix.effort.${String(tip.p.effort)}`)}
          </span>
          <span>{use(tip.p)}</span>
        </div>
      ) : null}
      <div className="matrix-key">{t("matrix.key")}</div>
    </div>
  );
}

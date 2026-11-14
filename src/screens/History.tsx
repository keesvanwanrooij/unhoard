// Storage history (F17): the drive's used space over time, the period switch, and what grew
// or shrank, each with its explanation and a sparkline. A click on a grower selects it in
// the overview when the current scan has it.
import { startTransition, useEffect, useMemo, useState, type JSX } from "react";
import { useShallow } from "zustand/react/shallow";
import { useTranslation } from "react-i18next";

import { api } from "../api/ipc";
import type { GrowthItem, GrowthView, TimelinePoint } from "../api/types";
import { explainShort } from "../explain";
import { fmtBytes } from "../format";
import { useApp } from "../store";

/** The periods of the switch, in days. */
const PERIODS = [1, 7, 30, 90] as const;

/** A signed size: "+54 GB" or "−31 GB". */
export function fmtDelta(delta: number, lang: "en" | "nl"): string {
  return `${delta < 0 ? "−" : "+"}${fmtBytes(Math.abs(delta), lang)}`;
}

/** About `n` round values from `lo` to `hi` for axis ticks: steps of 1, 2 or 5 times a power
 * of ten. */
export function niceTicks(lo: number, hi: number, n: number): number[] {
  if (!(hi > lo) || n < 1) return [lo];
  const raw = (hi - lo) / n;
  const mag = 10 ** Math.floor(Math.log10(raw));
  const step = ([1, 2, 5, 10].map((m) => m * mag).find((x) => x >= raw) ?? 10 * mag);
  const out: number[] = [];
  for (let v = Math.ceil(lo / step) * step; v <= hi + step * 1e-9; v += step) out.push(v);
  return out;
}

/** The timeline: used space per snapshot, as a line with a point per snapshot, on a grid with
 * a labelled size axis and date axis. */
function Timeline({ points }: { points: TimelinePoint[] }): JSX.Element {
  const { t } = useTranslation();
  const lang = useApp((s) => s.lang);
  const [hover, setHover] = useState<TimelinePoint | null>(null);
  const known = points.filter((p) => p.totals.used !== null);
  const W = 720;
  const H = 240;
  const left = 78;
  const right = 16;
  const top = 14;
  const bottom = 34;
  if (known.length < 2) return <p className="note">{t("history.fewPoints")}</p>;
  const t0 = known[0]?.at ?? 0;
  const t1 = known[known.length - 1]?.at ?? 1;
  const used = known.map((p) => p.totals.used ?? 0);
  const GB = 1024 ** 3;
  // The size axis in whole gigabytes, with some room above and below the line.
  const spread = Math.max(Math.max(...used) - Math.min(...used), GB);
  const yTicks = niceTicks((Math.min(...used) - spread * 0.15) / GB, (Math.max(...used) + spread * 0.15) / GB, 4).map((g) => g * GB);
  const lo = Math.min(yTicks[0] ?? 0, ...used);
  const hi = Math.max(yTicks[yTicks.length - 1] ?? 1, ...used);
  const x = (at: number): number => left + ((at - t0) / Math.max(1, t1 - t0)) * (W - left - right);
  const y = (v: number): number => H - bottom - ((v - lo) / Math.max(1, hi - lo)) * (H - top - bottom);
  const days = (t1 - t0) / 86_400;
  const xTicks = niceTicks(t0, t1, 5).filter((v) => v >= t0 && v <= t1);
  const dateText = (at: number): string =>
    new Date(at * 1000).toLocaleDateString(lang, days < 2 ? { hour: "2-digit", minute: "2-digit" } : { day: "numeric", month: "short" });
  const d = known.map((p, i) => `${i === 0 ? "M" : "L"}${x(p.at).toFixed(1)},${y(p.totals.used ?? 0).toFixed(1)}`).join(" ");
  return (
    <div className="timeline">
      <svg viewBox={`0 0 ${String(W)} ${String(H)}`} role="img" aria-label={t("history.timeline")}>
        {yTicks.map((v) => (
          <g key={`y${String(v)}`}>
            <line className="grid" x1={left} x2={W - right} y1={y(v)} y2={y(v)} />
            <text className="axis-label" x={left - 8} y={y(v) + 4} textAnchor="end">
              {fmtBytes(v, lang)}
            </text>
          </g>
        ))}
        {xTicks.map((v) => (
          <g key={`x${String(v)}`}>
            <line className="tick" x1={x(v)} x2={x(v)} y1={H - bottom} y2={H - bottom + 5} />
            <text className="axis-label" x={x(v)} y={H - bottom + 18} textAnchor="middle">
              {dateText(v)}
            </text>
          </g>
        ))}
        <line className="axis" x1={left} x2={left} y1={top} y2={H - bottom} />
        <line className="axis" x1={left} x2={W - right} y1={H - bottom} y2={H - bottom} />
        <text className="axis-title" x={12} y={(top + H - bottom) / 2} transform={`rotate(-90 12 ${String((top + H - bottom) / 2)})`} textAnchor="middle">
          {t("history.axisUsed")}
        </text>
        <path d={d} className="line" />
        {known.map((p) => (
          <circle
            key={p.at}
            cx={x(p.at)}
            cy={y(p.totals.used ?? 0)}
            r={hover?.at === p.at ? 6 : 4}
            className={`pt ${p.kind}`}
            onMouseEnter={() => {
              setHover(p);
            }}
            onMouseLeave={() => {
              setHover(null);
            }}
          />
        ))}
      </svg>
      {hover ? (
        <p className="note">
          {t("history.point", {
            date: new Date(hover.at * 1000).toLocaleString(lang),
            kind: t(`history.kind.${hover.kind}`),
            used: fmtBytes(hover.totals.used, lang),
            free: fmtBytes(hover.totals.free, lang),
            hidden: fmtBytes(hover.totals.hidden, lang),
          })}
        </p>
      ) : (
        <p className="note">{t("history.legend")}</p>
      )}
    </div>
  );
}

/** A small line of one grower's size per snapshot. */
function Sparkline({ trend }: { trend: [number, number][] }): JSX.Element | null {
  if (trend.length < 2) return null;
  const W = 90;
  const H = 24;
  const sizes = trend.map(([, s]) => s);
  const lo = Math.min(...sizes);
  const hi = Math.max(...sizes) || 1;
  const t0 = trend[0]?.[0] ?? 0;
  const t1 = trend[trend.length - 1]?.[0] ?? 1;
  const d = trend
    .map(([at, s], i) => `${i === 0 ? "M" : "L"}${(((at - t0) / Math.max(1, t1 - t0)) * W).toFixed(1)},${(H - 2 - ((s - lo) / Math.max(1, hi - lo)) * (H - 4)).toFixed(1)}`)
    .join(" ");
  return (
    <svg className="spark" viewBox={`0 0 ${String(W)} ${String(H)}`} aria-hidden="true">
      <path d={d} />
    </svg>
  );
}

/** The History screen. */
export function History(): JSX.Element {
  const { t } = useTranslation();
  const { lang, drive, select, setScreen, setFocus } = useApp(
    useShallow((s) => ({ lang: s.lang, drive: s.drive, select: s.select, setScreen: s.setScreen, setFocus: s.setFocus })),
  );
  const [days, setDays] = useState<(typeof PERIODS)[number]>(7);
  const [points, setPoints] = useState<TimelinePoint[] | null>(null);
  const [growth, setGrowth] = useState<GrowthView | null>(null);

  useEffect(() => {
    let alive = true;
    api
      .historyTimeline(drive)
      .then((p) => {
        // Loaded data renders as an interruptible update, so opening the screen never blocks
        // input with one long task (P1, P11).
        if (alive) startTransition(() => {
          setPoints(p);
        });
      })
      .catch(() => {
        if (alive) setPoints([]);
      });
    return () => {
      alive = false;
    };
  }, [drive]);

  useEffect(() => {
    let alive = true;
    api
      .historyGrowth(drive, days)
      .then((g) => {
        if (alive) startTransition(() => {
          setGrowth(g);
        });
      })
      .catch(() => {
        if (alive) setGrowth(null);
      });
    return () => {
      alive = false;
    };
  }, [drive, days]);

  const open = (g: GrowthItem): void => {
    if (g.id === null) return;
    setFocus(0);
    select(g.id);
    setScreen("overview");
  };
  const range = useMemo(() => {
    if (!growth?.from || !growth.to) return null;
    return t("history.range", { from: new Date(growth.from * 1000).toLocaleDateString(lang), to: new Date(growth.to * 1000).toLocaleDateString(lang) });
  }, [growth, lang, t]);

  return (
    <div className="page">
      <header>
        <h1>{t("history.title")}</h1>
        <p className="note">{t("history.lead")}</p>
      </header>
      <section className="panel">
        {points === null ? <p className="note">{t("history.loading")}</p> : <Timeline points={points} />}
      </section>
      <section className="panel">
        <div className="row-head">
          <h2>{t("history.grew")}</h2>
          <div className="seg" role="radiogroup" aria-label={t("history.period")}>
            {PERIODS.map((d) => (
              <button
                key={d}
                role="radio"
                aria-checked={days === d}
                aria-pressed={days === d}
                onClick={() => {
                  setDays(d);
                }}
              >
                {t(`history.periods.${String(d)}`)}
              </button>
            ))}
          </div>
        </div>
        {range ? <p className="note">{range}</p> : null}
        {growth && growth.items.length > 0 ? (
          <table className="table growth">
            <tbody>
              {growth.items.map((g) => (
                <tr key={g.path} className={g.id !== null ? "clickable" : undefined} onClick={() => { open(g); }}>
                  <td>
                    <div>{g.ruleId ? t(`knownLocations.${g.ruleId}.title`) : g.name}</div>
                    <div className="note">{explainShort({ name: g.name, kind: "folder", files: 0, ruleId: g.ruleId, explain: g.explain }, t)}</div>
                    <div className="path">{g.path}</div>
                  </td>
                  <td>
                    <Sparkline trend={g.trend} />
                  </td>
                  <td className={`num ${g.delta < 0 ? "shrink" : "grow"}`}>{fmtDelta(g.delta, lang)}</td>
                  <td className="num note">
                    {fmtBytes(g.before, lang)} → {fmtBytes(g.after, lang)}
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        ) : (
          <p className="note">{growth?.from ? t("history.nothingGrew") : t("history.fewPoints")}</p>
        )}
      </section>
    </div>
  );
}

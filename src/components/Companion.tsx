// Lumen, the companion: a holographic orb that speaks in bubbles, never in popups. One
// bubble at a time; Esc or × closes it; a click on the orb shows earlier bubbles, and
// dragging the orb moves it to another spot on the stage.
import { memo, useEffect, useRef, useState, type JSX } from "react";
import { useShallow } from "zustand/react/shallow";
import { useTranslation } from "react-i18next";

import { useApp } from "../store";

/** The Lumen character as an inline SVG; memoised, as it never changes. */
export const Lumen = memo(function Lumen(): JSX.Element {
  return (
    <svg viewBox="0 0 120 120" aria-hidden="true">
      <defs>
        <radialGradient id="lmCore" cx="40%" cy="35%" r="65%">
          <stop offset="0" stopColor="#fff" />
          <stop offset=".35" stopColor="var(--accent-2)" />
          <stop offset="1" stopColor="var(--accent)" stopOpacity=".3" />
        </radialGradient>
        <radialGradient id="lmHalo">
          <stop offset=".55" stopColor="var(--accent)" stopOpacity=".35" />
          <stop offset="1" stopColor="var(--accent)" stopOpacity="0" />
        </radialGradient>
      </defs>
      <g className="cmp-float">
        <circle cx="60" cy="60" r="44" fill="url(#lmHalo)" className="cmp-pulse" />
        <g className="cmp-spin">
          <ellipse cx="60" cy="60" rx="40" ry="12" fill="none" stroke="var(--accent-2)" strokeOpacity=".7" strokeWidth="1.4" strokeDasharray="3 5" />
        </g>
        <g className="cmp-spin-rev">
          <ellipse cx="60" cy="60" rx="12" ry="38" fill="none" stroke="var(--accent)" strokeOpacity=".5" strokeWidth="1" />
        </g>
        <circle cx="60" cy="60" r="22" fill="url(#lmCore)" />
        <g className="cmp-eyes">
          <rect x="49" y="55" width="7" height="9" rx="3.5" fill="#06131c" />
          <rect x="64" y="55" width="7" height="9" rx="3.5" fill="#06131c" />
        </g>
        <circle cx="53" cy="51" r="3" fill="#fff" opacity=".8" />
      </g>
    </svg>
  );
});

const TTL: Record<string, number> = { tip: 7000, explanation: 11000, progress: 60000, celebration: 6000, warning: 20000 };
/** Where the dragged position is remembered. */
const POS_KEY = "unhoard:lumen-position";
/** Half the orb's size, in pixels: the orb's centre keeps this distance from the edges. */
const HALF = 44;
/** Pointer travel, in pixels, that turns a press into a drag instead of a click. */
const DRAG_START = 5;

/** The orb's centre as fractions of the stage, or `null` for the default corner. */
type Position = { x: number; y: number } | null;

function loadPosition(): Position {
  try {
    const raw = window.localStorage.getItem(POS_KEY);
    if (!raw) return null;
    const p = JSON.parse(raw) as unknown;
    if (p && typeof p === "object" && "x" in p && "y" in p && typeof p.x === "number" && typeof p.y === "number") {
      return { x: Math.min(1, Math.max(0, p.x)), y: Math.min(1, Math.max(0, p.y)) };
    }
  } catch {
    // Storage can be unavailable; the orb then starts in its corner.
  }
  return null;
}

function savePosition(p: Position): void {
  try {
    window.localStorage.setItem(POS_KEY, JSON.stringify(p));
  } catch {
    // Not remembering the position is harmless; the orb stays where it was dropped.
  }
}

/** The companion with its current bubble. Drag the orb to move it; click it for history. */
export function Companion(): JSX.Element {
  const { t } = useTranslation();
  const { queue, dismiss, expire, bubbleVersion } = useApp(
    useShallow((s) => ({ queue: s.queue, dismiss: s.dismiss, expire: s.expire, bubbleVersion: s.bubbleVersion })),
  );
  // The history belongs to the bubble that was showing when it was opened, so any new
  // bubble closes it and old messages never stay on screen.
  const [historyFor, setHistoryFor] = useState<number | null>(null);
  const [pos, setPos] = useState<Position>(loadPosition);
  const drag = useRef<{ x: number; y: number; moved: boolean; box: DOMRect } | null>(null);
  const rootRef = useRef<HTMLDivElement>(null);
  const current = queue.current;
  const showHistory = historyFor === (current?.id ?? 0);

  useEffect(() => {
    if (!current) return;
    const timer = window.setTimeout(expire, TTL[current.type] ?? 8000);
    return () => {
      window.clearTimeout(timer);
    };
  }, [current, bubbleVersion, expire]);

  useEffect(() => {
    const onKey = (e: KeyboardEvent): void => {
      if (e.key === "Escape" && useApp.getState().queue.current) dismiss();
    };
    window.addEventListener("keydown", onKey);
    return () => {
      window.removeEventListener("keydown", onKey);
    };
  }, [dismiss]);

  const onPointerDown = (e: React.PointerEvent<HTMLButtonElement>): void => {
    const box = rootRef.current?.parentElement?.getBoundingClientRect();
    if (!box || e.button !== 0) return;
    drag.current = { x: e.clientX, y: e.clientY, moved: false, box };
    e.currentTarget.setPointerCapture(e.pointerId);
  };
  const onPointerMove = (e: React.PointerEvent<HTMLButtonElement>): void => {
    const d = drag.current;
    if (!d) return;
    if (!d.moved && Math.hypot(e.clientX - d.x, e.clientY - d.y) < DRAG_START) return;
    d.moved = true;
    const mx = HALF / d.box.width;
    const my = HALF / d.box.height;
    setPos({
      x: Math.min(1 - mx, Math.max(mx, (e.clientX - d.box.left) / d.box.width)),
      y: Math.min(1 - my, Math.max(my, (e.clientY - d.box.top) / d.box.height)),
    });
  };
  const onPointerUp = (): void => {
    if (drag.current?.moved) savePosition(pos);
  };
  const onClick = (): void => {
    // A drag ends with a click event too; only a press without movement opens the history.
    const moved = drag.current?.moved ?? false;
    drag.current = null;
    if (!moved) setHistoryFor(showHistory ? null : (current?.id ?? 0));
  };

  // The bubble opens towards the middle of the stage, so it never leaves the screen.
  const side = `${pos && pos.x < 0.5 ? " right" : ""}${pos && pos.y < 0.45 ? " below" : ""}`;
  const style = pos ? { left: `${String(pos.x * 100)}%`, top: `${String(pos.y * 100)}%` } : undefined;
  return (
    <div ref={rootRef} className={`companion${current ? " talking" : ""}${pos ? " placed" : ""}${side}`} style={style}>
      <div className="cmp-pop">
        {showHistory && queue.history.length > 0 ? (
          <div className="history-list">
            <div className="kind">{t("companion.earlier")}</div>
            {queue.history
              .slice(-20)
              .reverse()
              .map((b) => (
                <div key={b.id}>{b.text}</div>
              ))}
          </div>
        ) : null}
        <div role="status" aria-live="polite">
          {current ? (
            <div className={`bubble ${current.type}`}>
              <div className="kind">{t(`companion.types.${current.type}`)}</div>
              <div>{current.text}</div>
              {current.actions ? (
                <div className="btns">
                  {current.actions.map((a, i) => (
                    <button
                      key={a.label}
                      className={`btn small${i === 0 ? " primary" : ""}`}
                      onClick={() => {
                        dismiss();
                        a.run();
                      }}
                    >
                      {a.label}
                    </button>
                  ))}
                </div>
              ) : null}
              <button className="x" onClick={dismiss} aria-label={t("companion.close")}>
                ×
              </button>
            </div>
          ) : null}
        </div>
      </div>
      <button
        className="char"
        onClick={onClick}
        onPointerDown={onPointerDown}
        onPointerMove={onPointerMove}
        onPointerUp={onPointerUp}
        aria-label={t("companion.history")}
        title={t("companion.drag")}
      >
        <Lumen />
      </button>
    </div>
  );
}

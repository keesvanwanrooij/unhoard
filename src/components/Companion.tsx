// Lumen, the companion: a holographic orb that speaks in bubbles, never in popups. One
// bubble at a time; Esc closes it; earlier bubbles stay in a history panel.
import { useEffect, useState, type JSX } from "react";
import { useShallow } from "zustand/react/shallow";
import { useTranslation } from "react-i18next";

import { useApp } from "../store";

/** The Lumen character as an inline SVG. */
export function Lumen(): JSX.Element {
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
}

const TTL: Record<string, number> = { tip: 7000, explanation: 11000, progress: 60000, celebration: 6000, warning: 20000 };

/** The companion with its current bubble. */
export function Companion(): JSX.Element {
  const { t } = useTranslation();
  const { queue, dismiss, bubbleVersion } = useApp(useShallow((s) => ({ queue: s.queue, dismiss: s.dismiss, bubbleVersion: s.bubbleVersion })));
  const [showHistory, setShowHistory] = useState(false);
  const current = queue.current;

  useEffect(() => {
    if (!current) return;
    const timer = window.setTimeout(dismiss, TTL[current.type] ?? 8000);
    return () => {
      window.clearTimeout(timer);
    };
  }, [current, bubbleVersion, dismiss]);

  useEffect(() => {
    const onKey = (e: KeyboardEvent): void => {
      if (e.key === "Escape" && useApp.getState().queue.current) dismiss();
    };
    window.addEventListener("keydown", onKey);
    return () => {
      window.removeEventListener("keydown", onKey);
    };
  }, [dismiss]);

  return (
    <div className={`companion${current ? " talking" : ""}`}>
      <div role="status" aria-live="polite">
        {current ? (
          <div className={`bubble ${current.type}`}>
            <div className="kind">{t(`companion.types.${current.type}`)}</div>
            <div>{current.text}</div>
            <button className="x" onClick={dismiss} aria-label={t("companion.close")}>
              ×
            </button>
          </div>
        ) : null}
      </div>
      {showHistory && queue.history.length > 0 ? (
        <div className="history-list">
          {queue.history
            .slice(-20)
            .reverse()
            .map((b) => (
              <div key={b.id}>{b.text}</div>
            ))}
        </div>
      ) : null}
      <button
        className="char"
        onClick={() => {
          setShowHistory((s) => !s);
        }}
        aria-label={t("companion.history")}
        title={t("companion.history")}
      >
        <Lumen />
      </button>
    </div>
  );
}

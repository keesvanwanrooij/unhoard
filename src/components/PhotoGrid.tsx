// The review grid of one photo group: nothing is selected at the start (S1); "select all
// but the keeper" is one action; arrows move, Space selects, Enter moves, N skips.
import { useEffect, useRef, useState, type JSX } from "react";
import { useShallow } from "zustand/react/shallow";
import { useTranslation } from "react-i18next";

import type { Group, MediaItem, ThumbOut } from "../api/types";
import { fmtBytes } from "../format";
import { useApp } from "../store";

/** Draws a base64 RGBA thumbnail into a canvas. */
function Thumb({ load }: { load: () => Promise<ThumbOut> }): JSX.Element {
  const ref = useRef<HTMLCanvasElement>(null);
  const [failed, setFailed] = useState(false);
  const { t } = useTranslation();
  useEffect(() => {
    let alive = true;
    load()
      .then((th) => {
        const c = ref.current;
        if (!alive || !c) return;
        const bytes = Uint8ClampedArray.from(atob(th.rgba), (ch) => ch.charCodeAt(0));
        c.width = th.width;
        c.height = th.height;
        c.getContext("2d")?.putImageData(new ImageData(bytes, th.width, th.height), 0, 0);
      })
      .catch(() => {
        if (alive) setFailed(true);
      });
    return () => {
      alive = false;
    };
  }, [load]);
  return failed ? <div className="ph">{t("photos.noThumb")}</div> : <canvas ref={ref} />;
}

/** The grid of one group. */
export function PhotoGrid({
  group,
  media,
  thumb,
  onMove,
  onNext,
}: {
  group: Group;
  media: MediaItem[];
  thumb: (index: number) => Promise<ThumbOut>;
  onMove: (nodeIds: number[]) => void;
  onNext: () => void;
}): JSX.Element {
  const { t } = useTranslation();
  const { lang } = useApp(useShallow((s) => ({ lang: s.lang })));
  const [picked, setPicked] = useState<Set<number>>(new Set());
  const [focus, setFocusState] = useState(0);
  // Keys can arrive faster than React renders, so the handler reads the focus from a ref.
  const focusRef = useRef(0);
  const setFocus = (f: number | ((prev: number) => number)): void => {
    focusRef.current = typeof f === "function" ? f(focusRef.current) : f;
    setFocusState(focusRef.current);
  };
  const items = group.items;

  const toggle = (i: number): void => {
    setPicked((p) => {
      const n = new Set(p);
      if (n.has(i)) n.delete(i);
      else n.add(i);
      return n;
    });
  };
  const move = (): void => {
    const ids = items.filter((i) => picked.has(i)).map((i) => media[i]?.node).filter((n): n is number => n !== undefined);
    if (ids.length > 0) onMove(ids);
  };
  const onKey = (e: React.KeyboardEvent): void => {
    if (e.key === "ArrowRight" || e.key === "ArrowDown") setFocus((f) => Math.min(items.length - 1, f + 1));
    else if (e.key === "ArrowLeft" || e.key === "ArrowUp") setFocus((f) => Math.max(0, f - 1));
    else if (e.key === " ") {
      const i = items[focusRef.current];
      if (i !== undefined) toggle(i);
    } else if (e.key === "Enter") move();
    else if (e.key === "n" || e.key === "N") onNext();
    else return;
    e.preventDefault();
  };

  return (
    <div className="page wide" style={{ gap: 10 }}>
      <div className="btns">
        <button
          className="btn"
          onClick={() => {
            setPicked(new Set(items.filter((i) => i !== group.keeper)));
          }}
        >
          {t("photos.selectAllButKeeper")}
        </button>
        <button
          className="btn"
          onClick={() => {
            setPicked(new Set());
          }}
        >
          {t("photos.clear")}
        </button>
        <button className="btn primary" disabled={picked.size === 0} onClick={move}>
          {t("photos.move")}
        </button>
        <button className="btn" onClick={onNext}>
          {t("photos.next")}
        </button>
        <span className="note">{t("photos.selected", { count: picked.size })}</span>
      </div>
      <p className="note">{t("photos.keys")}</p>
      <div className="grid" role="group" tabIndex={0} onKeyDown={onKey} aria-label={t(`photos.kind.${group.kind}`)}>
        {items.map((i, k) => {
          const m = media[i];
          if (!m) return null;
          return (
            <button
              key={i}
              role="checkbox"
              aria-checked={picked.has(i)}
              className={`tile${k === focus ? " focus" : ""}`}
              onClick={() => {
                setFocus(k);
                toggle(i);
              }}
            >
              <Thumb load={() => thumb(i)} />
              {group.keeper === i ? <span className="star" title={t("photos.keeper")}>★</span> : null}
              <span>{m.name}</span>
              <span className="note">{fmtBytes(m.size, lang)}</span>
            </button>
          );
        })}
      </div>
    </div>
  );
}

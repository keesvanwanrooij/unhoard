// Photos & videos: groups of duplicates, near duplicates, bursts, chat media and
// screenshots, reviewed one grid at a time. Online-only photos are never downloaded.
import { useCallback, useEffect, useMemo, useState, type JSX } from "react";
import { useShallow } from "zustand/react/shallow";
import { useTranslation } from "react-i18next";

import { api, onPhotosProgress } from "../api/ipc";
import type { GroupKind, MovePlan, PhotoState } from "../api/types";
import { PhotoGrid } from "../components/PhotoGrid";
import { MovePreview } from "../components/Preview";
import { fmtBytes, fmtInt } from "../format";
import { useApp } from "../store";

/** The kinds of group, in the order of the filter. */
const KINDS: GroupKind[] = ["exactDuplicates", "nearDuplicates", "burst", "chatMedia", "screenshots"];
/** How many groups the list shows at first, and adds per click. */
const PAGE = 100;

/** The Photos screen. */
export function Photos(): JSX.Element {
  const { t } = useTranslation();
  const { lang, scan, say, sayError, treeChanged, endTopic } = useApp(useShallow((s) => ({ lang: s.lang, scan: s.scan, say: s.say, sayError: s.sayError, treeChanged: s.treeChanged, endTopic: s.endTopic })));
  const [state, setState] = useState<PhotoState | null>(null);
  const [busy, setBusy] = useState(false);
  const [current, setCurrent] = useState(0);
  const [preview, setPreview] = useState<{ ids: number[]; plan: MovePlan } | null>(null);
  const [progress, setProgress] = useState<[number, number] | null>(null);
  const [filter, setFilter] = useState<GroupKind | "all">("all");
  const [limit, setLimit] = useState(PAGE);

  useEffect(() => {
    const off = onPhotosProgress((done, total) => {
      setProgress([done, total]);
    });
    return () => {
      off.then((f) => {
        f();
      }).catch(() => undefined);
    };
  }, []);

  // The groups of the chosen kind, largest first: a big library has thousands of groups.
  const order = useMemo(() => {
    if (!state) return [];
    const bytes = state.groups.map((g) => g.items.reduce((sum, k) => sum + (state.media[k]?.size ?? 0), 0));
    return state.groups
      .map((_, i) => i)
      .filter((i) => filter === "all" || state.groups[i]?.kind === filter)
      .sort((a, b) => (bytes[b] ?? 0) - (bytes[a] ?? 0));
  }, [state, filter]);
  const next = (): void => {
    const at = order.indexOf(current);
    const to = order[Math.min(at + 1, order.length - 1)];
    if (to !== undefined) setCurrent(to);
  };

  const analyze = (): void => {
    setBusy(true);
    setProgress(null);
    say({ type: "progress", text: t("photos.analyzing"), key: "photos" });
    api
      .photosAnalyze()
      .then((s) => {
        setState(s);
        setFilter("all");
        setLimit(PAGE);
        // The largest group first.
        const size = (i: number): number => s.groups[i]?.items.reduce((sum, k) => sum + (s.media[k]?.size ?? 0), 0) ?? 0;
        setCurrent(s.groups.reduce((best, _, i) => (size(i) > size(best) ? i : best), 0));
        say({ type: "tip", text: t("companion.script.photoReview.0") });
      })
      .catch((e: unknown) => {
        sayError(e, t);
      })
      .finally(() => {
        setBusy(false);
        endTopic("photos");
      });
  };

  const thumb = useCallback(
    (index: number) => {
      if (!state) return Promise.reject(new Error("no analysis"));
      return api.photoThumbnail(state.generation, index, 192);
    },
    [state],
  );

  if (!scan?.done) return <div className="page note">{t("photos.scanFirst")}</div>;
  const group = state?.groups[current];
  return (
    <div className="page wide">
      <header>
        <h1>{t("photos.title")}</h1>
        <button className="btn primary" disabled={busy} onClick={analyze}>
          {t("photos.analyze")}
        </button>
        {state ? <span className="note">{t("photos.summary", { media: fmtInt(state.media.length, lang), groups: fmtInt(state.groups.length, lang) })}</span> : null}
      </header>
      {!state && !busy ? <p className="note">{t("photos.start")}</p> : null}
      {busy && progress && progress[1] > 0 ? (
        <div className="photo-progress">
          <progress value={progress[0]} max={progress[1]} />
          <span className="note">{t("photos.progress", { done: fmtInt(progress[0], lang), total: fmtInt(progress[1], lang) })}</span>
        </div>
      ) : null}
      {state && state.groups.length === 0 ? <p className="note">{t("photos.empty")}</p> : null}
      {state && group ? (
        <div className="photos">
          <div className="panel groups" role="listbox">
            <div className="seg wrap" role="group">
              {(["all", ...KINDS] as const).map((k) => {
                const n = k === "all" ? state.groups.length : state.groups.filter((g) => g.kind === k).length;
                if (n === 0) return null;
                return (
                  <button
                    key={k}
                    aria-pressed={filter === k}
                    onClick={() => {
                      setFilter(k);
                      setLimit(PAGE);
                    }}
                  >
                    {k === "all" ? t("photos.all") : t(`photos.kind.${k}`)} · {fmtInt(n, lang)}
                  </button>
                );
              })}
            </div>
            {order.slice(0, limit).map((i) => {
              const g = state.groups[i];
              if (!g) return null;
              const bytes = g.items.reduce((sum, k) => sum + (state.media[k]?.size ?? 0), 0);
              return (
                <button
                  key={`${g.kind}-${String(i)}`}
                  role="option"
                  aria-selected={i === current}
                  onClick={() => {
                    setCurrent(i);
                    setPreview(null);
                  }}
                >
                  <div>
                    {t(`photos.kind.${g.kind}`)} {g.label} · {g.items.length}
                  </div>
                  <div className="note">
                    {t(`photos.certainty.${g.certainty}`)} · {fmtBytes(bytes, lang)}
                  </div>
                </button>
              );
            })}
            {order.length > limit ? (
              <button
                className="more"
                onClick={() => {
                  setLimit((n) => n + PAGE);
                }}
              >
                {t("photos.more", { count: fmtInt(Math.min(PAGE, order.length - limit), lang) })}
              </button>
            ) : null}
          </div>
          <div className="panel" style={{ minHeight: 0 }}>
            <h2>
              {t(`photos.kind.${group.kind}`)} · {t(`photos.certainty.${group.certainty}`)}
            </h2>
            {group.keeper !== null && group.keeperReasons.length > 0 ? (
              <p className="note">
                ★ {t("photos.keeper")}: {group.keeperReasons.map((r) => t(`photos.reason.${r}`)).join(", ")}
              </p>
            ) : null}
            {group.items.some((k) => state.media[k]?.cloudOnly || state.media[k]?.inSyncRoot) ? <p className="note">{t("photos.cloudNote")}</p> : null}
            {preview ? (
              <MovePreview
                plan={preview.plan}
                busy={busy}
                onCancel={() => {
                  setPreview(null);
                }}
                onConfirm={() => {
                  setBusy(true);
                  api
                    .quarantineMove(state.generation, preview.ids)
                    .then(() => {
                      setPreview(null);
                      say({ type: "explanation", text: t("companion.script.quarantineMoved.0") });
                      treeChanged();
                      next();
                    })
                    .catch((e: unknown) => {
                      sayError(e, t);
                    })
                    .finally(() => {
                      setBusy(false);
                    });
                }}
              />
            ) : (
              <PhotoGrid
                key={current}
                group={group}
                media={state.media}
                thumb={thumb}
                onMove={(ids) => {
                  api
                    .quarantinePreview(state.generation, ids)
                    .then((plan) => {
                      setPreview({ ids, plan });
                    })
                    .catch((e: unknown) => {
                      sayError(e, t);
                    });
                }}
                onNext={next}
              />
            )}
          </div>
        </div>
      ) : null}
    </div>
  );
}

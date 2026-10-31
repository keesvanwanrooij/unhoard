// Photos & videos: groups of duplicates, near duplicates, bursts, chat media and
// screenshots, reviewed one grid at a time. Online-only photos are never downloaded.
import { useCallback, useState, type JSX } from "react";
import { useShallow } from "zustand/react/shallow";
import { useTranslation } from "react-i18next";

import { api } from "../api/ipc";
import type { MovePlan, PhotoState } from "../api/types";
import { PhotoGrid } from "../components/PhotoGrid";
import { MovePreview } from "../components/Preview";
import { fmtBytes, fmtInt } from "../format";
import { useApp } from "../store";

/** The Photos screen. */
export function Photos(): JSX.Element {
  const { t } = useTranslation();
  const { lang, scan, say, sayError, treeChanged } = useApp(useShallow((s) => ({ lang: s.lang, scan: s.scan, say: s.say, sayError: s.sayError, treeChanged: s.treeChanged })));
  const [state, setState] = useState<PhotoState | null>(null);
  const [busy, setBusy] = useState(false);
  const [current, setCurrent] = useState(0);
  const [preview, setPreview] = useState<{ ids: number[]; plan: MovePlan } | null>(null);

  const analyze = (): void => {
    setBusy(true);
    say({ type: "progress", text: t("photos.analyzing"), key: "photos" });
    api
      .photosAnalyze()
      .then((s) => {
        setState(s);
        setCurrent(0);
        say({ type: "tip", text: t("companion.script.photoReview.0") });
      })
      .catch((e: unknown) => {
        sayError(e, t);
      })
      .finally(() => {
        setBusy(false);
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
    <div className="page">
      <header>
        <h1>{t("photos.title")}</h1>
        <button className="btn primary" disabled={busy} onClick={analyze}>
          {t("photos.analyze")}
        </button>
        {state ? <span className="note">{t("photos.summary", { media: fmtInt(state.media.length, lang), groups: fmtInt(state.groups.length, lang) })}</span> : null}
      </header>
      {state && state.groups.length === 0 ? <p className="note">{t("photos.empty")}</p> : null}
      {state && group ? (
        <div className="photos">
          <div className="panel groups" role="listbox">
            {state.groups.map((g, i) => {
              const bytes = g.items.reduce((s, k) => s + (state.media[k]?.size ?? 0), 0);
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
                      setCurrent((c) => Math.min(c + 1, state.groups.length - 1));
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
                onNext={() => {
                  setCurrent((c) => Math.min(c + 1, state.groups.length - 1));
                }}
              />
            )}
          </div>
        </div>
      ) : null}
    </div>
  );
}

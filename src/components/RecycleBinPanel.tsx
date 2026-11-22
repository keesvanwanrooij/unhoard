// The Recycle Bin on the Suggestions screen: what the user deleted, by its original name and
// folder (read from Windows' info files), and "Empty" after a confirmation. Emptying goes
// through Windows' own function (S6).
import { startTransition, useCallback, useEffect, useState, type JSX } from "react";
import { useShallow } from "zustand/react/shallow";
import { useTranslation } from "react-i18next";

import { api } from "../api/ipc";
import type { RecycleBin } from "../api/types";
import { fmtBytes } from "../format";
import { useApp } from "../store";

/** How many of the largest items the panel lists. */
const SHOWN = 6;

/** The last part of a path, and the folder it was in. */
function split(path: string): [string, string] {
  const i = path.lastIndexOf("\\");
  return i < 0 ? [path, ""] : [path.slice(i + 1), path.slice(0, i)];
}

/** The Recycle Bin panel. */
export function RecycleBinPanel(): JSX.Element | null {
  const { t } = useTranslation();
  const { lang, say, sayError } = useApp(useShallow((s) => ({ lang: s.lang, say: s.say, sayError: s.sayError })));
  const [bin, setBin] = useState<RecycleBin | null>(null);
  const [confirming, setConfirming] = useState(false);
  const [busy, setBusy] = useState(false);

  const load = useCallback(() => {
    api
      .recycleBin()
      .then((b) => {
        startTransition(() => {
          setBin(b);
        });
      })
      .catch(() => {
        setBin(null);
      });
  }, []);
  useEffect(load, [load]);

  if (!bin || bin.count === 0) return null;
  const empty = (): void => {
    setBusy(true);
    api
      .recycleBinEmpty()
      .then((freed) => {
        setConfirming(false);
        say({ type: "celebration", text: t("recycle.done", { size: fmtBytes(freed, lang) }) });
        load();
      })
      .catch((e: unknown) => {
        sayError(e, t);
      })
      .finally(() => {
        setBusy(false);
      });
  };

  return (
    <section className="panel recycle">
      <div className="row-head">
        <h2>{t("recycle.title")}</h2>
        <span className="sz">{t("recycle.summary", { count: bin.count, size: fmtBytes(bin.total, lang) })}</span>
      </div>
      <p className="note">{t("recycle.lead")}</p>
      <ul className="recycle-list">
        {bin.items.slice(0, SHOWN).map((item) => {
          const [name, folder] = split(item.original);
          return (
            <li key={`${item.original}-${String(item.deletedAt)}`}>
              <span className="nm">{name}</span>
              <span className="note">
                {folder} · {t("recycle.deleted", { date: new Date(item.deletedAt * 1000).toLocaleDateString(lang) })}
              </span>
              <span className="sz">{fmtBytes(item.size, lang)}</span>
            </li>
          );
        })}
        {bin.count > SHOWN ? <li className="note">{t("recycle.more", { count: bin.count - SHOWN })}</li> : null}
      </ul>
      {confirming ? (
        <div className="sheet danger" role="group" aria-label={t("recycle.confirmTitle")}>
          <b>{t("recycle.confirmTitle")}</b>
          <div>{t("recycle.confirmText", { count: bin.count, size: fmtBytes(bin.total, lang) })}</div>
          <div className="btns">
            <button className="btn primary" disabled={busy} onClick={empty}>
              {t("recycle.confirm")}
            </button>
            <button
              className="btn"
              disabled={busy}
              onClick={() => {
                setConfirming(false);
              }}
            >
              {t("preview.cancel")}
            </button>
          </div>
        </div>
      ) : (
        <button
          className="btn small"
          style={{ alignSelf: "flex-start" }}
          onClick={() => {
            setConfirming(true);
          }}
        >
          {t("recycle.emptyButton")}
        </button>
      )}
    </section>
  );
}

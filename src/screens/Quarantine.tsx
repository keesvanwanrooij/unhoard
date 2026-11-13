// The quarantine: batches with their purge date, restore, "not freed yet", and how to
// delete earlier in Explorer. Purge warnings are shown by the companion and recorded.
import { useCallback, useEffect, useState, type JSX } from "react";
import { useShallow } from "zustand/react/shallow";
import { useTranslation } from "react-i18next";

import { api } from "../api/ipc";
import type { QuarantineState } from "../api/types";
import { fmtBytes } from "../format";
import { useApp } from "../store";

/** Loads the quarantine and shows due purge warnings once. */
export function useQuarantine(): [QuarantineState | null, () => void] {
  const { t } = useTranslation();
  const { lang, say } = useApp(useShallow((s) => ({ lang: s.lang, say: s.say })));
  const [state, setState] = useState<QuarantineState | null>(null);
  const load = useCallback(() => {
    api
      .quarantineState()
      .then((s) => {
        setState(s);
        for (const w of s.warnings) {
          const date = new Date(w.purgeAfterUnix * 1000 - 30 * 86400 * 1000).toLocaleDateString(lang);
          say({ type: "warning", text: t(`companion.script.purgeWarning.${String(w.daysLeft)}.0`, { date, size: fmtBytes(w.onDiskBytes, lang) }) });
          api.quarantineWarningShown(w.batchId, w.daysLeft).catch(() => {
            // Not recorded: the warning shows again next time, and the purge keeps waiting.
            say({ type: "warning", text: t("errors.invalidArgument") });
          });
        }
      })
      .catch(() => {
        setState({ batches: [], warnings: [], space: [] });
      });
  }, [lang, say, t]);
  useEffect(load, [load]);
  return [state, load];
}

/** The Quarantine screen. */
export function Quarantine(): JSX.Element {
  const { t } = useTranslation();
  const { lang, say, sayError, treeChanged } = useApp(useShallow((s) => ({ lang: s.lang, say: s.say, sayError: s.sayError, treeChanged: s.treeChanged })));
  const [state, reload] = useQuarantine();
  const [busy, setBusy] = useState(false);
  if (!state) return <div className="page" />;
  const active = state.batches.filter((b) => b.manifest.items.some((i) => i.status === "quarantined"));
  const notFreed = state.space.reduce((s, [, d]) => s + d.notFreedBytes, 0);
  const restore = (batchId: string, ids: string[]): void => {
    setBusy(true);
    api
      .quarantineRestore(batchId, ids)
      .then((results) => {
        const renamed = results.map((r) => r.outcome).find((o) => o.outcome === "restored" && o.renamed);
        if (renamed && renamed.outcome === "restored") say({ type: "explanation", text: t("quarantine.restoredAs", { name: renamed.to }) });
        else say({ type: "celebration", text: t("companion.script.restored.0") });
        treeChanged();
        reload();
      })
      .catch((e: unknown) => {
        sayError(e, t);
      })
      .finally(() => {
        setBusy(false);
      });
  };
  return (
    <div className="page">
      <header>
        <h1>{t("quarantine.title")}</h1>
        {notFreed > 0 ? <span className="chip q">{t("quarantine.notFreed", { size: fmtBytes(notFreed, lang) })}</span> : null}
      </header>
      <p className="note">{t("quarantine.intro")}</p>
      {active.length === 0 ? <p className="note">{t("quarantine.empty")}</p> : null}
      {active.map((b) => {
        const m = b.manifest;
        const left = m.items.filter((i) => i.status === "quarantined");
        return (
          <section key={m.batchId} className="panel">
            <h2>
              {t("quarantine.batch", {
                date: new Date(m.createdUnix * 1000).toLocaleDateString(lang),
                purge: new Date(m.purgeAfterUnix * 1000).toLocaleDateString(lang),
              })}
            </h2>
            <table className="table">
              <tbody>
                {m.items.map((i) => (
                  <tr key={i.id}>
                    <td>
                      <div className="path">{i.originalPath}</div>
                      {i.cloud ? <span className="chip">{t("quarantine.cloud")}</span> : null}
                    </td>
                    <td className="num">{fmtBytes(i.onDiskBytes || i.logicalBytes, lang)}</td>
                    <td>{t(`quarantine.status.${i.status}`)}</td>
                    <td>
                      {i.status === "quarantined" ? (
                        <button
                          className="btn small"
                          disabled={busy}
                          onClick={() => {
                            restore(m.batchId, [i.id]);
                          }}
                        >
                          {t("quarantine.restore")}
                        </button>
                      ) : null}
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
            <div className="btns" style={{ marginTop: 10 }}>
              <button
                className="btn"
                disabled={busy || left.length === 0}
                onClick={() => {
                  restore(
                    m.batchId,
                    left.map((i) => i.id),
                  );
                }}
              >
                {t("quarantine.restoreAll")}
              </button>
              <button
                className="btn"
                onClick={() => {
                  say({ type: "tip", text: t("companion.script.deleteEarly.0") });
                  api.quarantineOpen(m.batchId).catch((e: unknown) => {
                    sayError(e, t);
                  });
                }}
              >
                {t("quarantine.open")}
              </button>
            </div>
          </section>
        );
      })}
    </div>
  );
}

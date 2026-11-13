// Cloud: each sync root with cloud versus local size and placeholder states, and "free up
// space" with a preview. Nothing is downloaded or deleted.
import { useEffect, useState, type JSX } from "react";
import { useShallow } from "zustand/react/shallow";
import { useTranslation } from "react-i18next";

import { api } from "../api/ipc";
import type { CloudRoot, FreeUpPlan } from "../api/types";
import { fmtBytes, fmtInt } from "../format";
import { useApp } from "../store";

/** The Cloud screen. */
export function Cloud(): JSX.Element {
  const { t } = useTranslation();
  const { lang, scan, info, treeVersion, say, sayError, treeChanged } = useApp(useShallow((s) => ({ lang: s.lang, scan: s.scan, info: s.info, treeVersion: s.treeVersion, say: s.say, sayError: s.sayError, treeChanged: s.treeChanged })));
  const [roots, setRoots] = useState<CloudRoot[] | null>(null);
  const [plan, setPlan] = useState<{ id: number; plan: FreeUpPlan } | null>(null);
  const [busy, setBusy] = useState(false);
  const generation = scan?.done ? scan.generation : undefined;

  useEffect(() => {
    if (generation === undefined) return;
    let alive = true;
    api
      .cloudRoots(generation)
      .then((r) => {
        if (alive) setRoots(r);
      })
      .catch(() => {
        if (alive) setRoots([]);
      });
    return () => {
      alive = false;
    };
  }, [generation, treeVersion]);

  if ((info?.syncRoots.length ?? 0) === 0) return <div className="page note">{t("cloud.none")}</div>;
  if (generation === undefined || !roots) return <div className="page note">{t("cloud.scanFirst")}</div>;
  return (
    <div className="page">
      <header>
        <h1>{t("cloud.title")}</h1>
      </header>
      {roots.map((r) => (
        <section key={r.path} className="panel">
          <h2>{r.path}</h2>
          <p className="note">{r.path.toLowerCase().includes("onedrive") ? t("cloud.tested") : t("cloud.notTested")}</p>
          {r.summary && r.id !== null ? (
            <>
              <div className="stats">
                <div>
                  <span>{t("cloud.cloudSize")}</span>
                  <b>{fmtBytes(r.summary.cloudBytes, lang)}</b>
                </div>
                <div>
                  <span>{t("cloud.localSize")}</span>
                  <b>{fmtBytes(r.summary.localBytes, lang)}</b>
                </div>
                <div>
                  <span>{t("cloud.onlineOnly")}</span>
                  <b>{fmtInt(r.summary.onlineOnly, lang)}</b>
                </div>
                <div>
                  <span>{t("cloud.locallyAvailable")}</span>
                  <b>{fmtInt(r.summary.locallyAvailable, lang)}</b>
                </div>
                <div>
                  <span>{t("cloud.alwaysKeep")}</span>
                  <b>{fmtInt(r.summary.alwaysKeep, lang)}</b>
                </div>
              </div>
              {plan?.id === r.id ? (
                <div className="sheet">
                  <b>{t("preview.title")}</b>
                  <div>{t("preview.items", { count: plan.plan.files.length, size: fmtBytes(plan.plan.bytesToFree, lang) })}</div>
                  {plan.plan.skipped.length > 0 ? (
                    <div>
                      {t("preview.skipped")}: {plan.plan.skipped.length} · {t("preview.reasons.notInSync")}
                    </div>
                  ) : null}
                  <div className="note">{t("preview.freeNote")}</div>
                  <div className="btns">
                    <button
                      className="btn primary"
                      disabled={busy || plan.plan.files.length === 0}
                      onClick={() => {
                        const id = plan.id;
                        setBusy(true);
                        api
                          .freeUpApply(generation, [id])
                          .then((res) => {
                            setPlan(null);
                            const freed = Math.max(0, res.requestedBytes - res.stillLocalBytes);
                            say({ type: "celebration", text: t("companion.script.freeUpSpaceDone.0", { size: fmtBytes(freed, lang) }) });
                            treeChanged();
                          })
                          .catch((e: unknown) => {
                            sayError(e, t);
                          })
                          .finally(() => {
                            setBusy(false);
                          });
                      }}
                    >
                      {t("preview.confirmFree")}
                    </button>
                    <button
                      className="btn"
                      onClick={() => {
                        setPlan(null);
                      }}
                    >
                      {t("preview.cancel")}
                    </button>
                  </div>
                </div>
              ) : (
                <div className="btns" style={{ marginTop: 10 }}>
                  <button
                    className="btn primary"
                    disabled={r.summary.localBytes === 0}
                    onClick={() => {
                      const id = r.id;
                      if (id === null) return;
                      api
                        .freeUpPreview(generation, [id])
                        .then((p) => {
                          setPlan({ id, plan: p });
                          say({ type: "explanation", text: t("companion.script.cloudIntro.0") });
                        })
                        .catch((e: unknown) => {
                          sayError(e, t);
                        });
                    }}
                  >
                    {t("cloud.freeUp")}
                  </button>
                </div>
              )}
            </>
          ) : (
            <p className="note">{t("cloud.scanFirst")}</p>
          )}
        </section>
      ))}
    </div>
  );
}

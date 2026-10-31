// Suggestions: every recognised item and every large unrecognised file, by tier and size.
// Nothing is pre-selected; each move starts with its preview.
import { useEffect, useState, type JSX } from "react";
import { useShallow } from "zustand/react/shallow";
import { useTranslation } from "react-i18next";

import { api } from "../api/ipc";
import type { MovePlan, Suggestion, Tier } from "../api/types";
import { MovePreview, actionLabel } from "../components/Preview";
import { fmtBytes } from "../format";
import { useApp } from "../store";

const TIERS: Tier[] = ["safe", "likelySafe", "review", "keep"];

/** The Suggestions screen. */
export function Suggestions(): JSX.Element {
  const { t } = useTranslation();
  const { lang, scan, treeVersion, say, sayError, treeChanged } = useApp(useShallow((s) => ({ lang: s.lang, scan: s.scan, treeVersion: s.treeVersion, say: s.say, sayError: s.sayError, treeChanged: s.treeChanged })));
  const [items, setItems] = useState<Suggestion[] | null>(null);
  const [preview, setPreview] = useState<{ id: number; plan: MovePlan } | null>(null);
  const [busy, setBusy] = useState(false);
  const done = scan?.done ?? false;

  useEffect(() => {
    if (!done) return;
    let alive = true;
    api
      .suggestions(500)
      .then(([, list]) => {
        if (alive) setItems(list);
      })
      .catch(() => {
        if (alive) setItems([]);
      });
    return () => {
      alive = false;
    };
  }, [done, treeVersion]);

  if (!scan) return <div className="page note">{t("errors.noScan")}</div>;
  if (!done || !items) return <div className="page note">{t("top.scanning", { files: scan.files, seconds: Math.round(scan.elapsedMs / 1000) })}</div>;
  const generation = scan.generation;
  return (
    <div className="page">
      <header>
        <h1>{t("nav.suggestions")}</h1>
      </header>
      {TIERS.map((tier) => {
        const list = items.filter((s) => s.tier === tier);
        if (list.length === 0) return null;
        const total = list.reduce((s, x) => s + x.node.onDisk, 0);
        return (
          <section key={tier} className="panel">
            <h2>
              <span className={`chip tier-${tier}`}>{t(`tier.${tier}`)}</span> {fmtBytes(total, lang)}
            </h2>
            <table className="table">
              <tbody>
                {list.map((s) => {
                  const key = s.description.ruleId;
                  return (
                    <tr key={s.node.id}>
                      <td>
                        <div>{key ? t(`knownLocations.${key}.title`) : s.node.name}</div>
                        <div className="path">{s.path}</div>
                        <div className="note">{key ? t(`knownLocations.${key}.action`) : t("details.generic")}</div>
                        {preview?.id === s.node.id ? (
                          <MovePreview
                            plan={preview.plan}
                            busy={busy}
                            onCancel={() => {
                              setPreview(null);
                            }}
                            onConfirm={() => {
                              setBusy(true);
                              api
                                .quarantineMove(generation, [s.node.id])
                                .then(() => {
                                  setPreview(null);
                                  say({ type: "explanation", text: t("companion.script.quarantineMoved.0") });
                                  treeChanged();
                                })
                                .catch((e: unknown) => {
                                  sayError(e, t);
                                })
                                .finally(() => {
                                  setBusy(false);
                                });
                            }}
                          />
                        ) : null}
                      </td>
                      <td className="num">{fmtBytes(s.node.onDisk, lang)}</td>
                      <td>
                        <div className="btns">
                          {s.description.actions.map((a, i) => (
                            <button
                              key={`${a.type}${String(i)}`}
                              className="btn small"
                              onClick={() => {
                                if (a.type === "quarantine") {
                                  api
                                    .quarantinePreview(generation, [s.node.id])
                                    .then((plan) => {
                                      setPreview({ id: s.node.id, plan });
                                    })
                                    .catch((e: unknown) => {
                                      sayError(e, t);
                                    });
                                } else if (a.type === "showInExplorer") {
                                  api.showInExplorer(generation, s.node.id).catch((e: unknown) => {
                                    sayError(e, t);
                                  });
                                } else if (a.type === "windowsTool") {
                                  api.openTool(a.tool).catch((e: unknown) => {
                                    sayError(e, t);
                                  });
                                } else if (a.type === "uninstaller") {
                                  api.openTool("apps").catch((e: unknown) => {
                                    sayError(e, t);
                                  });
                                }
                              }}
                            >
                              {actionLabel(a, t)}
                            </button>
                          ))}
                        </div>
                      </td>
                    </tr>
                  );
                })}
              </tbody>
            </table>
          </section>
        );
      })}
    </div>
  );
}

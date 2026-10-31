// The list next to the map (its accessible equivalent) and the details panel with the
// explanation, the tier and the actions. Every move starts with a preview.
import { useEffect, useRef, useState, type JSX } from "react";
import { useShallow } from "zustand/react/shallow";
import { useTranslation } from "react-i18next";

import { api, errorKey } from "../api/ipc";
import type { Action, Details, MovePlan, Outcome, RevealedFolder, ViewResponse } from "../api/types";
import { fmtBytes, fmtInt, fmtShare } from "../format";
import { useApp } from "../store";
import { MovePreview, actionLabel } from "./Preview";

/** The sorted list of the focused folder's children. */
export function ItemList({ resp }: { resp: ViewResponse | null }): JSX.Element {
  const { t } = useTranslation();
  const { lang, selected, select, hover, setFocus, measure, scan } = useApp(useShallow((s) => ({ lang: s.lang, selected: s.selected, select: s.select, hover: s.hover, setFocus: s.setFocus, measure: s.settings.measure, scan: s.scan })));
  const logical = measure === "logical";
  const children = resp?.view.children ?? [];
  const total = (logical ? resp?.view.node.logical : resp?.view.node.onDisk) ?? 1;
  const max = children[0] ? (logical ? children[0].logical : children[0].onDisk) || 1 : 1;
  const scanning = scan ? !scan.done : false;
  return (
    <div className="panel">
      <h2>{t("list.title")}</h2>
      <div
        className="list"
        role="listbox"
        aria-label={t("list.title")}
        onMouseLeave={() => {
          hover(null);
        }}
      >
        {resp && resp.hiddenBytes !== null && resp.hiddenBytes > 0 ? (
          <button className="row" role="option" aria-selected={selected === -1} onClick={() => { select(-1); }} onMouseEnter={() => { hover(-1); }}>
            <span className="sw" style={{ background: "repeating-linear-gradient(45deg,var(--c-hidden) 0 3px,transparent 3px 6px)" }} />
            <span className="nm">{t("map.hiddenSpace")}</span>
            <span className="sz">{fmtBytes(resp.hiddenBytes, lang)}</span>
            <span className="bar" />
            <span className="pc" />
          </button>
        ) : null}
        {children.map((c) => {
          const v = logical ? c.logical : c.onDisk;
          return (
            <button
              key={c.id}
              className="row"
              role="option"
              aria-selected={selected === c.id}
              onClick={() => {
                select(c.id);
              }}
              onMouseEnter={() => {
                hover(c.id);
              }}
              onDoubleClick={() => {
                if (c.hasChildren && !scanning) setFocus(c.id);
              }}
            >
              <span className="sw" style={{ background: `var(--c-${c.category})` }} />
              <span className="nm">
                {c.name}
                {c.hasChildren ? " ›" : ""}
              </span>
              <span className="sz">{c.unscanned ? t("list.unknown") : fmtBytes(v, lang)}</span>
              <span className="bar">
                <i style={{ width: `${Math.max(2, (v / max) * 100)}%`, background: `var(--c-${c.category})` }} />
              </span>
              <span className="pc">{c.unscanned ? "" : fmtShare(v / (total || 1), lang)}</span>
            </button>
          );
        })}
      </div>
    </div>
  );
}

function isMove(a: Action): boolean {
  return a.type === "quarantine";
}

/** The details panel for the selected item (or the focused folder). */
export function DetailsPanel({ resp }: { resp: ViewResponse | null }): JSX.Element {
  const { t } = useTranslation();
  const { lang, selected, scan, focus, setFocus, say, treeChanged, sayError } = useApp(useShallow((s) => ({ lang: s.lang, selected: s.selected, scan: s.scan, focus: s.focus, setFocus: s.setFocus, say: s.say, treeChanged: s.treeChanged, sayError: s.sayError })));
  const [loaded, setLoaded] = useState<{ key: string; details: Details } | null>(null);
  const [planFor, setPlanFor] = useState<{ key: string; plan: MovePlan } | null>(null);
  const [revealed, setRevealed] = useState<RevealedFolder[] | null>(null);
  const [busy, setBusy] = useState(false);
  /** The selection the companion last explained, so a refresh does not repeat it. */
  const announced = useRef<string | null>(null);
  const generation = scan?.generation;
  const id = selected ?? focus;
  // While a scan runs, each new view (once per tick) reloads the sizes shown here.
  const refresh = scan && !scan.done ? resp : null;
  const key = `${String(generation)}-${String(id)}`;
  const details = loaded?.key === key ? loaded.details : null;
  const plan = planFor?.key === key ? planFor.plan : null;
  const setPlan = (p: MovePlan | null): void => {
    setPlanFor(p ? { key, plan: p } : null);
  };

  useEffect(() => {
    if (generation === undefined || id === -1) return;
    const k = `${String(generation)}-${String(id)}`;
    let alive = true;
    api
      .details(generation, id)
      .then((d) => {
        if (!alive) return;
        setLoaded({ key: k, details: d });
        if (selected !== null && announced.current !== k) {
          announced.current = k;
          const tierKey = d.description.protected ? "protected" : d.description.tier;
          if (tierKey) say({ type: "explanation", text: t(`companion.script.selected.${tierKey}.0`, { name: d.node.name }) });
          if (d.description.cloud && d.node.onDisk > 0) say({ type: "tip", text: t("companion.script.cloudIntro.0") });
        }
      })
      .catch(() => {
        if (alive) setLoaded(null);
      });
    return () => {
      alive = false;
    };
  }, [generation, id, selected, say, t, refresh]);

  if (id === -1 && resp?.hiddenBytes !== null && resp?.hiddenBytes !== undefined) {
    return (
      <div className="panel details">
        <div className="d-head">
          <span className="sw" style={{ background: "var(--c-hidden)" }} />
          <div className="d-name">{t("map.hiddenSpace")}</div>
        </div>
        <div className="stats">
          <div>
            <span>{t("details.sizeOnDisk")}</span>
            <b>{fmtBytes(resp.hiddenBytes, lang)}</b>
          </div>
        </div>
        <p className="info">{t("companion.script.hiddenSpace.0", { size: fmtBytes(resp.hiddenBytes, lang) })}</p>
        {revealed ? (
          <ul className="note">
            {revealed.map((r) => (
              <li key={r.path}>
                {r.path} · {r.error ? t(errorKey({ code: "fs", error: r.error })) : fmtBytes(r.onDisk, lang)}
              </li>
            ))}
          </ul>
        ) : null}
        <div className="btns">
          <button
            className="btn primary"
            disabled={busy || generation === undefined}
            onClick={() => {
              if (generation === undefined) return;
              setBusy(true);
              api
                .revealHidden(generation)
                .then((r) => {
                  if ("Ok" in r) {
                    setRevealed(r.Ok);
                    const size = r.Ok.reduce((s, f) => s + f.onDisk, 0);
                    say({ type: "explanation", text: t("companion.script.revealDone.0", { size: fmtBytes(size, lang) }) });
                  } else if (r.Err.kind === "declined") {
                    say({ type: "explanation", text: t("companion.script.revealDeclined.0") });
                  } else {
                    say({ type: "warning", text: t("errors.invalidArgument") });
                  }
                })
                .catch((e: unknown) => {
                  sayError(e, t);
                })
                .finally(() => {
                  setBusy(false);
                });
            }}
          >
            {t("actions.reveal")}
          </button>
        </div>
      </div>
    );
  }

  if (!details || generation === undefined) {
    return <div className="panel details note">{t("details.nothing")}</div>;
  }
  const d = details;
  const ruleId = d.description.ruleId;
  const generic = d.description.generic;
  const explain = ruleId ? t(`knownLocations.${ruleId}.explain`) : generic ? t("details.generic") : null;
  const reason = ruleId ? t(`knownLocations.${ruleId}.reason`) : generic ? t("details.genericReason") : null;
  const title = ruleId ? t(`knownLocations.${ruleId}.title`) : null;

  const run = (a: Action): void => {
    if (a.type === "quarantine") {
      api
        .quarantinePreview(generation, [d.node.id])
        .then((p) => {
          setPlan(p);
          say({ type: "explanation", text: t("companion.script.previewShown.0") });
        })
        .catch((e: unknown) => {
          sayError(e, t);
        });
    } else if (a.type === "showInExplorer") {
      api.showInExplorer(generation, d.node.id).catch((e: unknown) => {
        sayError(e, t);
      });
    } else if (a.type === "windowsTool") {
      say({ type: "tip", text: t("companion.script.windowsTool.0") });
      api.openTool(a.tool).catch((e: unknown) => {
        sayError(e, t);
      });
    } else if (a.type === "uninstaller") {
      api.openTool("apps").catch((e: unknown) => {
        sayError(e, t);
      });
    } else if (a.type === "freeUpSpace") {
      useApp.getState().setScreen("cloud");
    }
  };

  return (
    <div className="panel details">
      <div className="d-head">
        <span className="sw" style={{ background: `var(--c-${d.node.category})` }} />
        <div>
          <div className="d-name">{d.node.name}</div>
          {title ? <div className="note">{title}</div> : null}
          <div className="d-path">{d.path}</div>
        </div>
      </div>
      <div className="stats">
        <div>
          <span>{t("details.sizeOnDisk")}</span>
          <b>{d.node.unscanned ? t("list.unknown") : fmtBytes(d.node.onDisk, lang)}</b>
        </div>
        <div>
          <span>{t("details.logical")}</span>
          <b>{d.node.unscanned ? t("list.unknown") : fmtBytes(d.node.logical, lang)}</b>
        </div>
        <div>
          <span>{t("details.files")}</span>
          <b>{fmtInt(d.node.files, lang)}</b>
        </div>
      </div>
      <div className="btns">
        {d.description.tier ? <span className={`chip tier-${d.description.tier}`}>{t(`tier.${d.description.tier}`)}</span> : null}
        {d.description.protected ? <span className="chip tier-keep">{t("details.protected")}</span> : null}
      </div>
      {d.cloud ? (
        <p className="note">{t("details.cloudState", { local: fmtBytes(d.cloud.localBytes, lang), cloud: fmtBytes(d.cloud.cloudBytes, lang) })}</p>
      ) : null}
      {d.node.unscanned ? <p className="info">{t("details.unreadable")}</p> : null}
      {explain ? <p className="info">{explain}</p> : <p className="note">{t("details.folder", { count: d.node.files, files: fmtInt(d.node.files, lang) })}</p>}
      {reason ? <p className="note">{reason}</p> : null}
      {d.description.requiresAdmin ? <p className="note">{t("details.requiresAdmin")}</p> : null}
      {plan ? (
        <MovePreview
          plan={plan}
          busy={busy}
          onCancel={() => {
            setPlan(null);
          }}
          onConfirm={() => {
            setBusy(true);
            api
              .quarantineMove(generation, [d.node.id])
              .then((r) => {
                setPlan(null);
                const moved = r.results.filter((x) => x.outcome.outcome === "moved").length;
                if (moved > 0) say({ type: "explanation", text: t("companion.script.quarantineMoved.0") });
                const failed = r.results
                  .map((x) => x.outcome)
                  .find((o): o is Extract<Outcome, { outcome: "failed" }> => o.outcome === "failed");
                if (failed) say({ type: "warning", text: t(`errors.fs.${failed.kind}`) });
                useApp.getState().select(null);
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
      ) : (
        <div className="btns">
          {d.node.hasChildren && d.node.id !== focus ? (
            <button
              className="btn"
              onClick={() => {
                setFocus(d.node.id);
              }}
            >
              {t("actions.zoomIn")}
            </button>
          ) : null}
          {d.description.actions.map((a, i) => (
            <button
              key={`${a.type}${String(i)}`}
              className={`btn${i === 0 && (isMove(a) || a.type !== "showInExplorer") ? " primary" : ""}`}
              onClick={() => {
                run(a);
              }}
            >
              {actionLabel(a, t)}
            </button>
          ))}
        </div>
      )}
    </div>
  );
}

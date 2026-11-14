// Suggestions: every recognised item and every large unrecognised file, by tier and size.
// Items of the same kind form one expandable group with one preview for all of them
// (AC4.13). Nothing is pre-selected; each move starts with its preview (S1).
import { startTransition, useEffect, useState, type JSX } from "react";
import { useShallow } from "zustand/react/shallow";
import { useTranslation } from "react-i18next";

import { api } from "../api/ipc";
import type { Action, MovePlan, Suggestion, Tier } from "../api/types";
import { MovePreview, actionLabel } from "../components/Preview";
import { explainShort, leftoverText } from "../explain";
import { fmtBytes } from "../format";
import { useProgressive } from "../progressive";
import { useApp } from "../store";

const TIERS: Tier[] = ["safe", "likelySafe", "review", "keep"];

/** The group a suggestion belongs to: its rule, or what app knowledge says it is. */
export function groupKey(s: Suggestion): string {
  const d = s.description;
  if (d.steam) return d.steam.kind === "game" ? "steam:game" : "steam:leftover";
  if (d.ruleId) return `rule:${d.ruleId}`;
  if (d.program) return "program";
  if (d.leftover) return "leftover";
  return "large";
}

/** Suggestions grouped by [`groupKey`], largest group first, items largest first. */
export function groupSuggestions(items: Suggestion[]): { key: string; items: Suggestion[]; total: number }[] {
  const groups = new Map<string, Suggestion[]>();
  for (const s of items) {
    const k = groupKey(s);
    const g = groups.get(k);
    if (g) g.push(s);
    else groups.set(k, [s]);
  }
  return [...groups.entries()]
    .map(([key, list]) => ({
      key,
      items: [...list].sort((a, b) => b.node.onDisk - a.node.onDisk),
      total: list.reduce((n, s) => n + s.node.onDisk, 0),
    }))
    .sort((a, b) => b.total - a.total);
}

/** How much of `ids` is in the selection. */
export function selectionState(selected: Set<number>, ids: number[]): "none" | "some" | "all" {
  const n = ids.filter((id) => selected.has(id)).length;
  return n === 0 ? "none" : n === ids.length ? "all" : "some";
}

/** Selects all of `ids`, or none of them when all already are. */
export function toggleAll(selected: Set<number>, ids: number[]): Set<number> {
  const next = new Set(selected);
  if (selectionState(selected, ids) === "all") for (const id of ids) next.delete(id);
  else for (const id of ids) next.add(id);
  return next;
}

/** The selection without items that are no longer offered (moved, or gone after a rescan). */
export function keepOffered(selected: Set<number>, offered: number[]): Set<number> {
  const keep = new Set(offered);
  return new Set([...selected].filter((id) => keep.has(id)));
}

/** True when the suggestion can be moved to the quarantine. */
function movable(s: Suggestion): boolean {
  return s.description.actions.some((a) => a.type === "quarantine");
}

/** A checkbox that also shows a partly selected group. */
function Check({ state, label, onChange }: { state: "none" | "some" | "all"; label: string; onChange: () => void }): JSX.Element {
  return (
    <input
      type="checkbox"
      className="pick"
      aria-label={label}
      checked={state === "all"}
      ref={(el) => {
        if (el) el.indeterminate = state === "some";
      }}
      onChange={onChange}
    />
  );
}

/** The Suggestions screen. */
export function Suggestions(): JSX.Element {
  const { t } = useTranslation();
  const { lang, scan, treeVersion, say, sayError, treeChanged } = useApp(useShallow((s) => ({ lang: s.lang, scan: s.scan, treeVersion: s.treeVersion, say: s.say, sayError: s.sayError, treeChanged: s.treeChanged })));
  const [items, setItems] = useState<Suggestion[] | null>(null);
  // One tier more per frame, so opening the screen never lays out every row at once (P11).
  const tiersShown = useProgressive(TIERS.length, items);
  const [open, setOpen] = useState<Set<string>>(new Set());
  const [preview, setPreview] = useState<{ key: string; ids: number[]; plan: MovePlan } | null>(null);
  const [busy, setBusy] = useState(false);
  // Nothing is selected at the start (S1); each tick is a user click.
  const [selected, setSelected] = useState<Set<number>>(new Set());
  const done = scan?.done ?? false;

  useEffect(() => {
    if (!done) return;
    let alive = true;
    api
      .suggestions(2000)
      .then(([, list]) => {
        // Loaded data renders as an interruptible update, so opening the screen never blocks
        // input with one long task (P1, P11).
        if (alive) startTransition(() => {
          setItems(list);
          setSelected((sel) => keepOffered(sel, list.filter(movable).map((x) => x.node.id)));
        });
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

  const showPreview = (key: string, ids: number[]): void => {
    api
      .quarantinePreview(generation, ids)
      .then((plan) => {
        setPreview({ key, ids, plan });
      })
      .catch((e: unknown) => {
        sayError(e, t);
      });
  };

  const run = (s: Suggestion, a: Action): void => {
    const fail = (e: unknown): void => {
      sayError(e, t);
    };
    if (a.type === "quarantine") showPreview(`item:${String(s.node.id)}`, [s.node.id]);
    else if (a.type === "showInExplorer") api.showInExplorer(generation, s.node.id).catch(fail);
    else if (a.type === "windowsTool") api.openTool(a.tool).catch(fail);
    else if (a.type === "uninstaller") api.openTool("apps").catch(fail);
    else if (a.type === "appUninstall") api.appUninstall(generation, s.node.id).catch(fail);
    else if (a.type === "freeUpSpace") useApp.getState().setScreen("cloud");
  };

  const previewSheet = (key: string): JSX.Element | null =>
    preview?.key === key ? (
      <MovePreview
        plan={preview.plan}
        busy={busy}
        onCancel={() => {
          setPreview(null);
        }}
        onConfirm={() => {
          setBusy(true);
          api
            .quarantineMove(generation, preview.ids)
            .then(() => {
              setPreview(null);
              setSelected(new Set());
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
    ) : null;

  const title = (s: Suggestion): string => {
    const d = s.description;
    if (d.steam?.kind === "game") return d.steam.name;
    if (d.program) return d.program.name;
    return d.ruleId ? t(`knownLocations.${d.ruleId}.title`) : s.node.name;
  };
  const note = (s: Suggestion): string => {
    const d = s.description;
    if (d.steam?.kind === "game") return t("facts.steamGameAction");
    if (d.steam?.kind === "leftover") return t("facts.steamLeftover");
    if (d.ruleId) return t(`knownLocations.${d.ruleId}.action`);
    if (d.program) return t("facts.programAction");
    if (d.leftover) return leftoverText(d.leftover, t, lang);
    return explainShort({ ...s.node, ruleId: null, explain: d.explain }, t);
  };
  const groupTitle = (key: string, first: Suggestion): string => {
    if (key.startsWith("rule:")) return t(`knownLocations.${key.slice(5)}.title`);
    if (key === "steam:game") return t("suggestions.groups.steamGames");
    if (key === "steam:leftover") return t("suggestions.groups.steamLeftovers");
    if (key === "program") return t("suggestions.groups.programs");
    if (key === "large") return t("suggestions.groups.large");
    if (key === "leftover") return t("suggestions.groups.leftover");
    return title(first);
  };

  const pick = (ids: number[]): void => {
    setSelected((sel) => toggleAll(sel, ids));
  };
  const row = (s: Suggestion): JSX.Element => (
    <tr key={s.node.id}>
      <td className="pick-cell">
        {movable(s) ? <Check state={selected.has(s.node.id) ? "all" : "none"} label={t("suggestions.select", { name: title(s) })} onChange={() => { pick([s.node.id]); }} /> : null}
      </td>
      <td>
        <div>{title(s)}</div>
        <div className="path">{s.path}</div>
        <div className="note">{note(s)}</div>
        {previewSheet(`item:${String(s.node.id)}`)}
      </td>
      <td className="num">{fmtBytes(s.node.onDisk, lang)}</td>
      <td>
        <div className="btns">
          {s.description.actions
            .filter((a) => a.type !== "explainOnly")
            .map((a, i) => (
              <button
                key={`${a.type}${String(i)}`}
                className="btn small"
                onClick={() => {
                  run(s, a);
                }}
              >
                {actionLabel(a, t)}
              </button>
            ))}
        </div>
      </td>
    </tr>
  );

  return (
    <div className="page">
      <header>
        <h1>{t("nav.suggestions")}</h1>
        <span className="note">{t("suggestions.selectHint")}</span>
      </header>
      {TIERS.slice(0, tiersShown).map((tier) => {
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
                {groupSuggestions(list).map((g) => {
                  const [first] = g.items;
                  if (!first) return null;
                  if (g.items.length === 1) return row(first);
                  const groupId = `${tier}:${g.key}`;
                  const expanded = open.has(groupId);
                  const movable = g.items.filter((s) => s.description.actions.some((a) => a.type === "quarantine")).map((s) => s.node.id);
                  return [
                    <tr key={groupId} className="group-row">
                      <td className="pick-cell">
                        {movable.length > 0 ? (
                          <Check
                            state={selectionState(selected, movable)}
                            label={t("suggestions.select", { name: groupTitle(g.key, first) })}
                            onChange={() => {
                              pick(movable);
                            }}
                          />
                        ) : null}
                      </td>
                      <td>
                        <button
                          className="group-toggle"
                          aria-expanded={expanded}
                          onClick={() => {
                            setOpen((o) => {
                              const n = new Set(o);
                              if (n.has(groupId)) n.delete(groupId);
                              else n.add(groupId);
                              return n;
                            });
                          }}
                        >
                          <svg className={`chev${expanded ? " open" : ""}`} viewBox="0 0 10 10" aria-hidden="true">
                            <path d="M3 1.5 7 5 3 8.5" />
                          </svg>
                          {groupTitle(g.key, first)}
                        </button>
                        <div className="note">{t("suggestions.count", { count: g.items.length })}</div>
                        {previewSheet(`group:${groupId}`)}
                      </td>
                      <td className="num">{fmtBytes(g.total, lang)}</td>
                      <td>
                        {movable.length > 0 ? (
                          <button
                            className="btn small"
                            onClick={() => {
                              showPreview(`group:${groupId}`, movable);
                            }}
                          >
                            {t("suggestions.moveAll", { count: movable.length })}
                          </button>
                        ) : null}
                      </td>
                    </tr>,
                    ...(expanded ? g.items.map(row) : []),
                  ];
                })}
              </tbody>
            </table>
          </section>
        );
      })}
      {selected.size > 0 ? (
        <div className="select-bar panel" role="region" aria-label={t("suggestions.moveSelected")}>
          <b>{t("suggestions.selected", { count: selected.size, size: fmtBytes(items.filter((x) => selected.has(x.node.id)).reduce((n, x) => n + x.node.onDisk, 0), lang) })}</b>
          <button
            className="btn small primary"
            onClick={() => {
              showPreview("selection", [...selected]);
            }}
          >
            {t("suggestions.moveSelected")}
          </button>
          <button
            className="btn small"
            onClick={() => {
              setSelected(new Set());
              setPreview(null);
            }}
          >
            {t("suggestions.clearSelection")}
          </button>
          {previewSheet("selection")}
        </div>
      ) : null}
    </div>
  );
}

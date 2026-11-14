// Optimise (F16): the honest check-up. Findings in six groups, each with its state, what was
// measured, the source and the next step; no score. The only action is switching a startup
// app of the current user, with a preview first and an undo after.
import { startTransition, useCallback, useEffect, useState, type JSX } from "react";
import { useShallow } from "zustand/react/shallow";
import { useTranslation } from "react-i18next";

import { api } from "../api/ipc";
import type { Finding, FindingGroup, StartupChange, StartupScope } from "../api/types";
import { fmtBytes } from "../format";
import { useProgressive } from "../progressive";
import { useApp } from "../store";

const GROUPS: FindingGroup[] = ["performance", "storage", "startup", "space", "health", "config"];
/** Values that are sizes in bytes, shown as sizes. */
const SIZES = new Set(["total", "available", "commit", "commitLimit", "memory", "free", "size"]);

/** The kind of a finding, from its ID (`disk:Name` is a `disk`). */
function kindOf(id: string): string {
  return id.split(":")[0] ?? id;
}

/** The startup scope in a finding ID (`startup:UserRun:Name`), as the command takes it. */
function scopeOf(id: string): StartupScope | null {
  const raw = id.split(":")[1] ?? "";
  const map: Record<string, StartupScope> = { UserRun: "userRun", MachineRun: "machineRun", UserFolder: "userFolder", CommonFolder: "commonFolder" };
  return map[raw] ?? null;
}

/** Values Windows reports as words, shown in the user's language. */
const WORDS = new Set(["health", "mode", "on"]);

/** One finding's values as text parameters: sizes formatted, Windows' words translated. */
function params(f: Finding, lang: "en" | "nl", t: (k: string) => string): Record<string, string> {
  return Object.fromEntries(
    f.values.map(([k, v]) => [k, SIZES.has(k) ? fmtBytes(Number(v), lang) : WORDS.has(k) ? t(`optimise.words.${k}.${v}`) : v]),
  );
}

/** The last check-up, kept while the app runs: opening the screen again shows it at once
 * instead of measuring for seconds; "Check again" measures anew. */
let lastCheck: { findings: Finding[]; updates: Finding[]; at: number } | null = null;

/** The Optimise screen. */
export function Optimise(): JSX.Element {
  const { t } = useTranslation();
  const { lang, say, sayError, setScreen } = useApp(useShallow((s) => ({ lang: s.lang, say: s.say, sayError: s.sayError, setScreen: s.setScreen })));
  const [findings, setFindings] = useState<Finding[] | null>(lastCheck?.findings ?? null);
  const [at, setAt] = useState<number | null>(lastCheck?.at ?? null);
  // One group more per frame, so opening the screen never lays out every finding at once (P11).
  const groupsShown = useProgressive(GROUPS.length, findings);
  const [updates, setUpdates] = useState<Finding[] | "loading">(lastCheck?.updates ?? "loading");
  const [preview, setPreview] = useState<{ finding: Finding; change: StartupChange } | null>(null);
  const [undo, setUndo] = useState<Map<string, number>>(new Map());

  const fetchAll = useCallback(() => {
    api
      .optimiseCheck()
      // Loaded data renders as an interruptible update, so opening the screen never blocks
      // input with one long task (P1, P11).
      .then((f) => {
        const now = Date.now();
        lastCheck = { findings: f, updates: lastCheck?.updates ?? [], at: now };
        startTransition(() => {
          setFindings(f);
          setAt(now);
        });
      })
      .catch((e: unknown) => {
        setFindings([]);
        sayError(e, t);
      });
    api
      .optimiseUpdates()
      .then((u) => {
        if (lastCheck) lastCheck.updates = u;
        startTransition(() => {
          setUpdates(u);
        });
      })
      .catch(() => {
        setUpdates([]);
      });
  }, [sayError, t]);
  // Measure only on the first visit; later visits show the last check-up.
  useEffect(() => {
    if (!lastCheck) fetchAll();
  }, [fetchAll]);
  const load = (): void => {
    setFindings(null);
    setUpdates("loading");
    fetchAll();
  };

  const ask = (f: Finding, enabled: boolean): void => {
    const scope = scopeOf(f.id);
    const name = f.values.find(([k]) => k === "name")?.[1];
    if (!scope || !name) return;
    api
      .startupPreview(scope, name, enabled)
      .then((r) => {
        if ("Ok" in r) setPreview({ finding: f, change: r.Ok });
        else say({ type: "warning", text: t(`optimise.startupErrors.${r.Err}`) });
      })
      .catch((e: unknown) => {
        sayError(e, t);
      });
  };
  const apply = (): void => {
    if (!preview) return;
    const { finding, change } = preview;
    api
      .startupSet(change.scope, change.name, change.enabled)
      .then((r) => {
        setPreview(null);
        if ("Ok" in r) {
          setUndo((m) => new Map(m).set(finding.id, r.Ok));
          say({ type: "explanation", text: t(change.enabled ? "optimise.startupOn" : "optimise.startupOff", { name: change.name }) });
          load();
        } else say({ type: "warning", text: t(`optimise.startupErrors.${r.Err}`) });
      })
      .catch((e: unknown) => {
        sayError(e, t);
      });
  };
  const revert = (f: Finding): void => {
    const index = undo.get(f.id);
    if (index === undefined) return;
    api
      .startupUndo(index)
      .then((r) => {
        if ("Ok" in r) {
          setUndo((m) => {
            const n = new Map(m);
            n.delete(f.id);
            return n;
          });
          load();
        } else say({ type: "warning", text: t(`optimise.startupErrors.${r.Err}`) });
      })
      .catch((e: unknown) => {
        sayError(e, t);
      });
  };

  const all = [...(findings ?? []), ...(updates === "loading" ? [] : updates)];
  const row = (f: Finding): JSX.Element => {
    const kind = kindOf(f.id);
    const p = params(f, lang, t);
    const enabled = p["enabled"] === "true";
    const known = p["known"];
    if (kind === "partition") {
      const k = p["kind"] ?? "other";
      const facts = [
        p["letter"] ? t("optimise.partition.drive", { letter: p["letter"] }) : null,
        p["label"] ?? null,
        p["fileSystem"] ?? null,
        p["size"] ?? null,
        p["free"] ? t("optimise.partition.free", { free: p["free"] }) : null,
      ].filter((x): x is string => x !== null);
      return (
        <li key={f.id} className="finding sub">
          <span className="chip part">{p["number"]}</span>
          <div>
            <div className="finding-head">
              <b>{t(`optimise.partition.kind.${k}.name`)}</b>
              <span className="note">{facts.join(" · ")}</span>
            </div>
            <p className="note">{t(`optimise.partition.kind.${k}.text`)}</p>
          </div>
        </li>
      );
    }
    const title =
      kind === "process" && known ? `${t(`processes.${known}.name`)} (${p["name"] ?? ""})` : t(`optimise.findings.${kind}.title`, p);
    const buttons: JSX.Element[] = [];
    if (kind === "startup" && f.state === "action")
      buttons.push(
        <button
          key="toggle"
          className="btn small"
          onClick={() => {
            ask(f, !enabled);
          }}
        >
          {enabled ? t("optimise.turnOff") : t("optimise.turnOn")}
        </button>,
      );
    if (undo.has(f.id))
      buttons.push(
        <button
          key="undo"
          className="btn small"
          onClick={() => {
            revert(f);
          }}
        >
          {t("optimise.undo")}
        </button>,
      );
    if (kind === "freeSpace" && f.state !== "ok")
      buttons.push(
        <button
          key="suggestions"
          className="btn small"
          onClick={() => {
            setScreen("suggestions");
          }}
        >
          {t("optimise.openSuggestions")}
        </button>,
      );
    if (kind === "memory")
      buttons.push(
        <button
          key="tm"
          className="btn small"
          onClick={() => {
            api.openTool("taskManager").catch((e: unknown) => {
              sayError(e, t);
            });
          }}
        >
          {t("optimise.openTaskManager")}
        </button>,
      );
    if (kind === "disk")
      buttons.push(
        <button
          key="disks"
          className="btn small"
          onClick={() => {
            api.openTool("disks").catch((e: unknown) => {
              sayError(e, t);
            });
          }}
        >
          {t("optimise.openDisks")}
        </button>,
      );
    if (kind === "updates" && f.state !== "ok")
      buttons.push(
        <button
          key="wu"
          className="btn small"
          onClick={() => {
            api.openTool("windowsUpdate").catch((e: unknown) => {
              sayError(e, t);
            });
          }}
        >
          {t("optimise.openWindowsUpdate")}
        </button>,
      );
    return (
      <li key={f.id} className={`finding ${f.state}`}>
        <span className={`chip state-${f.state}`}>{t(`optimise.state.${f.state}`)}</span>
        <div>
          <div className="finding-head">
            <b>{title}</b>
            {buttons.length > 0 ? <span className="btns">{buttons}</span> : null}
          </div>
          {kind === "process" ? <p className="info">{known ? t(`processes.${known}.what`) : t("optimise.unknownProcess")}</p> : null}
          {kind === "startup" && known ? <p className="info">{t(`processes.${known}.what`)}</p> : null}
          <p className={kind === "process" ? "note" : "info"}>{t(`optimise.findings.${kind}.text`, p)}</p>
          {kind === "startup" && p["missing"] === "true" ? <p className="note">{t("optimise.findings.startup.missing")}</p> : null}
          {f.state !== "ok" ? <p className="note">{t(`optimise.findings.${kind}.next`, p)}</p> : null}
          <p className="note source">{t(`optimise.source.${f.source}`)}</p>
        </div>
      </li>
    );
  };

  return (
    <div className="page">
      <header>
        <h1>{t("optimise.title")}</h1>
        <p className="note">{t("optimise.lead")}</p>
        <button className="btn small" onClick={load}>
          {t("optimise.again")}
        </button>
        {at !== null && findings !== null ? <span className="note">{t("optimise.lastMeasured", { time: new Date(at).toLocaleTimeString(lang, { hour: "2-digit", minute: "2-digit" }) })}</span> : null}
      </header>
      {findings === null ? <p className="note">{t("optimise.measuring")}</p> : null}
      {GROUPS.slice(0, groupsShown).map((g) => {
        const list = all.filter((f) => f.group === g);
        const waiting = g === "health" && updates === "loading";
        if (list.length === 0 && !waiting) return null;
        return (
          <section key={g} className="panel">
            <h2>{t(`optimise.groups.${g}`)}</h2>
            <ul className="findings">{list.map(row)}</ul>
            {waiting ? <p className="note">{t("optimise.updatesLoading")}</p> : null}
          </section>
        );
      })}
      <section className="panel">
        <h2>{t("optimise.mythsTitle")}</h2>
        <p className="note">{t("optimise.myths")}</p>
      </section>
      {preview ? (
        <div className="overlay" role="dialog" aria-modal="true" aria-label={t("optimise.previewTitle")}>
          <div className="welcome">
            <h2>{t("optimise.previewTitle")}</h2>
            <p className="info">{t(preview.change.enabled ? "optimise.previewOn" : "optimise.previewOff", { name: preview.change.name })}</p>
            <p className="note">{t("optimise.previewHow")}</p>
            <div className="btns">
              <button className="btn primary" onClick={apply}>
                {preview.change.enabled ? t("optimise.turnOn") : t("optimise.turnOff")}
              </button>
              <button
                className="btn"
                onClick={() => {
                  setPreview(null);
                }}
              >
                {t("optimise.cancel")}
              </button>
            </div>
          </div>
        </div>
      ) : null}
    </div>
  );
}

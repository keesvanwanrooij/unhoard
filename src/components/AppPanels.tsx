// What the details panel adds from app knowledge: the program or game an item belongs to,
// when it was last used, items read with admin rights, the button
// that reads an unreadable folder, and the Ollama and Docker panels (AC3.8, AC4.11, AC4.12).
import { useState, type JSX } from "react";
import { useShallow } from "zustand/react/shallow";
import { useTranslation } from "react-i18next";

import { api } from "../api/ipc";
import type { AppName, Details, DockerUsage, MovePlan, OllamaModel, ToolError } from "../api/types";
import { fmtAgo, fmtBytes } from "../format";
import { useApp } from "../store";
import { MovePreview } from "./Preview";

/** `YYYYMMDD` as a localised date. */
function installDate(d: string, lang: string): string {
  const date = new Date(Number(d.slice(0, 4)), Number(d.slice(4, 6)) - 1, Number(d.slice(6, 8)));
  return Number.isNaN(date.getTime()) ? d : date.toLocaleDateString(lang === "nl" ? "nl-NL" : "en-GB");
}

/** Program, game, last use and admin facts about an item. `now` in seconds. */
export function ItemFacts({ d, now }: { d: Details; now: number }): JSX.Element {
  const { t } = useTranslation();
  const lang = useApp((s) => s.lang);
  const desc = d.description;
  const p = desc.program;
  const steam = desc.steam;
  return (
    <>
      {p ? (
        <p className="info">
          {t("facts.program", { name: p.name, publisher: p.publisher ?? t("facts.unknownPublisher") })}
          {p.installDate ? ` ${t("facts.installed", { date: installDate(p.installDate, lang) })}` : ""}
        </p>
      ) : null}
      {steam?.kind === "game" ? (
        <>
          <p className="info">{t("facts.steamGame", { name: steam.name })}</p>
          <div className="btns">
            <OpenAppButton app="steam" />
          </div>
        </>
      ) : null}
      {steam?.kind === "leftover" ? <p className="info">{t("facts.steamLeftover")}</p> : null}
      <div className="facts">
        {desc.lastUse ? (
          <span>{t(`matrix.use.${desc.lastUse.source}`, { when: fmtAgo(desc.lastUse.at, now, lang) })}</span>
        ) : null}
      </div>
      {desc.elevated ? <p className="note">{t("facts.elevated")}</p> : null}
      {d.measured && d.measured.length > 0 ? (
        <ul className="note">
          {d.measured.slice(0, 10).map(([name, size]) => (
            <li key={name}>
              {name} · {fmtBytes(size, lang)}
            </li>
          ))}
        </ul>
      ) : null}
    </>
  );
}

/** "Read with admin rights" for one unreadable folder (AC3.8). */
export function AdminReadButton({ generation, id }: { generation: number; id: number }): JSX.Element {
  const { t } = useTranslation();
  const { lang, say, sayError, treeChanged } = useApp(useShallow((s) => ({ lang: s.lang, say: s.say, sayError: s.sayError, treeChanged: s.treeChanged })));
  const [busy, setBusy] = useState(false);
  return (
    <div className="btns">
      <button
        className="btn primary"
        disabled={busy}
        onClick={() => {
          setBusy(true);
          api
            .adminRead(generation, [id])
            .then((r) => {
              if ("Ok" in r) {
                say({ type: "explanation", text: t("companion.script.adminReadDone.0", { count: r.Ok.folders, size: fmtBytes(r.Ok.onDisk, lang) }) });
                treeChanged();
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
        {t("actions.adminRead")}
      </button>
    </div>
  );
}

/** Opens Docker Desktop, Ollama or Steam; says so when the app is not installed. */
export function OpenAppButton({ app }: { app: AppName }): JSX.Element {
  const { t } = useTranslation();
  const { say, sayError } = useApp(useShallow((s) => ({ say: s.say, sayError: s.sayError })));
  return (
    <button
      className="btn small"
      onClick={() => {
        api
          .openApp(app)
          .then((r) => {
            if ("Err" in r) say({ type: "warning", text: t(`adapters.open.errors.${r.Err.kind}`) });
          })
          .catch((e: unknown) => {
            sayError(e, t);
          });
      }}
    >
      {t(`adapters.open.${app}`)}
    </button>
  );
}

/** A command the user can copy. */
function CopyCommand({ command }: { command: string }): JSX.Element {
  const { t } = useTranslation();
  const [copied, setCopied] = useState(false);
  return (
    <div className="command">
      <code>{command}</code>
      <button
        className="btn small"
        onClick={() => {
          navigator.clipboard
            .writeText(command)
            .then(() => {
              setCopied(true);
            })
            .catch(() => {
              setCopied(false);
            });
        }}
      >
        {copied ? t("actions.copied") : t("actions.copy")}
      </button>
    </div>
  );
}

/** Ollama's models with their size, download date and a reversible move (AC4.12). */
export function OllamaPanel({ generation, now }: { generation: number; now: number }): JSX.Element {
  const { t } = useTranslation();
  const { lang, say, sayError, treeChanged } = useApp(useShallow((s) => ({ lang: s.lang, say: s.say, sayError: s.sayError, treeChanged: s.treeChanged })));
  const [models, setModels] = useState<OllamaModel[] | null>(null);
  const [preview, setPreview] = useState<{ name: string; ids: number[]; plan: MovePlan } | null>(null);
  const [busy, setBusy] = useState(false);
  if (!models) {
    return (
      <div className="btns">
        <OpenAppButton app="ollama" />
        <button
          className="btn"
          onClick={() => {
            api
              .ollamaModels(generation)
              .then(setModels)
              .catch((e: unknown) => {
                sayError(e, t);
              });
          }}
        >
          {t("adapters.ollama.show")}
        </button>
      </div>
    );
  }
  if (models.length === 0) return <p className="note">{t("adapters.ollama.none")}</p>;
  return (
    <div className="adapter">
      <div className="row-head">
        <b>{t("adapters.ollama.title")}</b>
        <OpenAppButton app="ollama" />
      </div>
      <p className="note">{t("adapters.ollama.explain")}</p>
      <ul className="models">
        {models.map((m) => (
          <li key={m.name}>
            <div className="row-head">
              <span className="nm">{m.name}</span>
              <span className="sz">{fmtBytes(m.uniqueBytes, lang)}</span>
            </div>
            <div className="note">
              {t("adapters.ollama.pulled", { when: fmtAgo(m.modified, now, lang) })}
              {m.bytes > m.uniqueBytes ? ` · ${t("adapters.ollama.shared", { size: fmtBytes(m.bytes - m.uniqueBytes, lang) })}` : ""}
            </div>
            {preview?.name === m.name ? (
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
                      setModels(null);
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
            ) : (
              <div className="btns">
                {m.ids ? (
                  <button
                    className="btn small"
                    onClick={() => {
                      const ids = m.ids ?? [];
                      api
                        .quarantinePreview(generation, ids)
                        .then((plan) => {
                          setPreview({ name: m.name, ids, plan });
                        })
                        .catch((e: unknown) => {
                          sayError(e, t);
                        });
                    }}
                  >
                    {t("adapters.ollama.quarantine")}
                  </button>
                ) : null}
                <CopyCommand command={`ollama rm ${m.name}`} />
              </div>
            )}
          </li>
        ))}
      </ul>
    </div>
  );
}

/** What fills Docker Desktop's disk, and how to clean and compact it (AC4.12). */
export function DockerPanel(): JSX.Element {
  const { t } = useTranslation();
  const lang = useApp((s) => s.lang);
  const [usage, setUsage] = useState<DockerUsage[] | null>(null);
  const [error, setError] = useState<ToolError | null>(null);
  const [busy, setBusy] = useState(false);
  return (
    <div className="adapter">
      <div className="row-head">
        <b>{t("adapters.docker.title")}</b>
        <OpenAppButton app="dockerDesktop" />
      </div>
      <p className="note">{t("adapters.docker.explain")}</p>
      {usage ? (
        <table className="table">
          <tbody>
            {usage.map((u) => (
              <tr key={u.kind}>
                <td>{t(`adapters.docker.kinds.${u.kind.replace(/\s/g, "")}`, { defaultValue: u.kind })}</td>
                <td className="num">{fmtBytes(u.bytes, lang)}</td>
                <td className="num">{t("adapters.docker.reclaimable", { size: fmtBytes(u.reclaimable, lang) })}</td>
              </tr>
            ))}
          </tbody>
        </table>
      ) : (
        <div className="btns">
          <button
            className="btn"
            disabled={busy}
            onClick={() => {
              setBusy(true);
              api
                .dockerUsage()
                .then((r) => {
                  if ("Ok" in r) {
                    setUsage(r.Ok);
                    setError(null);
                  } else {
                    setError(r.Err);
                  }
                })
                .catch(() => {
                  setError({ kind: "failed" });
                })
                .finally(() => {
                  setBusy(false);
                });
            }}
          >
            {t("adapters.docker.show")}
          </button>
        </div>
      )}
      {error ? <p className="note">{t(`adapters.docker.errors.${error.kind}`)}</p> : null}
      <ol className="steps">
        <li>
          {t("adapters.docker.buildCache")}
          <CopyCommand command="docker builder prune -a" />
        </li>
        <li>
          {t("adapters.docker.containers")}
          <CopyCommand command="docker container prune" />
        </li>
        <li>
          {t("adapters.docker.images")}
          <CopyCommand command="docker image prune -a" />
        </li>
        <li>
          {t("adapters.docker.volumes")}
          <CopyCommand command="docker volume prune" />
        </li>
        <li>{t("adapters.docker.compact")}</li>
      </ol>
      <p className="note">{t("adapters.docker.never")}</p>
    </div>
  );
}

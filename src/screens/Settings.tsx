// Settings: language, map style, motion, size measure, companion mode, background
// insights, the AI chat, and About.
import { useCallback, useEffect, useState, type JSX, type ReactNode } from "react";
import { useShallow } from "zustand/react/shallow";
import { useTranslation } from "react-i18next";

import { api, onChatPull } from "../api/ipc";
import type { ChatError, ChatSetup, PullProgress, Settings as S } from "../api/types";
import { fmtBytes } from "../format";
import { setLanguage } from "../i18n";
import { useApp } from "../store";

/** One setting: its name with a short explanation under it, and its control. */
function Field({ name, hint, children }: { name: string; hint: string; children: ReactNode }): JSX.Element {
  const { t } = useTranslation();
  return (
    <div className="field">
      <span>
        {t(name)}
        <small>{t(hint)}</small>
      </span>
      {children}
    </div>
  );
}

/** A choice between a few values of one setting. */
function Choice<K extends keyof S>({ field, options }: { field: K; options: [S[K], string][] }): JSX.Element {
  const { t } = useTranslation();
  const { settings, updateSettings } = useApp(useShallow((s) => ({ settings: s.settings, updateSettings: s.updateSettings })));
  return (
    <div className="seg" role="radiogroup">
      {options.map(([value, label]) => (
        <button
          key={String(value)}
          role="radio"
          aria-checked={settings[field] === value}
          aria-pressed={settings[field] === value}
          onClick={() => {
            updateSettings({ [field]: value });
          }}
        >
          {t(label)}
        </button>
      ))}
    </div>
  );
}

/** Sets up the chat: explains Ollama when it is missing, starts it when it is not running,
 * recommends a model for this PC and downloads it on a click, and picks the model. */
function ChatSetupPanel(): JSX.Element {
  const { t } = useTranslation();
  const { settings, updateSettings, lang, say, sayError } = useApp(
    useShallow((s) => ({ settings: s.settings, updateSettings: s.updateSettings, lang: s.lang, say: s.say, sayError: s.sayError })),
  );
  const [setup, setSetup] = useState<ChatSetup | null>(null);
  const [problem, setProblem] = useState<ChatError | null>(null);
  const [pull, setPull] = useState<PullProgress | null>(null);
  const [pulling, setPulling] = useState(false);

  const load = useCallback(() => {
    // Save the settings first: the chat may have been switched on a moment ago, and the
    // core asks Ollama only when the saved setting says the chat is on (AC8.6).
    api
      .settingsSet(useApp.getState().settings)
      .then(() => api.chatSetup())
      .then((r) => {
        if ("Ok" in r) {
          setSetup(r.Ok);
          setProblem(null);
        } else setProblem(r.Err);
      })
      .catch(() => {
        setProblem("badReply");
      });
  }, []);
  useEffect(load, [load]);
  /** Looks again, after the user installed or started Ollama. */
  const check = (): void => {
    setSetup(null);
    setProblem(null);
    load();
  };
  useEffect(() => {
    const off = onChatPull(setPull);
    return () => {
      off.then((f) => {
        f();
      }).catch(() => undefined);
    };
  }, []);

  const download = (model: string): void => {
    setPulling(true);
    setPull(null);
    api
      .chatPull(model)
      .then((r) => {
        if ("Ok" in r && r.Ok) {
          updateSettings({ chatModel: model });
          say({ type: "explanation", text: t("chat.setup.pulled", { model }) });
          check();
        } else say({ type: "warning", text: t("chat.setup.pullFailed") });
      })
      .catch((e: unknown) => {
        sayError(e, t);
      })
      .finally(() => {
        setPulling(false);
      });
  };

  if (problem) return <p className="note warn">{t(`chat.errors.${problem}`, { model: settings.chatModel ?? "qwen3:8b" })}</p>;
  if (!setup) return <p className="note">{t("chat.setup.checking")}</p>;
  const again = (
    <button className="btn small" onClick={check}>
      {t("chat.setup.again")}
    </button>
  );
  if (setup.ollama !== "running") {
    const installed = setup.ollama === "notRunning";
    return (
      <div className="chat-setup">
        <p className="info">{t("chat.setup.whatIsOllama")}</p>
        <p className="info">
          <b>{t(installed ? "chat.setup.notRunning" : "chat.setup.notInstalled")}</b> {installed ? null : t("chat.setup.installSteps")}
        </p>
        <div className="btns">
          <button
            className="btn small primary"
            onClick={() => {
              api
                .openApp(installed ? "ollama" : "ollamaDownload")
                .then(() => {
                  if (installed) window.setTimeout(check, 4000);
                })
                .catch((e: unknown) => {
                  sayError(e, t);
                });
            }}
          >
            {t(installed ? "chat.setup.start" : "chat.setup.download")}
          </button>
          {again}
        </div>
      </div>
    );
  }
  const rec = setup.recommendation;
  const hasRec = setup.models.some((m) => m.name === rec.model);
  const machine =
    setup.ram !== null
      ? setup.vram !== null
        ? t("chat.setup.machine", { ram: fmtBytes(setup.ram, lang), vram: fmtBytes(setup.vram, lang) })
        : t("chat.setup.machineNoVram", { ram: fmtBytes(setup.ram, lang) })
      : "";
  const done = pull?.completed ?? null;
  const total = pull?.total ?? null;
  return (
    <div className="chat-setup">
      <p className="info">
        {machine} {t("chat.setup.advice", { model: rec.model, size: fmtBytes(rec.download, lang) })} {t(`chat.setup.fit.${rec.fit}`)}
      </p>
      {!hasRec ? (
        <>
          <p className="info">
            <b>{t("chat.setup.noModel")}</b> {t("chat.setup.pullNote")}
          </p>
          <div className="btns">
            <button
              className="btn small primary"
              disabled={pulling}
              onClick={() => {
                download(rec.model);
              }}
            >
              {t("chat.setup.pull", { model: rec.model })}
            </button>
            {again}
          </div>
          {pulling ? (
            <div className="photo-progress">
              {done !== null && total ? <progress value={done} max={total} /> : <progress />}
              <span className="note">
                {done !== null && total
                  ? t("chat.setup.pulling", { model: rec.model, done: fmtBytes(done, lang), total: fmtBytes(total, lang) })
                  : t("chat.setup.pullingStep", { model: rec.model, status: pull?.status ?? "…" })}
              </span>
            </div>
          ) : null}
        </>
      ) : (
        <p className="note">{t("chat.setup.ready")}</p>
      )}
      {setup.models.length > 0 ? (
        <Field name="settings.chatModel" hint="settings.hints.chatModel">
          <select
            value={settings.chatModel ?? ""}
            onChange={(e) => {
              updateSettings({ chatModel: e.target.value === "" ? null : e.target.value });
            }}
          >
            <option value="">{t("settings.recommendedModel")}</option>
            {setup.models.map((m) => (
              <option key={m.name} value={m.name}>
                {m.name} · {fmtBytes(m.size, lang)}
                {m.name === rec.model ? ` · ${t("settings.recommended")}` : ""}
              </option>
            ))}
          </select>
        </Field>
      ) : null}
    </div>
  );
}

/** The Settings screen. */
export function Settings(): JSX.Element {
  const { t } = useTranslation();
  const { settings, updateSettings, setLang, info, say, sayError } = useApp(
    useShallow((s) => ({ settings: s.settings, updateSettings: s.updateSettings, setLang: s.setLang, info: s.info, say: s.say, sayError: s.sayError })),
  );
  return (
    <div className="page">
      <header>
        <h1>{t("settings.title")}</h1>
      </header>
      <section className="panel">
        <Field name="settings.language" hint="settings.hints.language">
          <div className="seg" role="radiogroup">
            {(
              [
                [null, "settings.followWindows"],
                ["en", "settings.english"],
                ["nl", "settings.dutch"],
              ] as const
            ).map(([value, label]) => (
              <button
                key={String(value)}
                role="radio"
                aria-checked={settings.language === value}
                aria-pressed={settings.language === value}
                onClick={() => {
                  updateSettings({ language: value });
                  setLanguage(value)
                    .then(setLang)
                    .catch(() => {
                      setLang("en");
                    });
                }}
              >
                {t(label)}
              </button>
            ))}
          </div>
        </Field>
        <Field name="settings.mapStyle" hint="settings.hints.mapStyle">
          <Choice
            field="mapStyle"
            options={[
              ["cells", "top.cells"],
              ["treemap", "top.treemap"],
              ["matrix", "top.matrix"],
            ]}
          />
        </Field>
        <Field name="settings.adminRead" hint="settings.hints.adminRead">
          <Choice
            field="adminRead"
            options={[
              ["ask", "settings.adminAsk"],
              ["always", "settings.adminAlways"],
              ["never", "settings.adminNever"],
            ]}
          />
        </Field>
        <Field name="settings.motion" hint="settings.hints.motion">
          <Choice
            field="motion"
            options={[
              ["living", "settings.living"],
              ["calm", "settings.calm"],
            ]}
          />
        </Field>
        <Field name="settings.measure" hint="settings.hints.measure">
          <Choice
            field="measure"
            options={[
              ["onDisk", "settings.onDisk"],
              ["logical", "settings.logicalSize"],
            ]}
          />
        </Field>
        <Field name="settings.companion" hint="settings.hints.companion">
          <Choice
            field="companion"
            options={[
              ["normal", "settings.normal"],
              ["quiet", "settings.quiet"],
            ]}
          />
        </Field>
      </section>
      <section className="panel">
        <h2>{t("settings.insightsTitle")}</h2>
        <p className="note">{t("settings.insightsText")}</p>
        <Field name="settings.insights" hint="settings.hints.insights">
          <Choice
            field="insights"
            options={[
              [true, "settings.on"],
              [false, "settings.off"],
            ]}
          />
        </Field>
        <button
          className="btn small"
          style={{ alignSelf: "flex-start" }}
          onClick={() => {
            api
              .historyClear()
              .then(() => {
                say({ type: "explanation", text: t("settings.historyCleared") });
              })
              .catch((e: unknown) => {
                sayError(e, t);
              });
          }}
        >
          {t("settings.clearHistory")}
        </button>
        <small className="hint">{t("settings.hints.clearHistory")}</small>
      </section>
      <section className="panel">
        <h2>{t("settings.chatTitle")}</h2>
        <p className="note">{t("settings.chatText")}</p>
        <Field name="settings.chat" hint="settings.hints.chat">
          <Choice
            field="chat"
            options={[
              [true, "settings.on"],
              [false, "settings.off"],
            ]}
          />
        </Field>
        {settings.chat ? <ChatSetupPanel /> : null}
      </section>
      <section className="panel">
        <h2>{t("settings.about")}</h2>
        <p className="info">{t("app.tagline")}</p>
        <p className="note">{t("settings.privacy")}</p>
        <p className="note">{t("settings.license")}</p>
        <p className="note">{t("settings.build", { build: info?.build ?? "" })}</p>
      </section>
      <button
        className="btn small"
        style={{ alignSelf: "flex-start" }}
        onClick={() => {
          updateSettings({ firstRunDone: false });
        }}
      >
        {t("firstRun.title")}
      </button>
      <small className="hint">{t("settings.hints.firstRun")}</small>
    </div>
  );
}

// Settings: language, map style, motion, size measure, companion mode, background
// insights, the AI chat, and About.
import { useEffect, useState, type JSX } from "react";
import { useShallow } from "zustand/react/shallow";
import { useTranslation } from "react-i18next";

import { api } from "../api/ipc";
import type { ChatError, ModelInfo, Settings as S } from "../api/types";
import { fmtBytes } from "../format";
import { setLanguage } from "../i18n";
import { useApp } from "../store";

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

/** Picks the chat model from what Ollama has installed; the recommended one is marked. */
function ModelPicker(): JSX.Element {
  const { t } = useTranslation();
  const { settings, updateSettings, lang } = useApp(useShallow((s) => ({ settings: s.settings, updateSettings: s.updateSettings, lang: s.lang })));
  const [models, setModels] = useState<ModelInfo[] | null>(null);
  const [problem, setProblem] = useState<ChatError | null>(null);
  useEffect(() => {
    api
      .chatModels()
      .then((r) => {
        if ("Ok" in r) setModels(r.Ok);
        else setProblem(r.Err);
      })
      .catch(() => {
        setProblem("badReply");
      });
  }, []);
  if (problem) return <p className="note warn">{t(`chat.errors.${problem}`, { model: settings.chatModel ?? "qwen3:8b" })}</p>;
  if (!models) return <p className="note">{t("settings.loadingModels")}</p>;
  return (
    <div className="field">
      <span>{t("settings.chatModel")}</span>
      <select
        value={settings.chatModel ?? ""}
        onChange={(e) => {
          updateSettings({ chatModel: e.target.value === "" ? null : e.target.value });
        }}
      >
        <option value="">{t("settings.recommendedModel")}</option>
        {models.map((m) => (
          <option key={m.name} value={m.name}>
            {m.name} · {fmtBytes(m.size, lang)}
            {m.recommended ? ` · ${t("settings.recommended")}` : ""}
          </option>
        ))}
      </select>
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
        <div className="field">
          <span>{t("settings.language")}</span>
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
        </div>
        <div className="field">
          <span>{t("settings.mapStyle")}</span>
          <Choice
            field="mapStyle"
            options={[
              ["cells", "top.cells"],
              ["treemap", "top.treemap"],
              ["matrix", "top.matrix"],
            ]}
          />
        </div>
        <div className="field">
          <span>{t("settings.adminRead")}</span>
          <Choice
            field="adminRead"
            options={[
              ["ask", "settings.adminAsk"],
              ["always", "settings.adminAlways"],
              ["never", "settings.adminNever"],
            ]}
          />
        </div>
        <div className="field">
          <span>{t("settings.motion")}</span>
          <Choice
            field="motion"
            options={[
              ["living", "settings.living"],
              ["calm", "settings.calm"],
            ]}
          />
        </div>
        <div className="field">
          <span>{t("settings.measure")}</span>
          <Choice
            field="measure"
            options={[
              ["onDisk", "settings.onDisk"],
              ["logical", "settings.logicalSize"],
            ]}
          />
        </div>
        <div className="field">
          <span>{t("settings.companion")}</span>
          <Choice
            field="companion"
            options={[
              ["normal", "settings.normal"],
              ["quiet", "settings.quiet"],
            ]}
          />
        </div>
      </section>
      <section className="panel">
        <h2>{t("settings.insightsTitle")}</h2>
        <p className="note">{t("settings.insightsText")}</p>
        <div className="field">
          <span>{t("settings.insights")}</span>
          <Choice
            field="insights"
            options={[
              [true, "settings.on"],
              [false, "settings.off"],
            ]}
          />
        </div>
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
      </section>
      <section className="panel">
        <h2>{t("settings.chatTitle")}</h2>
        <p className="note">{t("settings.chatText")}</p>
        <div className="field">
          <span>{t("settings.chat")}</span>
          <Choice
            field="chat"
            options={[
              [true, "settings.on"],
              [false, "settings.off"],
            ]}
          />
        </div>
        {settings.chat ? <ModelPicker /> : null}
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
    </div>
  );
}

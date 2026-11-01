// Settings: language, map style, motion, size measure, companion mode, and About.
import type { JSX } from "react";
import { useShallow } from "zustand/react/shallow";
import { useTranslation } from "react-i18next";

import type { Settings as S } from "../api/types";
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

/** The Settings screen. */
export function Settings(): JSX.Element {
  const { t } = useTranslation();
  const { settings, updateSettings, setLang, info } = useApp(useShallow((s) => ({ settings: s.settings, updateSettings: s.updateSettings, setLang: s.setLang, info: s.info })));
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

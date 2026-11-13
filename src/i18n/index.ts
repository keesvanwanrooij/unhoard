// i18next set-up: English and Dutch, chosen by the setting or the Windows language.
import i18n from "i18next";
import { initReactI18next } from "react-i18next";

import { detectLanguage, type Lang } from "./detect";
import en from "./en.json";
import nl from "./nl.json";

/** Initialises translations and returns the language in use. */
export async function initI18n(setting: string | null): Promise<Lang> {
  const lang = detectLanguage(setting, navigator.languages);
  await i18n.use(initReactI18next).init({
    resources: { en: { translation: en }, nl: { translation: nl } },
    lng: lang,
    fallbackLng: "en",
    interpolation: { escapeValue: false },
    returnNull: false,
  });
  document.documentElement.lang = lang;
  return lang;
}

/** Switches the language. */
export async function setLanguage(setting: string | null): Promise<Lang> {
  const lang = detectLanguage(setting, navigator.languages);
  await i18n.changeLanguage(lang);
  document.documentElement.lang = lang;
  return lang;
}

export { i18n };

// Picks the interface language: the user's choice, else Dutch for a Dutch Windows, else English.

/** The supported languages. */
export type Lang = "en" | "nl";

/** The language to use, from the setting (`null` = follow Windows) and the Windows languages. */
export function detectLanguage(setting: string | null, windowsLanguages: readonly string[]): Lang {
  if (setting === "en" || setting === "nl") return setting;
  const first = windowsLanguages[0]?.toLowerCase() ?? "";
  return first === "nl" || first.startsWith("nl-") ? "nl" : "en";
}

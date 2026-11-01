// Number formats: binary sizes labelled the way Explorer labels them, in the UI language.
// Intl formatters are costly to create, so each one is made once per language and option
// and reused: lists format hundreds of sizes per render.
import type { Lang } from "./i18n/detect";

const UNITS = ["bytes", "KB", "MB", "GB", "TB"] as const;

const numberFormats = new Map<string, Intl.NumberFormat>();
const relativeFormats = new Map<Lang, Intl.RelativeTimeFormat>();

function locale(lang: Lang): string {
  return lang === "nl" ? "nl-NL" : "en-GB";
}

/** A number formatter for `lang` with at most `digits` decimals, made once. */
function numberFormat(lang: Lang, digits?: number): Intl.NumberFormat {
  const key = `${lang}|${String(digits)}`;
  let f = numberFormats.get(key);
  if (!f) {
    f = new Intl.NumberFormat(locale(lang), digits === undefined ? undefined : { maximumFractionDigits: digits });
    numberFormats.set(key, f);
  }
  return f;
}

/** Formats a size with three significant digits, `null` as "unknown". */
export function fmtBytes(bytes: number | null | undefined, lang: Lang): string {
  if (bytes === null || bytes === undefined) return lang === "nl" ? "onbekend" : "unknown";
  let v = bytes;
  let i = 0;
  while (v >= 1024 && i < UNITS.length - 1) {
    v /= 1024;
    i++;
  }
  const digits = i === 0 || v >= 100 ? 0 : 1;
  return `${numberFormat(lang, digits).format(v)} ${UNITS[i] ?? ""}`;
}

/** Formats a share (0–1) as a percentage, one decimal below 1%. */
export function fmtShare(share: number, lang: Lang): string {
  const p = share * 100;
  return `${numberFormat(lang, p < 1 ? 1 : 0).format(p)}%`;
}

/** Formats a whole number with the locale's grouping. */
export function fmtInt(n: number, lang: Lang): string {
  return numberFormat(lang).format(n);
}

/** Formats a past moment (seconds since 1970) relative to `now`, as "3 months ago". */
export function fmtAgo(at: number, now: number, lang: Lang): string {
  let rtf = relativeFormats.get(lang);
  if (!rtf) {
    rtf = new Intl.RelativeTimeFormat(locale(lang), { numeric: "auto" });
    relativeFormats.set(lang, rtf);
  }
  const days = Math.round((now - at) / 86_400);
  if (days < 1) return rtf.format(0, "day");
  if (days < 45) return rtf.format(-days, "day");
  if (days < 548) return rtf.format(-Math.round(days / 30.4), "month");
  return rtf.format(-Math.round(days / 365.25), "year");
}

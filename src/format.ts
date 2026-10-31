// Number formats: binary sizes labelled the way Explorer labels them, in the UI language.
import type { Lang } from "./i18n/detect";

const UNITS = ["bytes", "KB", "MB", "GB", "TB"] as const;

/** Formats a size with three significant digits, `null` as "unknown". */
export function fmtBytes(bytes: number | null | undefined, lang: Lang): string {
  if (bytes === null || bytes === undefined) return lang === "nl" ? "onbekend" : "unknown";
  let v = bytes;
  let i = 0;
  while (v >= 1024 && i < UNITS.length - 1) {
    v /= 1024;
    i++;
  }
  const digits = i === 0 || v >= 100 ? 0 : v >= 10 ? 1 : 1;
  const n = new Intl.NumberFormat(lang === "nl" ? "nl-NL" : "en-GB", { maximumFractionDigits: digits }).format(v);
  return `${n} ${UNITS[i] ?? ""}`;
}

/** Formats a share (0–1) as a percentage, one decimal below 1%. */
export function fmtShare(share: number, lang: Lang): string {
  const p = share * 100;
  const n = new Intl.NumberFormat(lang === "nl" ? "nl-NL" : "en-GB", { maximumFractionDigits: p < 1 ? 1 : 0 }).format(p);
  return `${n}%`;
}

/** Formats a whole number with the locale's grouping. */
export function fmtInt(n: number, lang: Lang): string {
  return new Intl.NumberFormat(lang === "nl" ? "nl-NL" : "en-GB").format(n);
}

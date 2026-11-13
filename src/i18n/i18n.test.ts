// F9 languages: same keys in English and Dutch, language selection and number formats
// (T-9.1, T-9.2, AC9.3).
import { describe, expect, it } from "vitest";

import en from "./en.json";
import nl from "./nl.json";
import { detectLanguage } from "./detect";
import { fmtBytes, fmtShare } from "../format";

function keys(o: unknown, prefix = ""): string[] {
  if (Array.isArray(o)) return [`${prefix}[${o.length}]`];
  if (o !== null && typeof o === "object") {
    return Object.entries(o as Record<string, unknown>).flatMap(([k, v]) => keys(v, prefix ? `${prefix}.${k}` : k));
  }
  return [prefix];
}

describe("translations", () => {
  it("f9_english_and_dutch_have_the_same_keys", () => {
    const a = keys(en).sort();
    const b = keys(nl).sort();
    expect(a.filter((k) => !b.includes(k))).toEqual([]);
    expect(b.filter((k) => !a.includes(k))).toEqual([]);
    expect(a.length).toBeGreaterThan(300);
  });

  it("f9_the_language_follows_windows_with_an_english_fallback", () => {
    expect(detectLanguage(null, ["nl-NL", "en-US"])).toBe("nl");
    expect(detectLanguage(null, ["nl-BE"])).toBe("nl");
    expect(detectLanguage(null, ["de-DE", "nl-NL"])).toBe("en");
    expect(detectLanguage(null, [])).toBe("en");
    expect(detectLanguage("en", ["nl-NL"])).toBe("en");
    expect(detectLanguage("nl", ["en-GB"])).toBe("nl");
  });

  it("f9_sizes_use_binary_units_labelled_like_explorer_in_the_locale_format", () => {
    expect(fmtBytes(1.5 * 1024 ** 3, "nl")).toBe("1,5 GB");
    expect(fmtBytes(1.5 * 1024 ** 3, "en")).toBe("1.5 GB");
    expect(fmtBytes(214 * 1024 ** 3, "en")).toBe("214 GB");
    expect(fmtBytes(512, "en")).toBe("512 bytes");
    expect(fmtBytes(5 * 1024 * 1024, "nl")).toBe("5 MB");
    expect(fmtBytes(null, "en")).toBe("unknown");
    expect(fmtBytes(null, "nl")).toBe("onbekend");
    expect(fmtShare(0.004, "nl")).toBe("0,4%");
    expect(fmtShare(0.25, "en")).toBe("25%");
  });
});

// Insights phase in the UI: chat links only select or open a group (AC8.10), growth colours
// follow the change (AC17.6), signed sizes, and every new text exists in both languages.
import { describe, expect, it } from "vitest";

import en from "../i18n/en.json";
import nl from "../i18n/nl.json";
import { fmtDelta } from "../screens/History";
import { parseAnswer } from "./ChatPanel";
import { growthClass } from "./Stage";

const GB = 1024 ** 3;
const MB = 1024 ** 2;

describe("insights", () => {
  it("f8_chat_links_select_items_or_open_groups_and_nothing_else", () => {
    const parts = parseAnswer("See [Steam](unhoard:item/42), [caches](unhoard:group/rule:chrome-cache) and [x](https://evil.example).");
    expect(parts).toEqual([
      { text: "See " },
      { text: "Steam", item: 42 },
      { text: ", " },
      { text: "caches", group: "rule:chrome-cache" },
      { text: " and [x](https://evil.example)." },
    ]);
    expect(parseAnswer("[a](unhoard:item/notanumber)")).toEqual([{ text: "a" }]);
  });

  it("f17_growth_colours_follow_the_change", () => {
    expect(growthClass(undefined)).toBe("nodata");
    expect(growthClass(2 * GB)).toBe("growHigh");
    expect(growthClass(300 * MB)).toBe("grow");
    expect(growthClass(50 * MB)).toBe("same");
    expect(growthClass(-200 * MB)).toBe("shrink");
  });

  it("f17_changes_carry_their_sign", () => {
    expect(fmtDelta(54 * GB, "en")).toBe("+54 GB");
    expect(fmtDelta(-31 * GB, "nl")).toBe("−31 GB");
  });

  it("f16_every_finding_kind_has_title_and_text_in_both_languages", () => {
    for (const doc of [en, nl]) {
      for (const kind of ["memory", "process", "disk", "startup", "freeSpace", "updates", "systemFiles", "pageFile", "power"]) {
        const f = (doc.optimise.findings as Record<string, { title: string; text: string }>)[kind];
        expect(f?.title, kind).toBeTruthy();
        expect(f?.text, kind).toBeTruthy();
      }
    }
  });
});

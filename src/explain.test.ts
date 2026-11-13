// F4 explanations in the UI: every item gets a description of its own, built from its
// rule or from its place, type and contents, in English and in Dutch.
import i18next, { type i18n as I18n } from "i18next";
import { beforeAll, describe, expect, it } from "vitest";

import type { Explain, FileType, Place } from "./api/types";
import { FILE_TYPES, PLACES, explainShort, explainText, type Explained } from "./explain";
import en from "./i18n/en.json";
import nl from "./i18n/nl.json";

const langs: Record<string, I18n> = {};

beforeAll(async () => {
  for (const [lng, doc] of [
    ["en", en],
    ["nl", nl],
  ] as const) {
    const inst = i18next.createInstance();
    await inst.init({ lng, resources: { [lng]: { translation: doc } }, interpolation: { escapeValue: false }, returnNull: false });
    langs[lng] = inst;
  }
});

const facts = (over: Partial<Explain> = {}): Explain => ({ place: "drive", owner: null, ownerRoot: false, fileType: null, mix: null, ...over });
const item = (over: Partial<Explained> = {}): Explained => ({ name: "Thing", kind: "folder", files: 12, ruleId: null, explain: facts(), ...over });

/** Text that is real: not empty, no unresolved key and no placeholder left behind. */
function real(text: string): void {
  expect(text.trim().length).toBeGreaterThan(10);
  expect(text).not.toMatch(/explain\.|knownLocations\.|\{\{|undefined|null/);
}

describe("explanations", () => {
  it("f4_every_place_and_file_type_has_text_in_both_languages", () => {
    for (const lng of ["en", "nl"]) {
      const t = langs[lng]?.t;
      if (!t) throw new Error("no translations");
      for (const place of PLACES) {
        for (const kind of ["file", "folder"] as const) {
          for (const ownerRoot of [false, true]) {
            const it0 = item({ kind, name: "Sample", explain: facts({ place, owner: "Owner", ownerRoot }) });
            real(explainText(it0, t, "en"));
            real(explainShort(it0, t));
          }
        }
      }
      for (const fileType of FILE_TYPES) {
        const it0 = item({ kind: "file", name: "sample.bin", files: 1, explain: facts({ fileType }) });
        real(explainText(it0, t, "en"));
        real(explainShort(it0, t));
      }
    }
  });

  it("f4_two_items_never_share_a_generated_description", () => {
    const t = langs["nl"]?.t;
    if (!t) throw new Error("no translations");
    const a = explainText(item({ name: "Vakantie 2019", files: 214, explain: facts({ place: "pictures", mix: { category: "media", percent: 98 } }) }), t, "nl");
    const b = explainText(item({ name: "Belasting", files: 31, explain: facts({ place: "documents", mix: { category: "documents", percent: 80 } }) }), t, "nl");
    expect(a).toContain("Vakantie 2019");
    expect(a).toContain("214");
    expect(a).toContain("98%");
    expect(b).toContain("Belasting");
    expect(a).not.toBe(b);
  });

  it("f4_a_file_is_explained_by_its_type_and_an_unknown_type_by_its_extension", () => {
    const t = langs["en"]?.t;
    if (!t) throw new Error("no translations");
    const iso = explainText(item({ name: "linux.iso", kind: "file", files: 1, explain: facts({ place: "downloads", fileType: "diskImage" }) }), t, "en");
    expect(iso).toContain("linux.iso");
    expect(iso.toLowerCase()).toContain("disk image");
    expect(iso).toContain("Downloads");
    const odd = explainText(item({ name: "data.qqq", kind: "file", files: 1 }), t, "en");
    expect(odd).toContain(".qqq");
    const bare = explainText(item({ name: "LICENSE", kind: "file", files: 1 }), t, "en");
    real(bare);
  });

  it("f4_an_app_folder_names_the_app_and_its_own_folder_says_so", () => {
    const t = langs["en"]?.t;
    if (!t) throw new Error("no translations");
    const inside = explainText(item({ name: "Cache", explain: facts({ place: "appData", owner: "Vendor" }) }), t, "en");
    expect(inside).toContain("Vendor");
    const root = explainText(item({ name: "Vendor", explain: facts({ place: "appData", owner: "Vendor", ownerRoot: true }) }), t, "en");
    expect(root).toContain("Vendor");
    expect(root).not.toBe(inside);
  });

  it("f4_a_rule_gives_its_written_explanation", () => {
    const t = langs["nl"]?.t;
    if (!t) throw new Error("no translations");
    const it0 = item({ name: "Desktop", ruleId: "def-desktop" });
    expect(explainText(it0, t, "nl")).toBe(nl.knownLocations["def-desktop"].explain);
    expect(explainShort(it0, t)).toBe(nl.knownLocations["def-desktop"].title);
  });

  it("f4_the_lists_cover_every_value_of_the_core", () => {
    const places: Place[] = ["downloads", "desktop", "documents", "pictures", "videos", "music", "cloud", "appData", "userPrograms", "storeApp", "temp", "programFiles", "programData", "windows", "profile", "otherUser", "recycleBin", "quarantine", "drive"];
    const types: FileType[] = ["video", "photo", "rawPhoto", "audio", "archive", "diskImage", "virtualDisk", "installer", "program", "library", "document", "spreadsheet", "presentation", "pdf", "ebook", "text", "code", "data", "database", "log", "backup", "dump", "aiModel", "gameData", "font", "mail", "design", "partial", "temporary", "shortcut", "system"];
    expect([...PLACES].sort()).toEqual([...places].sort());
    expect([...FILE_TYPES].sort()).toEqual([...types].sort());
  });
});

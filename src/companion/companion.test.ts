// F8 companion: every trigger has text in both languages, warnings outrank tips, quiet
// mode shows only warnings (T-8.1, T-8.2, AC8.3, AC8.4).
import { describe, expect, it } from "vitest";

import en from "../i18n/en.json";
import nl from "../i18n/nl.json";
import { BubbleQueue } from "./queue";
import { TRIGGERS } from "./script";

function lookup(doc: unknown, path: string): unknown {
  return path.split(".").reduce<unknown>((o, k) => (o !== null && typeof o === "object" ? (o as Record<string, unknown>)[k] : undefined), doc);
}

describe("companion", () => {
  it("f8_every_trigger_has_english_and_dutch_text", () => {
    expect(TRIGGERS.length).toBeGreaterThan(15);
    for (const trigger of TRIGGERS) {
      for (const doc of [en, nl]) {
        const lines = lookup(doc, `companion.script.${trigger}`);
        expect(Array.isArray(lines), trigger).toBe(true);
        expect((lines as unknown[]).length, trigger).toBeGreaterThan(0);
        for (const l of lines as unknown[]) expect(typeof l === "string" && l.trim().length > 0, trigger).toBe(true);
      }
    }
  });

  it("f8_warnings_outrank_tips_in_the_bubble_queue", () => {
    const q = new BubbleQueue();
    q.push({ type: "tip", text: "tip one" });
    q.push({ type: "warning", text: "purge soon" });
    expect(q.current?.text).toBe("purge soon");
    q.push({ type: "tip", text: "tip two" });
    expect(q.current?.text).toBe("purge soon");
    q.dismiss();
    expect(q.current?.type).toBe("tip");
    expect(q.history.map((b) => b.text)).toContain("purge soon");
  });

  it("f8_one_bubble_at_a_time_and_progress_updates_in_place", () => {
    const q = new BubbleQueue();
    q.push({ type: "progress", text: "1 file", key: "scan" });
    q.push({ type: "progress", text: "2 files", key: "scan" });
    expect(q.current?.text).toBe("2 files");
    expect(q.waiting).toHaveLength(0);
  });

  it("f8_quiet_mode_shows_only_warnings", () => {
    const q = new BubbleQueue();
    q.quiet = true;
    q.push({ type: "tip", text: "a tip" });
    expect(q.current).toBeNull();
    q.push({ type: "warning", text: "a warning" });
    expect(q.current?.text).toBe("a warning");
  });
});

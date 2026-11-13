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

  it("f8_a_newer_message_replaces_an_older_one_instead_of_stacking", () => {
    const q = new BubbleQueue();
    q.push({ type: "explanation", text: "first" });
    q.push({ type: "explanation", text: "second" });
    expect(q.current?.text).toBe("second");
    expect(q.waiting).toHaveLength(0);
    q.dismiss();
    expect(q.current).toBeNull();
  });

  it("f8_a_dismissed_topic_stays_quiet_until_it_ends", () => {
    const q = new BubbleQueue();
    q.push({ type: "progress", text: "1 file", key: "scan" });
    q.dismiss();
    q.push({ type: "progress", text: "2 files", key: "scan" });
    expect(q.current).toBeNull();
    q.end("scan");
    q.push({ type: "progress", text: "next scan", key: "scan" });
    expect(q.current?.text).toBe("next scan");
  });

  it("f8_an_expired_bubble_does_not_silence_its_topic", () => {
    const q = new BubbleQueue();
    q.push({ type: "progress", text: "1 file", key: "scan" });
    q.expire();
    q.push({ type: "progress", text: "2 files", key: "scan" });
    expect(q.current?.text).toBe("2 files");
  });

  it("f8_progress_never_interrupts_a_message_and_waits_with_its_latest_text", () => {
    const q = new BubbleQueue();
    q.push({ type: "explanation", text: "about this folder" });
    q.push({ type: "progress", text: "1 file", key: "scan" });
    q.push({ type: "progress", text: "3 files", key: "scan" });
    expect(q.current?.text).toBe("about this folder");
    expect(q.waiting).toHaveLength(1);
    q.dismiss();
    expect(q.current?.text).toBe("3 files");
  });

  it("f8_ending_a_topic_removes_its_bubble_and_progress_stays_out_of_the_history", () => {
    const q = new BubbleQueue();
    q.push({ type: "progress", text: "1 file", key: "scan" });
    q.end("scan");
    expect(q.current).toBeNull();
    expect(q.history).toHaveLength(0);
  });

  it("f8_warnings_are_never_dropped_and_repeats_are_ignored", () => {
    const q = new BubbleQueue();
    q.push({ type: "warning", text: "purge in 3 days" });
    q.push({ type: "warning", text: "disk full" });
    q.push({ type: "warning", text: "purge in 3 days" });
    expect(q.current?.text).toBe("purge in 3 days");
    expect(q.waiting.map((b) => b.text)).toEqual(["disk full"]);
    q.dismiss();
    expect(q.current?.text).toBe("disk full");
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

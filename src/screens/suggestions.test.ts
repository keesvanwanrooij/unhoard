// F4 grouped suggestions (AC4.13): one group per rule or app kind, largest first.
import { describe, expect, it } from "vitest";

import type { Description, Suggestion } from "../api/types";
import { groupKey, groupSuggestions } from "./Suggestions";

function item(id: number, onDisk: number, d: Partial<Description>): Suggestion {
  return {
    node: { id, name: `n${String(id)}`, kind: "folder", logical: onDisk, onDisk, files: 1, category: "other", tier: "safe", ruleId: d.ruleId ?? null, hasChildren: false, unscanned: false, cloud: null },
    path: `C:\\x\\n${String(id)}`,
    tier: "safe",
    description: {
      ruleId: null,
      tier: "safe",
      actions: [],
      generic: false,
      protected: null,
      cloud: false,
      quarantineRoot: null,
      requiresAdmin: false,
      effort: 1,
      program: null,
      steam: null,
      elevated: false,
      lastUse: null,
      ...d,
    },
  };
}

describe("grouped suggestions", () => {
  it("f4_suggestions_group_by_rule_and_app_kind", () => {
    expect(groupKey(item(1, 1, { ruleId: "chrome-cache" }))).toBe("rule:chrome-cache");
    expect(groupKey(item(2, 1, { steam: { kind: "leftover" } }))).toBe("steam:leftover");
    expect(groupKey(item(3, 1, { steam: { kind: "game", appId: 1, name: "G", lastPlayed: null } }))).toBe("steam:game");
    expect(groupKey(item(4, 1, { program: { name: "P", publisher: null, installLocation: "C:\\P", installDate: null, estimatedBytes: null } }))).toBe("program");
    expect(groupKey(item(5, 1, {}))).toBe("generic");
  });

  it("f4_groups_sort_by_total_and_items_by_size", () => {
    const groups = groupSuggestions([
      item(1, 10, { ruleId: "a" }),
      item(2, 30, { ruleId: "b" }),
      item(3, 25, { ruleId: "a" }),
    ]);
    expect(groups.map((g) => g.key)).toEqual(["rule:a", "rule:b"]);
    expect(groups[0]?.total).toBe(35);
    expect(groups[0]?.items.map((s) => s.node.id)).toEqual([3, 1]);
  });
});

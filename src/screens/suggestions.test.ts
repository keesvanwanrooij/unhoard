// F4 grouped suggestions (AC4.13): one group per rule or app kind, largest first.
import { describe, expect, it } from "vitest";

import type { Description, Suggestion } from "../api/types";
import { groupKey, groupSuggestions, keepOffered, selectionState, toggleAll } from "./Suggestions";

function item(id: number, onDisk: number, d: Partial<Description>): Suggestion {
  return {
    node: { id, name: `n${String(id)}`, kind: "folder", logical: onDisk, onDisk, files: 1, category: "other", tier: "safe", ruleId: d.ruleId ?? null, explain: { place: "drive", owner: null, ownerRoot: false, fileType: null, mix: null }, hasChildren: false, unscanned: false, cloud: null },
    path: `C:\\x\\n${String(id)}`,
    tier: "safe",
    description: {
      ruleId: null,
      tier: "safe",
      actions: [],
      large: false,
      definition: false,
      explain: { place: "drive", owner: null, ownerRoot: false, fileType: null, mix: null },
      protected: null,
      cloud: false,
      quarantineRoot: null,
      requiresAdmin: false,
      program: null,
      steam: null,
      elevated: false,
      lastUse: null,
      leftover: null,
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
    expect(groupKey(item(5, 1, {}))).toBe("large");
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

describe("selection (multi-select)", () => {
  it("f4_s1_nothing_is_selected_at_the_start_and_a_group_toggles_all_or_none", () => {
    let sel = new Set<number>();
    expect(selectionState(sel, [1, 2, 3])).toBe("none");
    sel = toggleAll(sel, [1, 2, 3]);
    expect([...sel].sort()).toEqual([1, 2, 3]);
    expect(selectionState(sel, [1, 2, 3])).toBe("all");
    sel = toggleAll(sel, [1, 2, 3]);
    expect(sel.size).toBe(0);
    // A partly selected group becomes fully selected first.
    sel = toggleAll(new Set([2, 9]), [1, 2, 3]);
    expect([...sel].sort()).toEqual([1, 2, 3, 9]);
    expect(selectionState(new Set([2]), [1, 2, 3])).toBe("some");
  });

  it("f4_a_selection_keeps_only_items_that_are_still_offered", () => {
    expect([...keepOffered(new Set([1, 2, 3]), [2, 3, 4])].sort()).toEqual([2, 3]);
  });
});

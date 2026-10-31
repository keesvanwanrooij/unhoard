// F7 review grid: nothing selected at the start, "select all but the keeper" is one
// action, and the keyboard drives it (T-7.7, AC7.5, S1).
import { act, cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, beforeAll, describe, expect, it, vi } from "vitest";

import type { Group, MediaItem } from "../api/types";
import { initI18n } from "../i18n";
import { PhotoGrid } from "./PhotoGrid";

function media(i: number): MediaItem {
  return {
    node: 100 + i,
    path: `C:\\p\\IMG_${String(i)}.jpg`,
    name: `IMG_${String(i)}.jpg`,
    ext: "jpg",
    size: 1000 + i,
    modified: 0,
    cloudOnly: false,
    inSyncRoot: false,
    parsed: { source: "iPhone", taken: null, precise: false, video: false },
  };
}

const all = [0, 1, 2].map(media);
const group: Group = { kind: "burst", certainty: "sameMoment", items: [0, 1, 2], label: "", keeper: 1, keeperReasons: ["sharpest"] };

beforeAll(async () => {
  await initI18n("en");
});
afterEach(cleanup);

function setup() {
  const onMove = vi.fn();
  const onNext = vi.fn();
  render(<PhotoGrid group={group} media={all} thumb={() => Promise.reject(new Error("none"))} onMove={onMove} onNext={onNext} />);
  return { onMove, onNext };
}

describe("photo grid", () => {
  it("f7_s1_the_review_grid_starts_with_nothing_selected", () => {
    setup();
    const tiles = screen.getAllByRole("checkbox");
    expect(tiles).toHaveLength(3);
    for (const tile of tiles) expect(tile.getAttribute("aria-checked")).toBe("false");
  });

  it("f7_select_all_but_the_keeper_is_one_action", () => {
    const { onMove } = setup();
    fireEvent.click(screen.getByText("Select all but the keeper"));
    const checked = screen.getAllByRole("checkbox").map((t) => t.getAttribute("aria-checked"));
    expect(checked).toEqual(["true", "false", "true"]);
    fireEvent.click(screen.getByText("Move selected to quarantine"));
    expect(onMove).toHaveBeenCalledWith([100, 102]);
  });

  it("f7_keyboard_space_toggles_enter_moves_n_skips", () => {
    const { onMove, onNext } = setup();
    const grid = screen.getByRole("group");
    act(() => {
      fireEvent.keyDown(grid, { key: "ArrowRight" });
      fireEvent.keyDown(grid, { key: " " });
    });
    expect(screen.getAllByRole("checkbox")[1]?.getAttribute("aria-checked")).toBe("true");
    act(() => {
      fireEvent.keyDown(grid, { key: "Enter" });
    });
    expect(onMove).toHaveBeenCalledWith([101]);
    act(() => {
      fireEvent.keyDown(grid, { key: "n" });
    });
    expect(onNext).toHaveBeenCalled();
  });
});

// F2 keyboard tests: arrow keys move to the nearest cell in that direction (AC2.8).
import { describe, expect, it } from "vitest";

import { nearestInDirection, type Spot } from "./navigate";

/** A 3×3 grid of cells, 100 px apart, named by row and column ("r0c0" is top left). */
const grid: Spot[] = [0, 1, 2].flatMap((r) => [0, 1, 2].map((c) => ({ id: `r${String(r)}c${String(c)}`, at: [100 * c, 100 * r] as [number, number] })));

describe("map keyboard navigation", () => {
  it("f2_arrows_move_to_the_neighbour_in_that_direction", () => {
    expect(nearestInDirection(grid, "r1c1", "right")).toBe("r1c2");
    expect(nearestInDirection(grid, "r1c1", "left")).toBe("r1c0");
    expect(nearestInDirection(grid, "r1c1", "up")).toBe("r0c1");
    expect(nearestInDirection(grid, "r1c1", "down")).toBe("r2c1");
  });

  it("f2_arrow_at_the_edge_stays_put", () => {
    expect(nearestInDirection(grid, "r1c2", "right")).toBeNull();
    expect(nearestInDirection(grid, "r0c0", "up")).toBeNull();
  });

  it("f2_arrow_prefers_an_aligned_cell_over_a_closer_diagonal_one", () => {
    const spots: Spot[] = [
      { id: "from", at: [0, 0] },
      { id: "diagonal", at: [10, 8] },
      { id: "aligned", at: [14, 0] },
    ];
    expect(nearestInDirection(spots, "from", "right")).toBe("aligned");
  });

  it("f2_arrow_without_a_selection_starts_at_the_first_cell", () => {
    expect(nearestInDirection(grid, null, "down")).toBe("r0c0");
    expect(nearestInDirection(grid, "gone", "left")).toBe("r0c0");
    expect(nearestInDirection([], null, "left")).toBeNull();
  });
});

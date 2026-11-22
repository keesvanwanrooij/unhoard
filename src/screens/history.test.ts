// F17 storage history: the timeline's axis ticks are round and cover the range.
import { describe, expect, it } from "vitest";

import { niceTicks } from "./History";

describe("niceTicks", () => {
  it("f17_ticks_are_round_steps_inside_the_range", () => {
    expect(niceTicks(812, 851, 4)).toEqual([820, 830, 840, 850]);
    expect(niceTicks(0, 1, 4)).toEqual([0, 0.5, 1]);
    expect(niceTicks(5, 5, 4)).toEqual([5]);
  });
});

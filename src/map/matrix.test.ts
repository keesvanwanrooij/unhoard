// F2 priority matrix scales (AC2.10): log scales for size and age, four effort bands.
import { describe, expect, it } from "vitest";

import { MIN_BYTES, ageRadius, effortY, sizeX } from "./matrix";

const DAY = 86_400;

describe("priority matrix scales", () => {
  it("f2_size_uses_a_log_scale_from_one_mib_to_the_largest_item", () => {
    const max = 100 * 2 ** 30;
    expect(sizeX(MIN_BYTES, max)).toBe(0);
    expect(sizeX(max, max)).toBe(1);
    expect(sizeX(10, max)).toBe(0);
    // A log scale puts 10x steps equally far apart.
    const a = sizeX(10 * 2 ** 20, max) - sizeX(2 ** 20, max);
    const b = sizeX(100 * 2 ** 20, max) - sizeX(10 * 2 ** 20, max);
    expect(Math.abs(a - b)).toBeLessThan(1e-9);
    expect(sizeX(2 ** 20, 2 ** 20)).toBe(0);
  });

  it("f2_age_maps_to_a_bubble_radius_on_a_log_scale", () => {
    const now = 1_790_000_000;
    const small = ageRadius(now, now);
    const big = ageRadius(now - 3650 * DAY, now);
    expect(small).not.toBeNull();
    expect(big).not.toBeNull();
    expect(big ?? 0).toBeGreaterThan(small ?? 0);
    expect(ageRadius(now - 20_000 * DAY, now)).toBe(big);
    expect(ageRadius(null, now)).toBeNull();
    const month = ageRadius(now - 30 * DAY, now) ?? 0;
    const year = ageRadius(now - 365 * DAY, now) ?? 0;
    expect(month).toBeGreaterThan(small ?? 0);
    expect(year).toBeGreaterThan(month);
  });

  it("f2_effort_bands_never_overlap_and_one_is_at_the_bottom", () => {
    for (let e = 1; e <= 4; e++) {
      const lo = effortY(e, 0);
      const hi = effortY(e, 1);
      expect(lo).toBeGreaterThanOrEqual((e - 1) / 4);
      expect(hi).toBeLessThanOrEqual(e / 4);
    }
    expect(effortY(1, 0.5)).toBeLessThan(effortY(4, 0.5));
  });
});

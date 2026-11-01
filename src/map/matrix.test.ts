// F2 priority matrix scales (AC2.10): log scales for days unused (x) and size (y and bubble),
// and the zoom that keeps the point under the pointer in place.
import { describe, expect, it } from "vitest";

import { MIN_BYTES, ageScale, sizeRadius, sizeScale, zoomAt } from "./matrix";

const DAY = 86_400;
const NOW = 1_790_000_000;

describe("priority matrix scales", () => {
  it("f2_size_uses_a_log_scale_from_one_mib_to_the_largest_item", () => {
    const max = 100 * 2 ** 30;
    expect(sizeScale(MIN_BYTES, max)).toBe(0);
    expect(sizeScale(max, max)).toBe(1);
    expect(sizeScale(10, max)).toBe(0);
    const a = sizeScale(10 * 2 ** 20, max) - sizeScale(2 ** 20, max);
    const b = sizeScale(100 * 2 ** 20, max) - sizeScale(10 * 2 ** 20, max);
    expect(Math.abs(a - b)).toBeLessThan(1e-9);
    expect(sizeScale(2 ** 20, 2 ** 20)).toBe(0);
  });

  it("f2_bubble_radius_grows_with_size_on_the_same_log_scale", () => {
    const max = 100 * 2 ** 30;
    const small = sizeRadius(MIN_BYTES, max);
    const mid = sizeRadius(2 ** 30, max);
    const big = sizeRadius(max, max);
    expect(mid).toBeGreaterThan(small);
    expect(big).toBeGreaterThan(mid);
    expect(sizeRadius(10 * max, max)).toBe(big);
  });

  it("f2_days_unused_use_a_log_scale_from_a_day_to_ten_years", () => {
    expect(ageScale(NOW, NOW)).toBe(0);
    expect(ageScale(NOW - 3650 * DAY, NOW)).toBe(1);
    expect(ageScale(NOW - 20_000 * DAY, NOW)).toBe(1);
    expect(ageScale(null, NOW)).toBeNull();
    const week = (ageScale(NOW - 7 * DAY, NOW) ?? 0) - (ageScale(NOW - 1 * DAY, NOW) ?? 0);
    const sevenWeeks = (ageScale(NOW - 49 * DAY, NOW) ?? 0) - (ageScale(NOW - 7 * DAY, NOW) ?? 0);
    expect(Math.abs(week - sevenWeeks)).toBeLessThan(1e-9);
  });

  it("f2_zoom_keeps_the_point_under_the_pointer_in_place", () => {
    const view = { k: 1, tx: 0, ty: 0 };
    const z = zoomAt(view, 2, 300, 200);
    // The data point that was at (300, 200) on screen is still there.
    expect((300 - z.tx) / z.k).toBeCloseTo(300);
    expect((200 - z.ty) / z.k).toBeCloseTo(200);
    expect(zoomAt(view, 0.1, 0, 0).k).toBe(1);
    expect(zoomAt({ k: 30, tx: 0, ty: 0 }, 2, 0, 0).k).toBe(40);
  });
});

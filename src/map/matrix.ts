// Scales of the priority matrix (AC2.10): size on disk on a log scale for x, effort in four
// bands for y, and days since last use on a log scale for the bubble radius. Log scales
// keep one huge item from squashing the rest and stay honest about ratios.

/** The smallest size on the x axis: suggestions start at 1 MiB. */
export const MIN_BYTES = 2 ** 20;
/** The longest age that still grows a bubble: ten years. */
const MAX_DAYS = 3650;
/** Bubble radius in pixels for a fresh and for a ten-year-old item. */
const R_MIN = 5;
const R_MAX = 26;
/** How much of a band's height the jitter may use, so bands keep a gap between them. */
const BAND_FILL = 0.7;

/** x position from 0 (1 MiB) to 1 (the largest item). */
export function sizeX(bytes: number, maxBytes: number): number {
  const lo = Math.log10(MIN_BYTES);
  const hi = Math.log10(Math.max(maxBytes, MIN_BYTES));
  if (hi <= lo) return 0;
  const v = (Math.log10(Math.max(bytes, MIN_BYTES)) - lo) / (hi - lo);
  return Math.min(1, Math.max(0, v));
}

/** Bubble radius for an item last used at `at` (seconds), or `null` when unknown. */
export function ageRadius(at: number | null, now: number): number | null {
  if (at === null) return null;
  const days = Math.min(MAX_DAYS, Math.max(0, (now - at) / 86_400));
  return R_MIN + ((R_MAX - R_MIN) * Math.log10(1 + days)) / Math.log10(1 + MAX_DAYS);
}

/** y position from 0 (bottom) to 1 (top) for an effort of 1 to 4; `jitter` in 0..1 spreads
 * items within their band so they overlap less. */
export function effortY(effort: number, jitter: number): number {
  const band = Math.min(4, Math.max(1, Math.round(effort))) - 1;
  const j = Math.min(1, Math.max(0, jitter));
  return (band + 0.5 + (j - 0.5) * BAND_FILL) / 4;
}

/** A stable value in 0..1 from an ID, for jitter that does not move between renders. */
export function jitterOf(id: number): number {
  const x = Math.sin(id * 12.9898) * 43758.5453;
  return x - Math.floor(x);
}

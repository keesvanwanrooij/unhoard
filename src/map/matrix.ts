// Scales of the priority matrix (AC2.10): days since last use on a log scale for x, size on
// disk on a log scale for y and for the bubble radius, and the zoom. Log scales keep one
// huge or very old item from squashing the rest and stay honest about ratios.

/** The smallest size on the y axis: suggestions start at 1 MiB. */
export const MIN_BYTES = 2 ** 20;
/** The oldest age on the x axis: ten years; anything older sits at the right edge. */
const MAX_DAYS = 3650;
/** Bubble radius in pixels for the smallest and the largest item. */
const R_MIN = 4;
const R_MAX = 24;
/** Zoom limits: from the whole matrix to forty times closer. */
const K_MIN = 1;
const K_MAX = 40;

/** Size from 0 (1 MiB) to 1 (the largest item). */
export function sizeScale(bytes: number, maxBytes: number): number {
  const lo = Math.log10(MIN_BYTES);
  const hi = Math.log10(Math.max(maxBytes, MIN_BYTES));
  if (hi <= lo) return 0;
  const v = (Math.log10(Math.max(bytes, MIN_BYTES)) - lo) / (hi - lo);
  return Math.min(1, Math.max(0, v));
}

/** Bubble radius on the same log scale as the size axis, so a bigger item is a bigger bubble. */
export function sizeRadius(bytes: number, maxBytes: number): number {
  return R_MIN + (R_MAX - R_MIN) * sizeScale(bytes, maxBytes);
}

/** Age from 0 (used today) to 1 (ten years or more), or `null` when unknown. */
export function ageScale(at: number | null, now: number): number | null {
  if (at === null) return null;
  const days = Math.min(MAX_DAYS, Math.max(1, (now - at) / 86_400));
  return Math.log10(days) / Math.log10(MAX_DAYS);
}

/** The day counts the age axis labels. */
export const AGE_TICKS = [1, 7, 30, 365, 3650];

/** The position of an age tick of `days` on the age scale. */
export function ageTick(days: number): number {
  return Math.log10(days) / Math.log10(MAX_DAYS);
}

/** A zoom: scale `k` and offset (`tx`, `ty`) in pixels within the plot. */
export interface Zoom {
  k: number;
  tx: number;
  ty: number;
}

/** Zooms by `factor` around the plot point (`x`, `y`), which stays where it is. */
export function zoomAt(z: Zoom, factor: number, x: number, y: number): Zoom {
  const k = Math.min(K_MAX, Math.max(K_MIN, z.k * factor));
  const r = k / z.k;
  return { k, tx: x - (x - z.tx) * r, ty: y - (y - z.ty) * r };
}

/** A stable value in 0..1 from an ID, for spreading items whose age is unknown. */
export function jitterOf(id: number): number {
  const x = Math.sin(id * 12.9898) * 43758.5453;
  return x - Math.floor(x);
}

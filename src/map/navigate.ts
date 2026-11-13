// Keyboard navigation of the cell map: from the selected cell, find the nearest cell in the
// direction of an arrow key (AC2.8). Cells have irregular shapes, so the search works on
// their centres.

/** A cell centre on the stage, in CSS pixels. */
export interface Spot {
  id: string;
  at: [number, number];
}

/** The four arrow-key directions. */
export type Direction = "left" | "right" | "up" | "down";

const STEP: Record<Direction, [number, number]> = {
  left: [-1, 0],
  right: [1, 0],
  up: [0, -1],
  down: [0, 1],
};

/** Weight of the sideways offset against the distance along the arrow. Above 1, a cell in
 * line with the selection beats a slightly closer one off to the side. */
const ACROSS_WEIGHT = 2;

/**
 * Returns the id of the cell to move to, or `null` when no cell lies in that direction.
 * Without a (known) selection, the first spot is the start, which is the largest cell.
 */
export function nearestInDirection(spots: readonly Spot[], fromId: string | null, dir: Direction): string | null {
  const from = spots.find((s) => s.id === fromId);
  if (!from) return spots[0]?.id ?? null;
  const [dx, dy] = STEP[dir];
  let best: string | null = null;
  let bestScore = Infinity;
  for (const s of spots) {
    if (s.id === from.id) continue;
    const vx = s.at[0] - from.at[0];
    const vy = s.at[1] - from.at[1];
    const along = vx * dx + vy * dy;
    if (along <= 0) continue;
    const score = along + ACROSS_WEIGHT * Math.abs(vx * dy - vy * dx);
    if (score < bestScore) {
      bestScore = score;
      best = s.id;
    }
  }
  return best;
}

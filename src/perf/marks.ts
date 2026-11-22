// User Timing helpers for the performance budgets: marks and measures that never throw,
// read by the performance scenarios (docs/25-performance.md).

/** Sets a mark. */
export function mark(name: string): void {
  performance.mark(name);
}

/** Measures from the latest `start` mark to `end` (default now), if that mark exists, and
 * clears it. */
export function measureSince(name: string, start: string, end?: number): void {
  if (performance.getEntriesByName(start, "mark").length === 0) return;
  performance.measure(name, end === undefined ? start : { start, end });
  performance.clearMarks(start);
}

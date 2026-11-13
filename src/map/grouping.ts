// Groups items under 0.5% of the current view into one "N smaller items" cell (AC2.6).

/** The share of the view below which items are grouped. */
export const SMALL_SHARE = 0.005;

/** An item to show, or the group of smaller items. */
export interface Shown {
  id: string;
  value: number;
  /** For the group: how many items it holds. */
  count?: number;
}

/** Keeps items of at least 0.5% of `total` and groups the rest as `smaller`. */
export function groupSmall<T extends { id: string; value: number }>(items: T[], total: number): (T | Shown)[] {
  const limit = total * SMALL_SHARE;
  const big: (T | Shown)[] = [];
  let small = 0;
  let count = 0;
  for (const i of items) {
    if (i.value < limit) {
      small += i.value;
      count++;
    } else {
      big.push(i);
    }
  }
  if (count > 0) big.push({ id: "smaller", value: small, count });
  return big;
}

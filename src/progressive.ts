// Renders a long screen in portions: the first part at once, one more part per animation
// frame, so no single task lays out a whole screen (P11).
import { useEffect, useState } from "react";

/** How many of `total` parts to show: one at first, then one more every frame. Starts over
 * when `key` changes (new data). */
export function useProgressive(total: number, key: unknown): number {
  const [shown, setShown] = useState({ key, n: 1 });
  const n = shown.key === key ? shown.n : 1;
  useEffect(() => {
    if (n >= total) return;
    const id = requestAnimationFrame(() => {
      setShown({ key, n: n + 1 });
    });
    return () => {
      cancelAnimationFrame(id);
    };
  }, [n, total, key]);
  return Math.min(n, total);
}

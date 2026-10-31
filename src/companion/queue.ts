// The companion's bubble queue: one bubble at a time, warnings first, progress updated in
// place, a history of everything shown, and a quiet mode that shows only warnings.

/** The kind of bubble. */
export type BubbleType = "tip" | "explanation" | "progress" | "warning" | "celebration";

/** A bubble to show. */
export interface BubbleInput {
  type: BubbleType;
  text: string;
  /** Bubbles with the same key replace each other (progress). */
  key?: string;
}

/** A bubble with its sequence number. */
export interface Bubble extends BubbleInput {
  id: number;
}

/** Higher shows first. */
export const PRIORITY: Record<BubbleType, number> = { warning: 4, explanation: 3, celebration: 2, progress: 2, tip: 1 };

/** The queue. */
export class BubbleQueue {
  current: Bubble | null = null;
  waiting: Bubble[] = [];
  history: Bubble[] = [];
  quiet = false;
  private next = 1;

  /** Adds a bubble; it shows now if it outranks the current one. */
  push(input: BubbleInput): void {
    if (this.quiet && input.type !== "warning") return;
    const b: Bubble = { ...input, id: this.next++ };
    if (this.current && input.key !== undefined && this.current.key === input.key) {
      this.current = { ...b, id: this.current.id };
      return;
    }
    this.waiting = this.waiting.filter((w) => input.key === undefined || w.key !== input.key);
    if (!this.current) {
      this.current = b;
    } else if (PRIORITY[b.type] > PRIORITY[this.current.type]) {
      if (this.current.type !== "progress") this.waiting.unshift(this.current);
      this.current = b;
    } else {
      this.waiting.push(b);
      this.waiting.sort((x, y) => PRIORITY[y.type] - PRIORITY[x.type]);
    }
  }

  /** Closes the current bubble and shows the next one. */
  dismiss(): void {
    if (this.current) this.history.push(this.current);
    this.current = this.waiting.shift() ?? null;
  }
}

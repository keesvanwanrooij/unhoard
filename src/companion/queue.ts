// The companion's bubble queue: one bubble at a time, and never a stack of stale ones.
// A newer message replaces an older one of the same or lower rank; a progress update
// changes its bubble in place and never interrupts another message; warnings are never
// dropped. Closing a bubble silences its topic until that operation ends, so a running
// scan does not pop a fresh progress bubble right after the user closed one.

/** The kind of bubble. */
export type BubbleType = "tip" | "explanation" | "progress" | "warning" | "celebration";

/** A bubble to show. */
export interface BubbleInput {
  type: BubbleType;
  text: string;
  /** The topic, such as "scan": bubbles with the same key replace each other. */
  key?: string;
  /** Buttons that answer a question; a click runs one and closes the bubble. */
  actions?: { label: string; run: () => void }[];
}

/** A bubble with its sequence number. */
export interface Bubble extends BubbleInput {
  id: number;
}

/** Higher shows first. */
export const PRIORITY: Record<BubbleType, number> = { warning: 4, explanation: 3, celebration: 2, progress: 2, tip: 1 };

/** Whether `b` may replace `current` right away. */
function showsNow(b: Bubble, current: Bubble | null): boolean {
  if (!current) return true;
  if (b.type === "progress") return current.type === "progress";
  return PRIORITY[b.type] >= PRIORITY[current.type];
}

/** The queue. */
export class BubbleQueue {
  current: Bubble | null = null;
  /** Bubbles waiting for the current one: every warning, and the newest of each other type. */
  waiting: Bubble[] = [];
  /** Bubbles shown earlier, without progress updates. */
  history: Bubble[] = [];
  quiet = false;
  private muted = new Set<string>();
  private next = 1;

  /** Adds a bubble: it shows now, updates its topic in place, or waits. */
  push(input: BubbleInput): void {
    if (this.quiet && input.type !== "warning") return;
    if (input.key !== undefined && this.muted.has(input.key)) return;
    const cur = this.current;
    if (cur && ((input.key !== undefined && cur.key === input.key) || (cur.type === input.type && cur.text === input.text))) {
      this.current = { ...input, id: cur.id };
      return;
    }
    // Older waiting bubbles of the same topic, text or (non-warning) type are stale now.
    this.waiting = this.waiting.filter(
      (w) => !((input.key !== undefined && w.key === input.key) || w.text === input.text || (w.type === input.type && w.type !== "warning")),
    );
    const b: Bubble = { ...input, id: this.next++ };
    if (showsNow(b, cur)) {
      if (cur) this.retire(cur);
      this.current = b;
    } else {
      this.waiting.push(b);
    }
  }

  /** The user closed the bubble: its topic stays quiet until `end` is called for it. */
  dismiss(): void {
    if (this.current?.key !== undefined) this.muted.add(this.current.key);
    this.close();
  }

  /** The bubble's time is up: the next one shows, and its topic may speak again. */
  expire(): void {
    this.close();
  }

  /** The operation behind `key` ended: its bubbles go and the topic may speak again. */
  end(key: string): void {
    this.muted.delete(key);
    this.waiting = this.waiting.filter((w) => w.key !== key);
    if (this.current?.key === key) this.current = this.takeNext();
  }

  /** A bubble replaced by a newer one: warnings wait, the rest goes to the history. */
  private retire(b: Bubble): void {
    if (b.type === "warning") this.waiting.unshift(b);
    else if (b.type !== "progress") this.history.push(b);
  }

  private close(): void {
    if (this.current && this.current.type !== "progress") this.history.push(this.current);
    this.current = this.takeNext();
  }

  /** The highest-ranked waiting bubble, the newest first within a rank. */
  private takeNext(): Bubble | null {
    let best = -1;
    this.waiting.forEach((w, i) => {
      const b = this.waiting[best];
      if (!b || PRIORITY[w.type] > PRIORITY[b.type] || (PRIORITY[w.type] === PRIORITY[b.type] && w.id > b.id)) best = i;
    });
    return best < 0 ? null : (this.waiting.splice(best, 1)[0] ?? null);
  }
}

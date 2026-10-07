// Play-queue logic as pure functions over an immutable value.

export interface Queue<T> {
  readonly items: readonly T[];
  /** Index of the current item; -1 when the queue is empty. */
  readonly index: number;
}

export const emptyQueue = <T>(): Queue<T> => ({ items: [], index: -1 });

export function startQueue<T>(items: readonly T[], index: number): Queue<T> {
  if (items.length === 0) return emptyQueue();
  return { items, index: Math.min(Math.max(index, 0), items.length - 1) };
}

export function current<T>(queue: Queue<T>): T | undefined {
  return queue.items[queue.index];
}

/** The next track, or `null` at the end (no wrap-around: the player stops). */
export function advance<T>(queue: Queue<T>): Queue<T> | null {
  return queue.index + 1 < queue.items.length ? { items: queue.items, index: queue.index + 1 } : null;
}

/** Restart the current track when more than `restartAfterSeconds` in, else go back one. */
export function back<T>(queue: Queue<T>, positionSeconds: number, restartAfterSeconds = 3): Queue<T> {
  if (positionSeconds > restartAfterSeconds || queue.index <= 0) return queue;
  return { items: queue.items, index: queue.index - 1 };
}

/** Fisher-Yates with an injected random source, keeping `keep` first when given. */
export function shuffled<T>(items: readonly T[], random: () => number, keep?: T): readonly T[] {
  const rest = items.filter((item) => item !== keep);
  for (let i = rest.length - 1; i > 0; i -= 1) {
    const j = Math.floor(random() * (i + 1));
    const a = rest[i];
    const b = rest[j];
    if (a === undefined || b === undefined) continue;
    rest[i] = b;
    rest[j] = a;
  }
  return keep === undefined ? rest : [keep, ...rest];
}

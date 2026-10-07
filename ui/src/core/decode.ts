// Runtime validation for data that crosses the Rust boundary. Types vanish at runtime, so every
// answer is checked here once and only typed values go further.

export class DecodeError extends Error {
  constructor(message: string) {
    super(message);
    this.name = 'DecodeError';
  }
}

export type Rec = Readonly<Record<string, unknown>>;

export function rec(value: unknown, what: string): Rec {
  if (typeof value !== 'object' || value === null || Array.isArray(value)) {
    throw new DecodeError(`${what} must be an object`);
  }
  return value as Rec;
}

export function str(r: Rec, key: string): string {
  const value = r[key];
  if (typeof value !== 'string') throw new DecodeError(`${key} must be a string`);
  return value;
}

export function num(r: Rec, key: string): number {
  const value = r[key];
  if (typeof value !== 'number' || !Number.isFinite(value)) throw new DecodeError(`${key} must be a number`);
  return value;
}

export function bool(r: Rec, key: string): boolean {
  const value = r[key];
  if (typeof value !== 'boolean') throw new DecodeError(`${key} must be a boolean`);
  return value;
}

export function numOrNull(r: Rec, key: string): number | null {
  const value = r[key];
  if (value === null || value === undefined) return null;
  return num(r, key);
}

export function list<T>(value: unknown, what: string, item: (entry: unknown) => T): readonly T[] {
  if (!Array.isArray(value)) throw new DecodeError(`${what} must be a list`);
  return value.map((entry) => item(entry));
}

import { list, rec, str } from './decode.ts';

export interface Me {
  readonly username: string;
  readonly role: string;
  readonly apps: readonly string[];
}

export interface ModuleManifest {
  readonly id: string;
  readonly title: string;
  readonly summary: string;
}

/** `null` means nobody is signed in. */
export function decodeMe(value: unknown): Me | null {
  if (value === null) return null;
  const r = rec(value, 'account');
  return { username: str(r, 'username'), role: str(r, 'role'), apps: list(r['apps'], 'apps', (a) => String(a)) };
}

export function decodeManifests(value: unknown): readonly ModuleManifest[] {
  return list(value, 'modules', (entry) => {
    const r = rec(entry, 'module');
    return { id: str(r, 'id'), title: str(r, 'title'), summary: str(r, 'summary') };
  });
}

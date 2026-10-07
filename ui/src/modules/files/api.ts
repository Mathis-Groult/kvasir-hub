// Typed access to the Rust files module.

import { bool, list, num, numOrNull, rec, str } from '../../core/decode.ts';
import { call, mediaUrl, upload } from '../../core/transport.ts';

export interface Account {
  readonly name: string;
  readonly used: number;
  /** `null` = no quota. */
  readonly quota: number | null;
}

export interface Library {
  readonly id: string;
  readonly name: string;
  readonly kind: string;
  readonly owner: string;
  readonly size: number;
  readonly modified: string;
  readonly encrypted: boolean;
  readonly writable: boolean;
}

export interface Entry {
  readonly name: string;
  readonly path: string;
  readonly isDir: boolean;
  readonly size: number;
  /** Unix seconds. */
  readonly modified: number;
  readonly hasThumbnail: boolean;
}

export const decodeAccount = (value: unknown): Account => {
  const r = rec(value, 'account');
  return { name: str(r, 'name'), used: num(r, 'used'), quota: numOrNull(r, 'quota') };
};

export const decodeLibrary = (value: unknown): Library => {
  const r = rec(value, 'library');
  return {
    id: str(r, 'id'), name: str(r, 'name'), kind: str(r, 'kind'), owner: str(r, 'owner'),
    size: num(r, 'size'), modified: str(r, 'modified'), encrypted: bool(r, 'encrypted'), writable: bool(r, 'writable'),
  };
};

export const decodeEntry = (value: unknown): Entry => {
  const r = rec(value, 'entry');
  return {
    name: str(r, 'name'), path: str(r, 'path'), isDir: bool(r, 'isDir'), size: num(r, 'size'),
    modified: num(r, 'modified'), hasThumbnail: bool(r, 'hasThumbnail'),
  };
};

export interface Overview {
  readonly account: Account;
  readonly libraries: readonly Library[];
}

export const decodeOverview = (value: unknown): Overview => {
  const r = rec(value, 'overview');
  return { account: decodeAccount(r['account']), libraries: list(r['libraries'], 'libraries', decodeLibrary) };
};

export const filesApi = {
  overview: async (): Promise<Overview> => decodeOverview(await call('files', 'overview')),
  ls: async (repo: string, path: string): Promise<readonly Entry[]> =>
    list(await call('files', 'ls', { repo, path }), 'entries', decodeEntry),
  mkdir: (repo: string, path: string): Promise<unknown> => call('files', 'mkdir', { repo, path }),
  rename: (repo: string, path: string, name: string, isDir: boolean): Promise<unknown> =>
    call('files', 'rename', { repo, path, name, isDir }),
  remove: (repo: string, path: string, isDir: boolean): Promise<unknown> =>
    call('files', 'delete', { repo, path, isDir }),
  put: (repo: string, dir: string, name: string, bytes: ArrayBuffer): Promise<unknown> =>
    upload('files', 'put', { repo, dir, name }, bytes),
};

export const downloadUrl = (repo: string, path: string): string => mediaUrl('files', 'download', { repo, path });
export const thumbUrl = (repo: string, path: string, size: number): string =>
  mediaUrl('files', 'thumb', { repo, path, size: String(size) });

const IMAGE = /\.(png|jpe?g|gif|webp|bmp|svg|avif)$/i;
export const isImage = (name: string): boolean => IMAGE.test(name);

/** Parent folder of an absolute path; the root is its own parent. */
export function parentOf(path: string): string {
  const cut = path.replace(/\/+$/, '').lastIndexOf('/');
  return cut <= 0 ? '/' : path.slice(0, cut);
}

/** `/a/b` -> [{name:'a',path:'/a'},{name:'b',path:'/a/b'}] */
export function crumbs(path: string): readonly { readonly name: string; readonly path: string }[] {
  const parts = path.split('/').filter(Boolean);
  return parts.map((name, i) => ({ name, path: `/${parts.slice(0, i + 1).join('/')}` }));
}

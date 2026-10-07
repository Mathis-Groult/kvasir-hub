// Hash routing. Pure parts (matching, parsing, building) are separate from the DOM wiring.

export type Params = Readonly<Record<string, string>>;

export interface Location {
  readonly path: string;
  readonly query: Readonly<Record<string, string>>;
}

/** `#/files/abc?path=%2Fx` -> { path: '/files/abc', query: { path: '/x' } }. */
export function parseHash(hash: string): Location {
  const raw = hash.startsWith('#') ? hash.slice(1) : hash;
  const [pathPart = '', queryPart = ''] = raw.split('?', 2);
  const path = pathPart === '' ? '/' : pathPart.startsWith('/') ? pathPart : `/${pathPart}`;
  const query: Record<string, string> = {};
  for (const [key, value] of new URLSearchParams(queryPart)) query[key] = value;
  return { path, query };
}

export function buildHash(path: string, query: Readonly<Record<string, string>> = {}): string {
  const search = new URLSearchParams(Object.entries(query)).toString();
  return search === '' ? `#${path}` : `#${path}?${search}`;
}

/** Matches `/music/album/:id` against `/music/album/abc` -> { id: 'abc' }; null when it does not fit. */
export function matchRoute(pattern: string, path: string): Params | null {
  const want = pattern.split('/').filter(Boolean);
  const have = path.split('/').filter(Boolean);
  if (want.length !== have.length) return null;
  const params: Record<string, string> = {};
  for (const [index, segment] of want.entries()) {
    const actual = have[index];
    if (actual === undefined) return null;
    if (segment.startsWith(':')) params[segment.slice(1)] = decodeURIComponent(actual);
    else if (segment !== actual) return null;
  }
  return params;
}

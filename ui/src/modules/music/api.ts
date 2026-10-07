// Typed access to the Rust music module. Every answer is decoded here once.

import { bool, list, num, rec, str, type Rec } from '../../core/decode.ts';
import { call, mediaUrl } from '../../core/transport.ts';

export interface Album {
  readonly id: string;
  readonly name: string;
  readonly artist: string;
  readonly artistId: string;
  readonly year: number;
  readonly songCount: number;
  readonly duration: number;
  readonly cover: string;
}

export interface Song {
  readonly id: string;
  readonly title: string;
  readonly artist: string;
  readonly album: string;
  readonly albumId: string;
  readonly track: number;
  readonly disc: number;
  readonly duration: number;
  readonly cover: string;
  readonly format: string;
  readonly bitRate: number;
  readonly starred: boolean;
}

export interface Artist {
  readonly id: string;
  readonly name: string;
  readonly albumCount: number;
  readonly cover: string;
}

export interface Playlist {
  readonly id: string;
  readonly name: string;
  readonly songCount: number;
  readonly duration: number;
  readonly cover: string;
}

export const decodeAlbum = (value: unknown): Album => {
  const r = rec(value, 'album');
  return {
    id: str(r, 'id'), name: str(r, 'name'), artist: str(r, 'artist'), artistId: str(r, 'artistId'),
    year: num(r, 'year'), songCount: num(r, 'songCount'), duration: num(r, 'duration'), cover: str(r, 'cover'),
  };
};

export const decodeSong = (value: unknown): Song => {
  const r = rec(value, 'song');
  return {
    id: str(r, 'id'), title: str(r, 'title'), artist: str(r, 'artist'), album: str(r, 'album'),
    albumId: str(r, 'albumId'), track: num(r, 'track'), disc: num(r, 'disc'), duration: num(r, 'duration'),
    cover: str(r, 'cover'), format: str(r, 'format'), bitRate: num(r, 'bitRate'), starred: bool(r, 'starred'),
  };
};

export const decodeArtist = (value: unknown): Artist => {
  const r = rec(value, 'artist');
  return { id: str(r, 'id'), name: str(r, 'name'), albumCount: num(r, 'albumCount'), cover: str(r, 'cover') };
};

export const decodePlaylist = (value: unknown): Playlist => {
  const r = rec(value, 'playlist');
  return {
    id: str(r, 'id'), name: str(r, 'name'), songCount: num(r, 'songCount'),
    duration: num(r, 'duration'), cover: str(r, 'cover'),
  };
};

export type AlbumList = 'newest' | 'recent' | 'frequent' | 'random' | 'alphabeticalByName' | 'starred';

export interface Overview {
  readonly newest: readonly Album[];
  readonly recent: readonly Album[];
  readonly frequent: readonly Album[];
}

export const decodeOverview = (value: unknown): Overview => {
  const r = rec(value, 'overview');
  return {
    newest: list(r['newest'], 'newest', decodeAlbum),
    recent: list(r['recent'], 'recent', decodeAlbum),
    frequent: list(r['frequent'], 'frequent', decodeAlbum),
  };
};

export interface Search {
  readonly artists: readonly Artist[];
  readonly albums: readonly Album[];
  readonly songs: readonly Song[];
}

export const decodeSearch = (value: unknown): Search => {
  const r = rec(value, 'search');
  return {
    artists: list(r['artists'], 'artists', decodeArtist),
    albums: list(r['albums'], 'albums', decodeAlbum),
    songs: list(r['songs'], 'songs', decodeSong),
  };
};

const withSongs = <T>(value: unknown, key: string, head: (v: unknown) => T) => {
  const r: Rec = rec(value, key);
  return { head: head(r[key]), songs: list(r['songs'], 'songs', decodeSong) };
};

export const musicApi = {
  overview: async (): Promise<Overview> => decodeOverview(await call('music', 'overview')),
  albums: async (type: AlbumList, offset: number, size: number): Promise<readonly Album[]> =>
    list(await call('music', 'albums', { type, offset, size }), 'albums', decodeAlbum),
  album: async (id: string) => {
    const { head, songs } = withSongs(await call('music', 'album', { id }), 'album', decodeAlbum);
    return { album: head, songs };
  },
  artists: async (): Promise<readonly Artist[]> => list(await call('music', 'artists'), 'artists', decodeArtist),
  artist: async (id: string) => {
    const r = rec(await call('music', 'artist', { id }), 'artist');
    return { artist: decodeArtist(r['artist']), albums: list(r['albums'], 'albums', decodeAlbum) };
  },
  playlists: async (): Promise<readonly Playlist[]> =>
    list(await call('music', 'playlists'), 'playlists', decodePlaylist),
  playlist: async (id: string) => {
    const { head, songs } = withSongs(await call('music', 'playlist', { id }), 'playlist', decodePlaylist);
    return { playlist: head, songs };
  },
  search: async (query: string): Promise<Search> => decodeSearch(await call('music', 'search', { query })),
  star: (id: string, on: boolean): Promise<unknown> => call('music', 'star', { id, on }),
  scrobble: (id: string): Promise<unknown> => call('music', 'scrobble', { id }),
};

/** Up to `count` albums: recently played first, then the newest ones not already listed. */
export function pickShowcase(overview: Overview, count: number): readonly Album[] {
  const seen = new Set<string>();
  const out: Album[] = [];
  for (const album of [...overview.recent, ...overview.newest]) {
    if (seen.has(album.id)) continue;
    seen.add(album.id);
    out.push(album);
    if (out.length === count) break;
  }
  return out;
}

/** Square cover URL; an empty cover id has no URL. */
export function coverUrl(cover: string, size: number): string | null {
  return cover === '' ? null : mediaUrl('music', 'cover', { id: cover, size: String(size) });
}

export const streamUrl = (songId: string): string => mediaUrl('music', 'stream', { id: songId });

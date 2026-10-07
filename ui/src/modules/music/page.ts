// Music: Home shelves, Albums, Artists, Playlists, Search, album/artist/playlist detail.

import { h, setChildren } from '../../core/dom.ts';
import { icon } from '../../core/icons.ts';
import { formatDuration, plural } from '../../core/format.ts';
import { buildHash } from '../../core/router.ts';
import { loadInto } from '../../ui/async_view.ts';
import { button, iconButton, skeletonRows, skeletonTiles, stateBlock, toast } from '../../ui/components.ts';
import type { Page, PageContext } from '../registry.ts';
import { coverUrl, musicApi, type Album, type Artist, type Playlist, type Song } from './api.ts';

const BASE = '/music';
const PAGE_SIZE = 40;

const tabs = [
  { label: 'Home', path: BASE },
  { label: 'Albums', path: `${BASE}/albums` },
  { label: 'Artists', path: `${BASE}/artists` },
  { label: 'Playlists', path: `${BASE}/playlists` },
  { label: 'Search', path: `${BASE}/search` },
] as const;

function tabBar(active: string): HTMLElement {
  return h('nav', { class: 'chips', attrs: { 'aria-label': 'Music sections' } },
    ...tabs.map((tab) => h('a', { class: 'chip', attrs: { href: buildHash(tab.path), ...(tab.path === active ? { 'aria-current': 'page' } : {}) } }, tab.label)));
}

function art(cover: string, size: number, round = false): HTMLElement {
  const src = coverUrl(cover, size);
  const cls = round ? 'tile__art tile__art--round' : 'tile__art';
  return src === null
    ? h('div', { class: cls }, icon(round ? 'people' : 'album'))
    : h('img', { class: cls, attrs: { src, alt: '', loading: 'lazy', decoding: 'async' } });
}

function albumTile(album: Album): HTMLElement {
  const meta = album.year > 0 ? `${album.artist} · ${album.year}` : album.artist;
  return h('a', { class: 'tile', attrs: { href: buildHash(`${BASE}/album/${encodeURIComponent(album.id)}`) } },
    art(album.cover, 300), h('span', { class: 'tile__title truncate' }, album.name), h('span', { class: 'dim small truncate' }, meta));
}

function artistTile(artist: Artist): HTMLElement {
  const count = plural(artist.albumCount, 'album');
  return h('a', { class: 'tile', attrs: { href: buildHash(`${BASE}/artist/${encodeURIComponent(artist.id)}`) } },
    art(artist.cover, 300, true), h('span', { class: 'tile__title truncate' }, artist.name), h('span', { class: 'dim small' }, count));
}

function playlistTile(playlist: Playlist): HTMLElement {
  return h('a', { class: 'tile', attrs: { href: buildHash(`${BASE}/playlist/${encodeURIComponent(playlist.id)}`) } },
    art(playlist.cover, 300), h('span', { class: 'tile__title truncate' }, playlist.name),
    h('span', { class: 'dim small' }, `${plural(playlist.songCount, 'song')} · ${formatDuration(playlist.duration)}`));
}

function songRow(ctx: PageContext, songs: readonly Song[], index: number, showArt: boolean): HTMLElement {
  const song = songs[index];
  if (song === undefined) return h('span');
  const lead = showArt
    ? h('span', { class: 'row__lead' }, (() => { const src = coverUrl(song.cover, 96); return src === null ? icon('music') : h('img', { attrs: { src, alt: '', loading: 'lazy' } }); })())
    : h('span', { class: 'row__lead dim' }, song.track > 0 ? String(song.track) : icon('music'));
  const playing = ctx.player.playingId === song.id;
  const play = h('button', {
    class: 'row__play', attrs: { type: 'button', 'aria-label': `Play ${song.title} by ${song.artist}` }, on: { click: () => ctx.player.play(songs, index) },
  }, lead, h('span', { class: 'row__text' },
    h('span', { class: 'truncate strong' }, song.title),
    h('span', { class: 'truncate dim small' }, showArt ? `${song.artist} · ${song.album}` : song.artist)));
  const star = iconButton(song.starred ? 'star' : 'starOutline', song.starred ? `Unstar ${song.title}` : `Star ${song.title}`, () => {
    musicApi.star(song.id, !song.starred).then(() => toast(song.starred ? 'Removed from favourites' : 'Added to favourites')).catch(() => toast('Could not change the star', true));
  });
  return h('div', { class: 'row', attrs: { ...(playing ? { 'aria-current': 'true' } : {}) } },
    play, h('span', { class: 'row__trail' }, formatDuration(song.duration), star));
}

function songList(ctx: PageContext, songs: readonly Song[], showArt: boolean): HTMLElement {
  return h('div', { class: 'rows' }, ...songs.map((_, i) => songRow(ctx, songs, i, showArt)));
}

function shelf(title: string, albums: readonly Album[], moreHref?: string): HTMLElement | null {
  if (albums.length === 0) return null;
  return h('section', { class: 'shelf' },
    h('div', { class: 'page__head' }, h('h2', {}, title), moreHref === undefined ? null : h('a', { attrs: { href: moreHref } }, 'See all')),
    h('div', { class: 'shelf__row' }, ...albums.map(albumTile)));
}

function head(title: string, ...extra: readonly (HTMLElement | null)[]): HTMLElement {
  return h('div', { class: 'page__head' }, h('h1', {}, title), ...extra);
}

// Pages -----------------------------------------------------------------------------------------

function homePage(): HTMLElement {
  const body = h('div', { class: 'page' });
  void loadInto(body, {
    placeholder: () => skeletonTiles(6),
    load: () => musicApi.overview(),
    render: (data) => {
      const shelves = [
        shelf('Recently played', data.recent), shelf('Most played', data.frequent),
        shelf('Recently added', data.newest, buildHash(`${BASE}/albums`)),
      ].filter((s): s is HTMLElement => s !== null);
      return shelves.length > 0 ? shelves : [stateBlock('music', 'Your library is empty', 'Add music from the Kvasir website and it appears here.')];
    },
  });
  return h('div', { class: 'page' }, head('Music'), tabBar(BASE), body);
}

function albumsPage(): HTMLElement {
  const grid = h('div', { class: 'tiles' });
  const footer = h('div', { class: 'page__head' });
  let offset = 0;
  const more = (): void => {
    musicApi.albums('alphabeticalByName', offset, PAGE_SIZE).then((albums) => {
      grid.append(...albums.map(albumTile));
      offset += albums.length;
      setChildren(footer, albums.length === PAGE_SIZE ? button('Load more', more, 'plain') : null);
    }).catch(() => setChildren(footer, button('Could not load, try again', more, 'plain', 'refresh')));
  };
  more();
  return h('div', { class: 'page' }, head('Albums'), tabBar(`${BASE}/albums`), grid, footer);
}

function artistsPage(): HTMLElement {
  const body = h('div', { class: 'page' });
  void loadInto(body, {
    placeholder: () => skeletonTiles(8),
    load: () => musicApi.artists(),
    render: (artists) => artists.length === 0 ? stateBlock('people', 'No artists yet', 'They appear once music is in the library.') : h('div', { class: 'tiles' }, ...artists.map(artistTile)),
  });
  return h('div', { class: 'page' }, head('Artists'), tabBar(`${BASE}/artists`), body);
}

function playlistsPage(): HTMLElement {
  const body = h('div', { class: 'page' });
  void loadInto(body, {
    placeholder: () => skeletonTiles(4),
    load: () => musicApi.playlists(),
    render: (lists) => lists.length === 0 ? stateBlock('playlist', 'No playlists yet', 'Playlists you make in the music player show up here.') : h('div', { class: 'tiles' }, ...lists.map(playlistTile)),
  });
  return h('div', { class: 'page' }, head('Playlists'), tabBar(`${BASE}/playlists`), body);
}

function searchPage(ctx: PageContext, query: string): HTMLElement {
  const results = h('div', { class: 'page' });
  const input = h('input', { class: 'input', attrs: { id: 'music-q', type: 'search', name: 'q', value: query, placeholder: 'Songs, albums, artists', 'aria-label': 'Search music', autocomplete: 'off', enterkeyhint: 'search' } });
  const form = h('form', { class: 'search', attrs: { role: 'search' }, on: { submit: (event) => {
    event.preventDefault();
    const q = input.value.trim();
    window.location.hash = q === '' ? buildHash(`${BASE}/search`) : buildHash(`${BASE}/search`, { q });
  } } }, input, h('button', { class: 'btn btn--primary', attrs: { type: 'submit' } }, icon('search'), 'Search'));
  if (query !== '') {
    void loadInto(results, {
      placeholder: () => skeletonRows(6),
      load: () => musicApi.search(query),
      render: (found) => {
        const sections: HTMLElement[] = [];
        if (found.songs.length > 0) sections.push(h('section', { class: 'shelf' }, h('h2', {}, 'Songs'), songList(ctx, found.songs, true)));
        if (found.albums.length > 0) sections.push(h('section', { class: 'shelf' }, h('h2', {}, 'Albums'), h('div', { class: 'shelf__row' }, ...found.albums.map(albumTile))));
        if (found.artists.length > 0) sections.push(h('section', { class: 'shelf' }, h('h2', {}, 'Artists'), h('div', { class: 'shelf__row' }, ...found.artists.map(artistTile))));
        return sections.length > 0 ? sections : [stateBlock('search', 'Nothing found', `No results for “${query}”.`)];
      },
    });
  }
  return h('div', { class: 'page' }, head('Search'), tabBar(`${BASE}/search`), form, results);
}

function albumPage(ctx: PageContext, id: string): HTMLElement {
  const body = h('div', { class: 'page' });
  void loadInto(body, {
    placeholder: () => skeletonRows(8),
    load: () => musicApi.album(id),
    render: ({ album, songs }) => {
      const cover = coverUrl(album.cover, 600);
      const meta = [album.year > 0 ? String(album.year) : '', plural(album.songCount, 'song'), formatDuration(album.duration)].filter(Boolean).join(' · ');
      return [
        h('div', { class: 'hero' },
          cover === null ? h('div', { class: 'hero__art' }) : h('img', { class: 'hero__art', attrs: { src: cover, alt: `Cover of ${album.name}` } }),
          h('div', { class: 'page' }, h('h1', {}, album.name),
            h('a', { attrs: { href: buildHash(`${BASE}/artist/${encodeURIComponent(album.artistId)}`) } }, album.artist),
            h('p', { class: 'dim' }, meta),
            h('div', { class: 'hero__actions' }, button('Play', () => ctx.player.play(songs, 0), 'primary', 'play')))),
        songList(ctx, songs, false),
      ];
    },
  });
  return h('div', { class: 'page' }, backLink(), body);
}

function artistPage(id: string): HTMLElement {
  const body = h('div', { class: 'page' });
  void loadInto(body, {
    placeholder: () => skeletonTiles(4),
    load: () => musicApi.artist(id),
    render: ({ artist, albums }) => [head(artist.name), h('div', { class: 'tiles' }, ...albums.map(albumTile))],
  });
  return h('div', { class: 'page' }, backLink(), body);
}

function playlistPage(ctx: PageContext, id: string): HTMLElement {
  const body = h('div', { class: 'page' });
  void loadInto(body, {
    placeholder: () => skeletonRows(8),
    load: () => musicApi.playlist(id),
    render: ({ playlist, songs }) => [
      head(playlist.name, songs.length > 0 ? button('Play', () => ctx.player.play(songs, 0), 'primary', 'play') : null),
      songs.length === 0 ? stateBlock('playlist', 'Empty playlist', 'Nothing here yet.') : songList(ctx, songs, true),
    ],
  });
  return h('div', { class: 'page' }, backLink(), body);
}

function backLink(): HTMLElement {
  return h('button', { class: 'btn btn--start', attrs: { type: 'button' }, on: { click: () => window.history.back() } }, icon('back'), 'Back');
}

export const musicPage: Page = {
  routes: [
    { pattern: BASE, render: () => homePage() },
    { pattern: `${BASE}/albums`, render: () => albumsPage() },
    { pattern: `${BASE}/artists`, render: () => artistsPage() },
    { pattern: `${BASE}/playlists`, render: () => playlistsPage() },
    { pattern: `${BASE}/search`, render: (ctx) => searchPage(ctx, ctx.location.query['q'] ?? '') },
    { pattern: `${BASE}/album/:id`, render: (ctx) => albumPage(ctx, ctx.params['id'] ?? '') },
    { pattern: `${BASE}/artist/:id`, render: (ctx) => artistPage(ctx.params['id'] ?? '') },
    { pattern: `${BASE}/playlist/:id`, render: (ctx) => playlistPage(ctx, ctx.params['id'] ?? '') },
  ],
};

import { h } from '../../core/dom.ts';
import { buildHash } from '../../core/router.ts';
import { coverUrl, musicApi, pickShowcase } from './api.ts';

const SHOWCASE_COUNT = 4;

/** Home widget: the most recently played (or newest) albums. */
export async function musicWidget(): Promise<HTMLElement | null> {
  const overview = await musicApi.overview();
  const albums = pickShowcase(overview, SHOWCASE_COUNT);
  if (albums.length === 0) return null;
  return h('section', { class: 'card widget' },
    h('div', { class: 'widget__row' }, h('h2', {}, 'Music'), h('a', { class: 'small', attrs: { href: buildHash('/music') } }, 'Open')),
    h('div', { class: 'tiles tiles--four' },
      ...albums.map((album) => {
        const src = coverUrl(album.cover, 200);
        return h('a', { attrs: { href: buildHash(`/music/album/${encodeURIComponent(album.id)}`), 'aria-label': `${album.name} by ${album.artist}` } },
          src === null ? h('div', { class: 'tile__art' }) : h('img', { class: 'tile__art', attrs: { src, alt: '', loading: 'lazy' } }));
      })));
}

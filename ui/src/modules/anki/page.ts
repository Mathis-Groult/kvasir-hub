// Anki: what to type into Anki / AnkiDroid to sync with the Kvasir server.

import { h } from '../../core/dom.ts';
import { loadInto } from '../../ui/async_view.ts';
import { iconButton, skeletonRows, toast } from '../../ui/components.ts';
import type { Page } from '../registry.ts';
import { ankiApi } from './api.ts';

/** Copies text; falls back to a message when the clipboard is unavailable (insecure context). */
async function copy(text: string, what: string): Promise<void> {
  try {
    await navigator.clipboard.writeText(text);
    toast(`${what} copied`);
  } catch {
    toast(`Select and copy the ${what.toLowerCase()} by hand`, true);
  }
}

function field(label: string, value: string): HTMLElement {
  return h('div', { class: 'card' },
    h('div', { class: 'widget__row' },
      h('div', { class: 'row__text' }, h('span', { class: 'dim small' }, label), h('code', { class: 'code' }, value)),
      iconButton('copy', `Copy ${label.toLowerCase()}`, () => { void copy(value, label); })));
}

function steps(title: string, items: readonly string[]): HTMLElement {
  return h('section', { class: 'card page' }, h('h2', {}, title), h('ol', { class: 'steps' }, ...items.map((item) => h('li', {}, item))));
}

function ankiPage(): HTMLElement {
  const body = h('div', { class: 'page' });
  void loadInto(body, {
    placeholder: () => skeletonRows(3),
    load: () => ankiApi.setup(),
    render: (setup) => [
      h('p', { class: 'dim' }, 'Anki syncs your cards through your own server. Sign in there with your Kvasir password.'),
      field('Sync URL', setup.syncUrl),
      field('Username', setup.username),
      steps('Anki on desktop', ['Open Preferences, then Syncing.', 'Paste the sync URL under “Self-hosted sync server” and restart Anki.', 'Click Sync and sign in with the username above and your Kvasir password.']),
      steps('AnkiDroid', ['Open Settings, then Sync.', 'Set “Custom sync server” to the sync URL.', 'Sign in with the same username and password.']),
      h('p', { class: 'dim small' }, 'On the first sync from the device that holds your cards, choose Upload.'),
    ],
  });
  return h('div', { class: 'page' }, h('div', { class: 'page__head' }, h('h1', {}, 'Anki')), body);
}

export const ankiPageDef: Page = { routes: [{ pattern: '/anki', render: () => ankiPage() }] };

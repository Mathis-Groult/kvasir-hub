import { h, setChildren } from '../core/dom.ts';
import { icon } from '../core/icons.ts';
import { buildHash } from '../core/router.ts';
import { MODULES } from '../modules/index.ts';
import { stateBlock } from '../ui/components.ts';

export interface Manifest {
  readonly id: string;
  readonly title: string;
  readonly summary: string;
}

function tile(manifest: Manifest, iconName: Parameters<typeof icon>[0], path: string): HTMLElement {
  return h('a', { class: 'card card--link app-tile', attrs: { href: buildHash(path) } },
    h('span', { class: 'app-tile__icon' }, icon(iconName)),
    h('h2', {}, manifest.title),
    h('p', { class: 'app-tile__summary' }, manifest.summary));
}

/** Home: greeting, live widgets first (what changed), then every app the account may open. */
export function homePage(username: string, manifests: readonly Manifest[]): HTMLElement {
  const granted = MODULES.filter((m) => manifests.some((manifest) => manifest.id === m.id));
  const widgets = h('div', { class: 'widgets' });
  for (const module of granted) {
    if (module.widget === undefined) continue;
    // A widget that fails must never break Home: it simply does not appear.
    void module.widget().then((node) => { if (node !== null) widgets.append(node); }).catch(() => undefined);
  }
  const tiles = granted.flatMap((module) => {
    const manifest = manifests.find((m) => m.id === module.id);
    return manifest === undefined ? [] : [tile(manifest, module.icon, module.path)];
  });
  return h('div', { class: 'page' },
    h('div', { class: 'greeting' }, h('h1', {}, `Hello, ${username}`), h('p', { class: 'dim' }, 'Everything on Kvasir, in one place.')),
    widgets,
    h('section', { attrs: { 'aria-labelledby': 'apps-h' }, class: 'page' }, h('h2', { attrs: { id: 'apps-h' } }, 'Your apps'),
      tiles.length > 0 ? h('div', { class: 'app-grid' }, ...tiles) : stateBlock('apps', 'No apps yet', 'An admin has not given your account access to any app.')));
}

export function clearHome(container: HTMLElement): void {
  setChildren(container);
}

// App shell: restore session or show login, then navigation + routed pages + player.

import { h, setChildren } from './core/dom.ts';
import { icon } from './core/icons.ts';
import { buildHash, matchRoute, parseHash } from './core/router.ts';
import { command, failureFrom, initTransport } from './core/transport.ts';
import { decodeManifests, decodeMe } from './core/session.ts';
import { MODULES } from './modules/index.ts';
import type { PageContext } from './modules/registry.ts';
import { showLogin } from './pages/login.ts';
import { homePage } from './pages/home.ts';
import { brand } from './ui/brand.ts';
import { button, menu, stateBlock } from './ui/components.ts';
import { activeItem, bottomItems, type NavItem } from './ui/nav.ts';
import { Player } from './ui/player.ts';

const root = document.getElementById('app');
if (root === null) throw new Error('missing #app');
const mount: HTMLElement = root;

async function start(): Promise<void> {
  try {
    await initTransport();
    const restored = await command('restore');
    if (restored === null) await showLogin(mount);
  } catch (error) {
    const failure = failureFrom(error);
    // Saved credentials exist but the server is unreachable: say so, never log the user out.
    setChildren(mount, h('main', { class: 'login' }, h('div', { class: 'card login__card' },
      stateBlock('warning', 'Cannot reach Kvasir', failure.message, button('Try again', () => { void start(); }, 'primary', 'refresh'), true))));
    return;
  }
  await runApp();
}

async function runApp(): Promise<void> {
  const [me, manifests] = await Promise.all([command('restore').then(decodeMe), command('modules').then(decodeManifests)]);
  if (me === null) { await showLogin(mount); return runApp(); }
  const player = new Player((visible) => document.querySelector('.app')?.classList.toggle('app--player', visible));

  const nav: readonly NavItem[] = [
    { id: 'home', label: 'Home', path: '/' },
    ...MODULES.flatMap((module) => {
      const manifest = manifests.find((m) => m.id === module.id);
      return manifest === undefined ? [] : [{ id: module.id, label: manifest.title, path: module.path }];
    }),
  ];
  const iconFor = (id: string) => (id === 'home' ? 'home' : (MODULES.find((m) => m.id === id)?.icon ?? 'apps'));

  const main = h('main', { class: 'main', attrs: { id: 'content', tabindex: '-1' } });
  const spacer = h('span', { class: 'topbar__title' });
  const bottom = h('nav', { class: 'bottomnav', attrs: { 'aria-label': 'Primary' } });
  const rail = h('nav', { class: 'rail', attrs: { 'aria-label': 'Primary' } });

  const doSignOut = (): void => { void command('sign_out').then(() => window.location.reload()); };
  const signOut = button('Sign out', doSignOut, 'plain', 'logout');
  const app = h('div', { class: 'app' },
    h('a', { class: 'sr-only', attrs: { href: '#content' } }, 'Skip to content'),
    rail,
    h('header', { class: 'topbar' }, brand(), spacer,
      h('div', { class: 'account' }, h('span', { class: 'dim small truncate account__name' }, me.username),
        menu('Account', [{ label: 'Sign out', icon: 'logout', run: doSignOut }]))),
    main, bottom, player.element);

  const linkFor = (item: NavItem, current: NavItem | undefined): HTMLElement =>
    h('a', { class: 'nav-item', attrs: { href: buildHash(item.path), ...(current?.id === item.id ? { 'aria-current': 'page' } : {}) } }, icon(iconFor(item.id)), item.label);

  const route = (): void => {
    const location = parseHash(window.location.hash);
    const current = activeItem(nav, location.path);
    document.title = current === undefined || current.id === 'home' ? 'Kvasir' : `${current.label} · Kvasir`;
    setChildren(bottom, ...bottomItems(nav).map((item) => linkFor(item, current)));
    setChildren(rail, brand(), ...nav.map((item) => linkFor(item, current)), h('div', { class: 'rail__spacer' }), h('div', { class: 'dim small rail__user' }, me.username), signOut);
    let page: HTMLElement | null = null;
    if (location.path === '/') page = homePage(me.username, manifests);
    for (const module of MODULES) {
      if (!manifests.some((m) => m.id === module.id)) continue;
      for (const candidate of module.page.routes) {
        const params = matchRoute(candidate.pattern, location.path);
        if (params === null) continue;
        const context: PageContext = { params, location, player };
        page = candidate.render(context);
      }
    }
    setChildren(main, page ?? stateBlock('search', 'Page not found', 'This page does not exist.', h('a', { class: 'btn', attrs: { href: buildHash('/') } }, 'Go home')));
    window.scrollTo(0, 0);
  };
  setChildren(mount, app);
  window.addEventListener('hashchange', route);
  route();
}

window.addEventListener('unhandledrejection', (event) => {
  console.error('unhandled rejection', event.reason);
});

void start();

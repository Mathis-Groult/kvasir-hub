// Shared building blocks. Each returns a real, semantic element (button, a, dialog).

import { h, type Child } from '../core/dom.ts';
import { icon, type IconName } from '../core/icons.ts';
import { percent } from '../core/format.ts';

export function button(label: string, onClick: () => void, variant: 'primary' | 'plain' | 'danger' = 'plain', iconName?: IconName): HTMLButtonElement {
  const cls = variant === 'primary' ? 'btn btn--primary' : variant === 'danger' ? 'btn btn--danger' : 'btn';
  return h('button', { class: cls, attrs: { type: 'button' }, on: { click: onClick } }, iconName ? icon(iconName) : null, label);
}

/** An icon-only button: the accessible name is mandatory. */
export function iconButton(name: IconName, label: string, onClick: () => void, filled = false): HTMLButtonElement {
  return h('button', {
    class: filled ? 'icon-btn icon-btn--filled' : 'icon-btn',
    attrs: { type: 'button', 'aria-label': label, title: label },
    on: { click: onClick },
  }, icon(name));
}

export function stateBlock(iconName: IconName, title: string, detail: string, action?: HTMLElement, error = false): HTMLElement {
  return h('div', { class: error ? 'state state--error' : 'state', attrs: { role: error ? 'alert' : 'status' } },
    icon(iconName), h('h2', {}, title), h('p', {}, detail), action ?? null);
}

export function skeletonRows(count: number): HTMLElement {
  const rows: Child[] = [];
  for (let i = 0; i < count; i += 1) rows.push(h('div', { class: 'skeleton skeleton--row' }));
  return h('div', { class: 'rows', attrs: { 'aria-busy': 'true', 'aria-label': 'Loading' } }, ...rows);
}

export function skeletonTiles(count: number): HTMLElement {
  const tiles: Child[] = [];
  for (let i = 0; i < count; i += 1) tiles.push(h('div', { class: 'skeleton skeleton--tile' }));
  return h('div', { class: 'tiles', attrs: { 'aria-busy': 'true', 'aria-label': 'Loading' } }, ...tiles);
}

/** A usage meter with text next to it: colour is never the only signal. */
export function meter(used: number, total: number): HTMLElement {
  const pct = percent(used, total);
  const level = pct >= 90 ? 'meter meter--bad' : pct >= 70 ? 'meter meter--warn' : 'meter';
  const node = h('div', {
    class: level,
    attrs: { role: 'meter', 'aria-valuemin': '0', 'aria-valuemax': '100', 'aria-valuenow': String(pct), 'aria-label': 'Storage used' },
  }, h('div', { class: 'meter__fill' }));
  // CSSOM writes are allowed by the strict CSP; a `style` attribute would not be.
  node.style.setProperty('--value', `${pct}%`);
  return node;
}

// Toasts ------------------------------------------------------------------------------------

const TOAST_MS = 4000;

function toastHost(): HTMLElement {
  const existing = document.querySelector('.toasts');
  if (existing instanceof HTMLElement) return existing;
  const host = h('div', { class: 'toasts', attrs: { role: 'status', 'aria-live': 'polite' } });
  document.body.append(host);
  return host;
}

export function toast(message: string, error = false): void {
  const node = h('div', { class: error ? 'toast toast--error' : 'toast' }, message);
  toastHost().append(node);
  window.setTimeout(() => node.remove(), TOAST_MS);
}

// Dialogs (native <dialog>: focus trap, Escape and backdrop come for free) ----------------------

function openDialog(content: readonly Child[]): HTMLDialogElement {
  const dialog = h('dialog', {}, ...content);
  dialog.addEventListener('close', () => dialog.remove());
  document.body.append(dialog);
  dialog.showModal();
  return dialog;
}

export function confirmDialog(title: string, detail: string, confirmLabel: string, danger = false): Promise<boolean> {
  return new Promise((resolve) => {
    const dialog = openDialog([
      h('div', { class: 'dialog__body' }, h('h2', {}, title), h('p', { class: 'dim' }, detail)),
      h('div', { class: 'dialog__actions' },
        button('Cancel', () => { resolve(false); dialog.close(); }),
        button(confirmLabel, () => { resolve(true); dialog.close(); }, danger ? 'danger' : 'primary')),
    ]);
    dialog.addEventListener('cancel', () => resolve(false));
  });
}

/** Asks for one line of text; resolves `null` when cancelled. */
export function promptDialog(title: string, label: string, initial: string, confirmLabel: string): Promise<string | null> {
  return new Promise((resolve) => {
    const input = h('input', { class: 'input', attrs: { id: 'prompt-input', type: 'text', value: initial, autocomplete: 'off', spellcheck: 'false' } });
    const submit = (): void => {
      const value = input.value.trim();
      resolve(value === '' ? null : value);
      dialog.close();
    };
    const dialog = openDialog([
      h('form', { class: 'dialog__body', on: { submit: (event) => { event.preventDefault(); submit(); } } },
        h('h2', {}, title),
        h('div', { class: 'field' }, h('label', { attrs: { for: 'prompt-input' } }, label), input),
        h('div', { class: 'dialog__actions' },
          button('Cancel', () => { resolve(null); dialog.close(); }),
          h('button', { class: 'btn btn--primary', attrs: { type: 'submit' } }, confirmLabel))),
    ]);
    dialog.addEventListener('cancel', () => resolve(null));
    input.select();
  });
}

/** Full-screen image preview. */
export function imageDialog(src: string, name: string): void {
  const dialog = openDialog([
    h('div', { class: 'dialog__body' },
      h('h2', { class: 'truncate' }, name),
      h('img', { class: 'preview', attrs: { src, alt: name } }),
      h('div', { class: 'dialog__actions' }, button('Close', () => dialog.close()))),
  ]);
}

// Overflow menu ---------------------------------------------------------------------------------

export interface MenuAction {
  readonly label: string;
  readonly icon: IconName;
  readonly danger?: boolean;
  readonly run: () => void;
}

export function menu(label: string, actions: readonly MenuAction[]): HTMLElement {
  const panel = h('div', { class: 'menu__panel', attrs: { role: 'menu', hidden: '' } },
    ...actions.map((action) => h('button', {
      class: action.danger === true ? 'menu__item menu__item--danger' : 'menu__item',
      attrs: { type: 'button', role: 'menuitem' },
      on: { click: () => { panel.hidden = true; action.run(); } },
    }, icon(action.icon), action.label)));
  const trigger = iconButton('more', label, () => { panel.hidden = !panel.hidden; });
  trigger.setAttribute('aria-haspopup', 'menu');
  const root = h('div', { class: 'menu' }, trigger, panel);
  const close = (event: Event): void => {
    if (!root.isConnected) { document.removeEventListener('click', close); return; }
    if (event.target instanceof Node && !root.contains(event.target)) panel.hidden = true;
  };
  document.addEventListener('click', close);
  root.addEventListener('keydown', (event) => { if (event.key === 'Escape') { panel.hidden = true; trigger.focus(); } });
  return root;
}

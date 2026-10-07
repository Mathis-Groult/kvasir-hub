// Files: libraries list with quota, folder browser, preview, upload, folders, rename, delete.

import { h, setChildren } from '../../core/dom.ts';
import { icon } from '../../core/icons.ts';
import { formatAge, formatBytes, plural } from '../../core/format.ts';
import { buildHash } from '../../core/router.ts';
import { failureFrom } from '../../core/transport.ts';
import { loadInto } from '../../ui/async_view.ts';
import { button, confirmDialog, imageDialog, menu, meter, promptDialog, skeletonRows, stateBlock, toast, type MenuAction } from '../../ui/components.ts';
import type { Page } from '../registry.ts';
import { crumbs, downloadUrl, filesApi, isImage, thumbUrl, type Entry, type Library } from './api.ts';

const BASE = '/files';
const THUMB_PX = 96;

const folderHref = (repo: string, path: string): string => buildHash(`${BASE}/${encodeURIComponent(repo)}`, { path });

function quotaCard(used: number, quota: number | null): HTMLElement {
  return h('div', { class: 'card widget' },
    h('div', { class: 'widget__row' }, h('h2', {}, 'Storage'), h('span', { class: 'dim' }, quota === null ? `${formatBytes(used)} used` : `${formatBytes(used)} of ${formatBytes(quota)}`)),
    quota === null ? null : meter(used, quota));
}

function libraryRow(library: Library): HTMLElement {
  const meta = [library.kind === 'mine' ? 'Yours' : library.owner, formatBytes(library.size), library.writable ? '' : 'read-only', library.encrypted ? 'encrypted' : ''].filter(Boolean).join(' · ');
  return h('a', { class: 'row', attrs: { href: folderHref(library.id, '/') } },
    h('span', { class: 'row__lead row__lead--folder' }, icon('folder')),
    h('span', { class: 'row__text' }, h('span', { class: 'truncate strong' }, library.name), h('span', { class: 'truncate dim small' }, meta)),
    h('span', { class: 'row__trail' }, icon('chevron')));
}

function librariesPage(): HTMLElement {
  const body = h('div', { class: 'page' });
  void loadInto(body, {
    placeholder: () => skeletonRows(4),
    load: () => filesApi.overview(),
    render: ({ account, libraries }) => [
      quotaCard(account.used, account.quota),
      libraries.length === 0 ? stateBlock('folder', 'No libraries yet', 'Create one in the Seafile web app.') : h('div', { class: 'rows' }, ...libraries.map(libraryRow)),
    ],
  });
  return h('div', { class: 'page' }, h('div', { class: 'page__head' }, h('h1', {}, 'Files')), body);
}

// Folder view -----------------------------------------------------------------------------------

interface Folder {
  readonly repo: string;
  readonly path: string;
  readonly reload: () => void;
}

function fail(error: unknown, what: string): void {
  toast(`${what}: ${failureFrom(error).message}`, true);
}

async function uploadFiles(folder: Folder, files: readonly File[]): Promise<void> {
  for (const file of files) {
    toast(`Uploading ${file.name}…`);
    try {
      await filesApi.put(folder.repo, folder.path, file.name, await file.arrayBuffer());
    } catch (error) {
      fail(error, `Could not upload ${file.name}`);
      return;
    }
  }
  toast(files.length === 1 ? 'Upload finished' : `${files.length} uploads finished`);
  folder.reload();
}

function entryActions(folder: Folder, entry: Entry): readonly MenuAction[] {
  const actions: MenuAction[] = [];
  if (!entry.isDir) actions.push({ label: 'Download', icon: 'download', run: () => { window.location.assign(downloadUrl(folder.repo, entry.path)); } });
  actions.push(
    { label: 'Rename', icon: 'edit', run: () => {
      void promptDialog('Rename', 'New name', entry.name, 'Rename').then((name) => {
        if (name === null || name === entry.name) return;
        filesApi.rename(folder.repo, entry.path, name, entry.isDir).then(folder.reload).catch((e: unknown) => fail(e, 'Could not rename'));
      });
    } },
    { label: 'Delete', icon: 'delete', danger: true, run: () => {
      void confirmDialog(`Delete “${entry.name}”?`, entry.isDir ? 'The folder and everything in it will be deleted.' : 'The file will be deleted.', 'Delete', true).then((yes) => {
        if (!yes) return;
        filesApi.remove(folder.repo, entry.path, entry.isDir).then(folder.reload).catch((e: unknown) => fail(e, 'Could not delete'));
      });
    } });
  return actions;
}

function entryRow(folder: Folder, entry: Entry, writable: boolean): HTMLElement {
  const lead = h('span', { class: entry.isDir ? 'row__lead row__lead--folder' : 'row__lead' },
    entry.isDir ? icon('folder')
      : entry.hasThumbnail && isImage(entry.name) ? h('img', { attrs: { src: thumbUrl(folder.repo, entry.path, THUMB_PX), alt: '', loading: 'lazy' } })
      : icon(isImage(entry.name) ? 'image' : 'file'));
  const meta = entry.isDir ? formatAge(entry.modified, Math.floor(Date.now() / 1000)) : `${formatBytes(entry.size)} · ${formatAge(entry.modified, Math.floor(Date.now() / 1000))}`;
  const text = h('span', { class: 'row__text' }, h('span', { class: 'truncate strong' }, entry.name), h('span', { class: 'truncate dim small' }, meta));
  const main = entry.isDir
    ? h('a', { class: 'row__play', attrs: { href: folderHref(folder.repo, entry.path) } }, lead, text)
    : h('button', { class: 'row__play', attrs: { type: 'button' }, on: { click: () => {
      if (isImage(entry.name)) imageDialog(downloadUrl(folder.repo, entry.path), entry.name);
      else window.location.assign(downloadUrl(folder.repo, entry.path));
    } } }, lead, text);
  return h('div', { class: 'row' }, main, writable || !entry.isDir ? menu(`Actions for ${entry.name}`, writable ? entryActions(folder, entry) : entryActions(folder, entry).filter((a) => a.label === 'Download')) : null);
}

function folderPage(repo: string, path: string): HTMLElement {
  const list = h('div', { class: 'page' });
  const toolbar = h('div', { class: 'hero__actions' });
  const hint = h('div', { class: 'dropzone-hint' }, 'Drop files here to upload');
  const picker = h('input', { attrs: { type: 'file', multiple: '', hidden: '', 'aria-hidden': 'true', tabindex: '-1' } });
  let writable = true;

  const reload = (): void => {
    void loadInto(list, {
      placeholder: () => skeletonRows(6),
      load: async () => ({ entries: await filesApi.ls(repo, path), overview: await filesApi.overview() }),
      render: ({ entries, overview }) => {
        writable = overview.libraries.find((l) => l.id === repo)?.writable ?? false;
        const mine = { repo, path, reload };
        setChildren(toolbar,
          writable ? button('Upload', () => picker.click(), 'primary', 'upload') : null,
          writable ? button('New folder', () => {
            void promptDialog('New folder', 'Folder name', '', 'Create').then((name) => {
              if (name === null) return;
              filesApi.mkdir(repo, `${path === '/' ? '' : path}/${name}`).then(reload).catch((e: unknown) => fail(e, 'Could not create the folder'));
            });
          }, 'plain', 'newFolder') : null,
          button('Refresh', reload, 'plain', 'refresh'));
        return entries.length === 0
          ? [stateBlock('folder', 'This folder is empty', writable ? 'Upload files or drop them on this page.' : 'Nothing here.')]
          : [h('div', { class: 'rows' }, ...entries.map((entry) => entryRow(mine, entry, writable)))];
      },
    });
  };

  picker.addEventListener('change', () => {
    const files = Array.from(picker.files ?? []);
    picker.value = '';
    if (files.length > 0) void uploadFiles({ repo, path, reload }, files);
  });
  const page = h('div', { class: 'page' },
    h('div', { class: 'page__head' }, h('h1', {}, path === '/' ? 'Library' : (crumbs(path).at(-1)?.name ?? 'Library'))),
    h('nav', { class: 'crumbs', attrs: { 'aria-label': 'Folder path' } },
      h('a', { attrs: { href: buildHash(BASE) } }, 'Libraries'),
      ...crumbs(path).flatMap((c) => [h('span', { class: 'crumbs__sep', attrs: { 'aria-hidden': 'true' } }, '/'), h('a', { attrs: { href: folderHref(repo, c.path) } }, c.name)])),
    toolbar, picker, list, hint);
  for (const type of ['dragover', 'dragenter'] as const) {
    page.addEventListener(type, (event) => { if (writable) { event.preventDefault(); hint.classList.add('dropzone-hint--active'); } });
  }
  page.addEventListener('dragleave', () => hint.classList.remove('dropzone-hint--active'));
  page.addEventListener('drop', (event) => {
    event.preventDefault();
    hint.classList.remove('dropzone-hint--active');
    const files = Array.from(event.dataTransfer?.files ?? []);
    if (writable && files.length > 0) void uploadFiles({ repo, path, reload }, files);
  });
  reload();
  return page;
}

export const filesPage: Page = {
  routes: [
    { pattern: BASE, render: () => librariesPage() },
    { pattern: `${BASE}/:repo`, render: (ctx) => folderPage(ctx.params['repo'] ?? '', ctx.location.query['path'] ?? '/') },
  ],
};

/** Home widget: storage used of quota. */
export async function filesWidget(): Promise<HTMLElement | null> {
  const { account, libraries } = await filesApi.overview();
  return h('a', { class: 'card card--link widget', attrs: { href: buildHash(BASE) } },
    h('div', { class: 'widget__row' }, h('h2', {}, 'Files'), h('span', { class: 'dim small' }, plural(libraries.length, 'library', 'libraries'))),
    h('div', { class: 'widget__row' }, h('span', { class: 'big-number' }, formatBytes(account.used)), h('span', { class: 'dim' }, account.quota === null ? 'no limit' : `of ${formatBytes(account.quota)}`)),
    account.quota === null ? null : meter(account.used, account.quota));
}

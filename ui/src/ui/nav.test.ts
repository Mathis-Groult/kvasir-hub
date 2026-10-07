import assert from 'node:assert/strict';
import test from 'node:test';
import { activeItem, bottomItems, type NavItem } from './nav.ts';

const item = (id: string, path: string): NavItem => ({ id, label: id, path });
const ALL = [item('home', '/'), item('files', '/files'), item('music', '/music'), item('anki', '/anki'), item('hermes', '/hermes')];

test('the bottom bar keeps at most four destinations', () => {
  assert.deepEqual(bottomItems(ALL).map((i) => i.id), ['home', 'files', 'music', 'anki']);
  assert.equal(bottomItems(ALL.slice(0, 2)).length, 2);
});

test('Home is active only on the root; a module is active on its sub-pages', () => {
  assert.equal(activeItem(ALL, '/')?.id, 'home');
  assert.equal(activeItem(ALL, '/music/album/a1')?.id, 'music');
  assert.equal(activeItem(ALL, '/musical'), undefined);
});

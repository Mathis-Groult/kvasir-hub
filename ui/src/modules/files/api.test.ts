import assert from 'node:assert/strict';
import test from 'node:test';
import { crumbs, decodeEntry, decodeOverview, isImage, parentOf } from './api.ts';

test('overview from the core decodes, null quota stays null', () => {
  const overview = decodeOverview({
    account: { name: 'hubtest', quota: null, used: 0 },
    libraries: [{ id: 'r', name: 'My Library', kind: 'mine', owner: 'hubtest', size: 0, modified: '2026-10-07T15:36:26+00:00', encrypted: false, writable: true }],
  });
  assert.equal(overview.account.quota, null);
  assert.equal(overview.libraries[0]?.writable, true);
});

test('an entry decodes', () => {
  const entry = decodeEntry({ hasThumbnail: false, isDir: false, modified: 1791387978, name: 'blob é.bin', path: '/Hub test/blob é.bin', size: 300000 });
  assert.equal(entry.path, '/Hub test/blob é.bin');
});

test('parentOf and crumbs', () => {
  assert.equal(parentOf('/a/b'), '/a');
  assert.equal(parentOf('/a'), '/');
  assert.equal(parentOf('/'), '/');
  assert.deepEqual(crumbs('/a/b'), [{ name: 'a', path: '/a' }, { name: 'b', path: '/a/b' }]);
  assert.deepEqual(crumbs('/'), []);
});

test('image detection ignores case', () => {
  assert.equal(isImage('Photo.JPG'), true);
  assert.equal(isImage('notes.txt'), false);
});

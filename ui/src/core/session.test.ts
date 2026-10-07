import assert from 'node:assert/strict';
import test from 'node:test';
import { decodeManifests, decodeMe } from './session.ts';

test('no session decodes to null', () => assert.equal(decodeMe(null), null));

test('the account the core sends decodes', () => {
  const me = decodeMe({ apps: ['kvasir', 'music', 'files'], role: 'user', username: 'hubtest' });
  assert.equal(me?.username, 'hubtest');
  assert.deepEqual(me?.apps, ['kvasir', 'music', 'files']);
});

test('manifests need id, title and summary', () => {
  assert.equal(decodeManifests([{ id: 'files', summary: 'x', title: 'Files' }])[0]?.title, 'Files');
  assert.throws(() => decodeManifests([{ id: 'files' }]), /title must be a string/);
});

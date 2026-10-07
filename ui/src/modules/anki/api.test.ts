import assert from 'node:assert/strict';
import test from 'node:test';
import { decodeSetup } from './api.ts';

test('the setup answer from the core decodes', () => {
  const setup = decodeSetup({ syncUrl: 'https://anki.example.org/', username: 'hubtest' });
  assert.equal(setup.syncUrl, 'https://anki.example.org/');
  assert.equal(setup.username, 'hubtest');
});

test('a missing username is a loud error', () => {
  assert.throws(() => decodeSetup({ syncUrl: 'x' }), /username must be a string/);
});

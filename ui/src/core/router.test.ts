import assert from 'node:assert/strict';
import test from 'node:test';
import { buildHash, matchRoute, parseHash } from './router.ts';

test('parseHash splits path and decoded query', () => {
  assert.deepEqual(parseHash('#/files/abc?path=%2FMy%20Files&x=1'), {
    path: '/files/abc',
    query: { path: '/My Files', x: '1' },
  });
  assert.deepEqual(parseHash(''), { path: '/', query: {} });
  assert.deepEqual(parseHash('#music'), { path: '/music', query: {} });
});

test('buildHash round-trips with parseHash', () => {
  const hash = buildHash('/files/abc', { path: '/a b/é' });
  assert.deepEqual(parseHash(hash), { path: '/files/abc', query: { path: '/a b/é' } });
  assert.equal(buildHash('/music'), '#/music');
});

test('matchRoute binds params and requires the same length', () => {
  assert.deepEqual(matchRoute('/music/album/:id', '/music/album/a%201'), { id: 'a 1' });
  assert.equal(matchRoute('/music/album/:id', '/music/album'), null);
  assert.equal(matchRoute('/music', '/files'), null);
  assert.deepEqual(matchRoute('/', '/'), {});
});

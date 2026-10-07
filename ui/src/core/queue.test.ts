import assert from 'node:assert/strict';
import test from 'node:test';
import { advance, back, current, emptyQueue, shuffled, startQueue } from './queue.ts';

test('an empty list gives an empty queue and no current item', () => {
  assert.deepEqual(startQueue([], 3), emptyQueue());
  assert.equal(current(emptyQueue<string>()), undefined);
});

test('start clamps the index into range', () => {
  assert.equal(startQueue(['a', 'b'], 9).index, 1);
  assert.equal(startQueue(['a', 'b'], -4).index, 0);
});

test('advance moves on and stops at the end instead of wrapping', () => {
  const queue = startQueue(['a', 'b'], 0);
  const second = advance(queue);
  assert.equal(second === null ? null : current(second), 'b');
  assert.equal(second === null ? 'x' : advance(second), null);
});

test('back restarts after three seconds, else steps back, and never leaves the start', () => {
  const queue = startQueue(['a', 'b', 'c'], 2);
  assert.equal(back(queue, 10).index, 2);
  assert.equal(back(queue, 1).index, 1);
  assert.equal(back(startQueue(['a'], 0), 0).index, 0);
});

test('shuffle keeps every item once and the kept one first, deterministically', () => {
  const items = ['a', 'b', 'c', 'd', 'e'];
  const random = (() => {
    let state = 7;
    return () => {
      state = (state * 48271) % 2147483647;
      return state / 2147483647;
    };
  })();
  const out = shuffled(items, random, 'c');
  assert.equal(out[0], 'c');
  assert.deepEqual([...out].sort(), items);
});

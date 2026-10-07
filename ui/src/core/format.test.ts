import assert from 'node:assert/strict';
import test from 'node:test';
import { formatAge, formatBytes, formatDuration, percent, plural } from './format.ts';

test('bytes use SI units with sensible precision', () => {
  assert.equal(formatBytes(0), '0 B');
  assert.equal(formatBytes(999), '999 B');
  assert.equal(formatBytes(1_500), '1.5 KB');
  assert.equal(formatBytes(24_716_686), '24.7 MB');
  assert.equal(formatBytes(500_000_000), '500 MB');
  assert.equal(formatBytes(2_000_000_000), '2 GB');
  assert.equal(formatBytes(-5), '0 B');
});

test('durations switch to hours only when needed', () => {
  assert.equal(formatDuration(193), '3:13');
  assert.equal(formatDuration(5), '0:05');
  assert.equal(formatDuration(3725), '1:02:05');
  assert.equal(formatDuration(-3), '0:00');
});

test('ages are relative for a month then dated', () => {
  const now = 1_790_000_000;
  assert.equal(formatAge(now - 10, now), 'just now');
  assert.equal(formatAge(now - 600, now), '10 min ago');
  assert.equal(formatAge(now - 7200, now), '2 h ago');
  assert.equal(formatAge(now - 3 * 86_400, now), '3 d ago');
  assert.equal(formatAge(1_700_000_000, now), '2023-11-14');
});

test('plural picks the singular only for exactly one', () => {
  assert.equal(plural(1, 'song'), '1 song');
  assert.equal(plural(0, 'song'), '0 songs');
  assert.equal(plural(2, 'library', 'libraries'), '2 libraries');
});

test('percent is clamped and safe for empty totals', () => {
  assert.equal(percent(50, 200), 25);
  assert.equal(percent(5, 0), 0);
  assert.equal(percent(300, 200), 100);
});

import assert from 'node:assert/strict';
import test from 'node:test';
import { decodeAlbum, decodeOverview, decodeSong } from './api.ts';

// Literal answers captured from the Rust core against production on 2026-10-07.
const ALBUM = {
  artist: 'Claudio Capéo', artistId: '6t4AGhzLIlGIqmEmGez4Sg',
  cover: 'al-7HdlpPqVznCaho31Rq5UIY_2c61bd7fceffe41b', duration: 206,
  id: '7HdlpPqVznCaho31Rq5UIY', name: 'Claudio Capéo (Version deluxe)', songCount: 1, year: 2016,
};

test('an album from the core decodes', () => {
  assert.equal(decodeAlbum(ALBUM).name, 'Claudio Capéo (Version deluxe)');
  assert.equal(decodeAlbum(ALBUM).year, 2016);
});

test('a missing field is a loud decode error naming the field', () => {
  const { year: _year, ...broken } = ALBUM;
  assert.throws(() => decodeAlbum(broken), /year must be a number/);
});

test('a wrong type is refused instead of coerced', () => {
  assert.throws(() => decodeAlbum({ ...ALBUM, songCount: '1' }), /songCount must be a number/);
});

test('overview needs all three shelves', () => {
  assert.equal(decodeOverview({ newest: [ALBUM], recent: [], frequent: [] }).newest.length, 1);
  assert.throws(() => decodeOverview({ newest: [] }), /recent must be a list/);
});

test('a song decodes with its starred flag', () => {
  const song = decodeSong({
    id: 's', title: 'Tonight (Radio Edit)', artist: 'Yuksek', album: 'Away', albumId: 'a', track: 2, disc: 1,
    duration: 193, cover: 'dc-x', format: 'flac', bitRate: 1021, starred: false,
  });
  assert.equal(song.format, 'flac');
  assert.equal(song.starred, false);
});

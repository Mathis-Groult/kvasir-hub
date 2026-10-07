// The global music player: one <audio> element, a queue, Media Session (lock screen controls),
// and the mini bar. The queue maths lives in core/queue.ts.

import { h, setChildren } from '../core/dom.ts';
import { formatDuration } from '../core/format.ts';
import { advance, back, current, emptyQueue, startQueue, type Queue } from '../core/queue.ts';
import { coverUrl, musicApi, streamUrl, type Song } from '../modules/music/api.ts';
import { iconButton } from './components.ts';

/** Report a play to the server after this many seconds (or the whole track if shorter). */
const SCROBBLE_AFTER_SECONDS = 30;

export class Player {
  readonly element: HTMLElement;
  private readonly audio = new Audio();
  private queue: Queue<Song> = emptyQueue();
  private scrobbled = false;
  private readonly seek = h('input', { class: 'player__seek', attrs: { type: 'range', min: '0', max: '0', value: '0', step: '1', 'aria-label': 'Seek' } });
  private readonly time = h('span', { class: 'player__time' }, '0:00');
  private readonly meta = h('div', { class: 'player__meta' });
  private readonly art = h('img', { class: 'player__art', attrs: { alt: '' } });
  private readonly controls = h('div', { class: 'player__controls' });
  private readonly onVisibility: (visible: boolean) => void;

  constructor(onVisibility: (visible: boolean) => void) {
    this.onVisibility = onVisibility;
    this.audio.preload = 'metadata';
    this.element = h('section', { class: 'player', attrs: { 'aria-label': 'Player', hidden: '' } },
      this.art, this.meta, this.controls, this.seek);
    this.bind();
    this.render();
  }

  /** Starts `songs[index]` and keeps the rest as the queue. */
  play(songs: readonly Song[], index: number): void {
    this.queue = startQueue(songs, index);
    this.load();
  }

  private bind(): void {
    this.audio.addEventListener('timeupdate', () => this.onTime());
    this.audio.addEventListener('ended', () => this.next());
    this.audio.addEventListener('play', () => this.render());
    this.audio.addEventListener('pause', () => this.render());
    this.audio.addEventListener('error', () => this.render());
    this.seek.addEventListener('input', () => { this.audio.currentTime = Number(this.seek.value); });
    const session = navigator.mediaSession;
    session.setActionHandler('play', () => { void this.audio.play(); });
    session.setActionHandler('pause', () => this.audio.pause());
    session.setActionHandler('nexttrack', () => this.next());
    session.setActionHandler('previoustrack', () => this.previous());
    session.setActionHandler('seekto', (details) => { if (details.seekTime !== undefined) this.audio.currentTime = details.seekTime; });
  }

  private load(): void {
    const song = current(this.queue);
    if (song === undefined) return;
    this.scrobbled = false;
    this.audio.src = streamUrl(song.id);
    void this.audio.play().catch(() => this.render());
    this.onVisibility(true);
    this.element.hidden = false;
    this.announce(song);
    this.render();
  }

  private announce(song: Song): void {
    const art = coverUrl(song.cover, 256);
    navigator.mediaSession.metadata = new MediaMetadata({
      title: song.title, artist: song.artist, album: song.album,
      artwork: art === null ? [] : [{ src: new URL(art, window.location.href).href, sizes: '256x256' }],
    });
  }

  private next(): void {
    const following = advance(this.queue);
    if (following === null) { this.audio.pause(); this.audio.currentTime = 0; this.render(); return; }
    this.queue = following;
    this.load();
  }

  private previous(): void {
    this.queue = back(this.queue, this.audio.currentTime);
    this.load();
  }

  private toggle(): void {
    if (this.audio.paused) void this.audio.play(); else this.audio.pause();
  }

  private onTime(): void {
    const song = current(this.queue);
    if (song === undefined) return;
    const total = Number.isFinite(this.audio.duration) ? this.audio.duration : song.duration;
    this.seek.max = String(Math.floor(total));
    this.seek.value = String(Math.floor(this.audio.currentTime));
    this.time.textContent = `${formatDuration(this.audio.currentTime)} / ${formatDuration(total)}`;
    if (!this.scrobbled && this.audio.currentTime >= Math.min(SCROBBLE_AFTER_SECONDS, total)) {
      this.scrobbled = true;
      void musicApi.scrobble(song.id).catch(() => { /* a missed play count is not worth interrupting music */ });
    }
  }

  private render(): void {
    const song = current(this.queue);
    if (song === undefined) return;
    const src = coverUrl(song.cover, 96);
    if (src !== null) this.art.src = src; else this.art.removeAttribute('src');
    setChildren(this.meta, h('span', { class: 'truncate strong' }, song.title), h('span', { class: 'truncate dim small' }, song.artist));
    const playing = !this.audio.paused;
    setChildren(this.controls,
      this.time,
      iconButton('previous', 'Previous track', () => this.previous()),
      iconButton(playing ? 'pause' : 'play', playing ? 'Pause' : 'Play', () => this.toggle(), true),
      iconButton('next', 'Next track', () => this.next()));
    navigator.mediaSession.playbackState = playing ? 'playing' : 'paused';
  }

  /** Id of the song that is loaded, for highlighting the playing row. */
  get playingId(): string | null {
    return current(this.queue)?.id ?? null;
  }
}

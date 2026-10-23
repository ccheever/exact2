// The web hosts' media glue (media-glue.js) over a stand-in element: the
// commands by HTML's method names (podcast F8, F18) and a failure that
// landed before the glue had the element (F19).
import { test, expect } from 'bun:test';

globalThis.exact ??= {};
globalThis.IntersectionObserver ??= class { observe() {} disconnect() {} };
await import('../media-glue.js');

/** An `<audio>` as the glue drives it: seeks, loads and plays are counted. */
function audio(props, { readyState = 4, error = null, handlers = ['error', 'seeked', 'loadedmetadata'], duration = NaN, played = 0, position = null, takes = () => true } = {}) {
  const listeners = {}, el = {
    localName: 'audio', readyState, error, isConnected: true, paused: true, seeks: position == null ? [] : [position], loads: 0, plays: 0, attrs: new Set(),
    exactMedia: { props, handlers }, duration, played: { length: played },
    set currentTime(t) { this.seeks.push(t); }, get currentTime() { return this.seeks.at(-1) ?? 0; },
    addEventListener(name, f) { (listeners[name] ??= []).push(f); },
    removeEventListener(name, f) { listeners[name] = (listeners[name] ?? []).filter((g) => g !== f); },
    toggleAttribute(name, on) { if (on) this.attrs.add(name); else this.attrs.delete(name); },
    load() { this.loads++; this.readyState = 0; this.error = null; this.paused = true; },
    play() { this.plays++; this.paused = false; return Promise.resolve(); },
    pause() { this.paused = true; },
    fire(name) { for (const f of [...(listeners[name] ?? [])]) f({ type: name }); },
  };
  const sent = [];
  globalThis.exact.installMedia(el, text => takes() ? void sent.push(text) : false); // false: a report the host cannot take yet
  el.sent = sent;
  return el;
}
const command = (el, name, seconds) => { (el.exactMedia.commands ??= []).push([name, seconds]); globalThis.exact.installMedia(el); };

test('fastSeek seeks every time, to the time the bound currentTime already holds too', () => {
  const el = audio({ src: 'a.mp3', paused: 'false', currentTime: '60' });
  expect(el.seeks).toEqual([60]);
  globalThis.exact.installMedia(el); // an unrelated commit: the binding is unchanged
  expect(el.seeks).toEqual([60]);
  command(el, 'fastSeek', 60);
  command(el, 'fastSeek', 60);
  expect(el.seeks).toEqual([60, 60, 60]);
  command(el, 'fastSeek', -1);
  expect([el.seeks.length, el.sent.at(-1)]).toEqual([3, 'error\ninvalid-value']);
});

test('fastSeek before metadata waits for it, as the bound currentTime does', () => {
  const el = audio({ src: 'a.mp3' }, { readyState: 0 });
  command(el, 'fastSeek', 12.5);
  expect(el.seeks).toEqual([]);
  el.fire('loadedmetadata');
  expect(el.seeks).toEqual([12.5]);
});

test('load loads the source again, as a changed src does: the bound time waits for metadata, a bound play plays', () => {
  const el = audio({ src: 'a.mp3', paused: 'false', currentTime: '30' });
  const plays = el.plays;
  command(el, 'load');
  expect([el.loads, el.plays]).toEqual([1, plays + 1]);
  el.fire('loadedmetadata');
  expect(el.seeks.at(-1)).toBe(30);
});

test("a load or a seek in flight holds a settle until the element can play at rest; a changed source's `emptied` is its load beginning, not its end", async () => {
  const settledOf = (held) => { let done = false; held?.done.then(() => { done = true; }); return () => done; };
  const turn = () => new Promise((r) => setTimeout(r, 0));
  // Ready at rest: nothing held.
  expect(globalThis.exact.mediaHold(audio({ src: 'a.mp4' }))).toBe(null);
  // A changed source (readyState 0 as the browser starts its load): held through `emptied` and the opening seek.
  const el = audio({ src: 'b.mp4', currentTime: '0.5' }, { readyState: 0 });
  el.currentSrc = 'b.mp4'; el.seeking = false;
  const held = globalThis.exact.mediaHold(el), done = settledOf(held);
  el.fire('emptied'); await turn();
  expect(done()).toBe(false);
  el.readyState = 4; el.seeking = true; el.fire('canplay'); await turn();
  expect(done()).toBe(false); // seeking yet
  el.seeking = false; el.fire('seeked'); await turn();
  expect(done()).toBe(true);
  // At rest with its loading suspended (a paused `preload="metadata"` with its metadata, a `preload="none"` with
  // nothing): nothing held.
  expect(globalThis.exact.mediaHold(Object.assign(audio({ src: 'p.mp4' }, { readyState: 1 }), { networkState: 1, seeking: false }))).toBe(null);
  expect(globalThis.exact.mediaHold(Object.assign(audio({ src: 'n.mp4' }, { readyState: 0 }), { networkState: 1, seeking: false, preload: 'none' }))).toBe(null);
  // Idle with nothing decoded (its download done ahead of its decoding): held through the metadata and the seek
  // bound to wait for it.
  const early = Object.assign(audio({ src: 'd.mp4', currentTime: '0.5' }, { readyState: 0 }), { currentSrc: 'd.mp4', networkState: 1, seeking: false, preload: 'auto' });
  const earlyHeld = globalThis.exact.mediaHold(early), earlyDone = settledOf(earlyHeld);
  early.fire('suspend'); await turn();
  expect(earlyDone()).toBe(false);
  early.readyState = 1; early.seeking = true; early.fire('loadedmetadata'); await turn();
  expect(earlyDone()).toBe(false);
  early.readyState = 4; early.seeking = false; early.fire('seeked'); await turn();
  expect(earlyDone()).toBe(true);
  // Retired: nothing of it holds anything after, however late its glue attaches.
  const late = Object.assign(audio({ src: 'r.mp4' }, { readyState: 0 }), { currentSrc: 'r.mp4', seeking: false });
  globalThis.exact.removeMedia(late);
  expect(globalThis.exact.mediaHold(late)).toBe(null);
  // Retired (the host's teardown keeps the props and drops the attribute: `removeMedia` lets go), a failure, a stall,
  // a suspended load: each lets go.
  for (const end of [(e) => globalThis.exact.removeMedia(e), (e) => { e.error = { code: 4 }; e.fire('error'); }, (e) => e.fire('stalled'), (e) => { e.readyState = 1; e.networkState = 1; e.fire('suspend'); }]) {
    const m = audio({ src: 'c.mp4' }, { readyState: 0 });
    m.currentSrc = 'c.mp4'; m.seeking = false;
    const h = globalThis.exact.mediaHold(m), d = settledOf(h);
    end(m); await turn();
    expect(d()).toBe(true);
  }
});

test('a source refused before the glue had the element is reported', () => {
  const el = audio({ src: 'app:/data/missing.wav' }, { readyState: 0, error: { code: 4, message: 'MEDIA_ELEMENT_ERROR: Format error' } });
  expect(el.sent).toEqual(['error\nsrc-not-supported']);
});

test('metadata the glue reported on attaching is not reported again by the event HTML had queued', () => {
  const el = audio({ src: 'a.mp3' });
  expect(el.sent).toEqual(['loadedmetadata\n', 'seeked\n']);
  el.fire('loadedmetadata');
  el.fire('seeked');
  expect(el.sent).toEqual(['loadedmetadata\n', 'seeked\n']);
  el.fire('emptied'); // a new load
  el.fire('loadedmetadata');
  el.fire('seeked');
  expect(el.sent).toEqual(['loadedmetadata\n', 'seeked\n', 'loadedmetadata\n', 'seeked\n']);
});

test('a player that already finished loading reports its seek and canplay once', async () => {
  // Boot on the wasm host hears the opening seek (synthetic-media: at 0, seeks 1,
  // ready true, fresh false). Attaching after that load still reports it once,
  // and does not start a second seek by assigning the time the element holds.
  const el = audio({ src: 'a.mp3', currentTime: '0' }, { readyState: 4, handlers: ['loadedmetadata', 'seeking', 'timeupdate', 'seeked', 'canplay'] });
  const once = ['loadedmetadata\n', 'seeking\n', 'timeupdate\n0', 'seeked\n', 'canplay\n'];
  expect(el.seeks).toEqual([]);
  expect(el.sent).toEqual(once);
  el.fire('seeking'); el.fire('timeupdate'); el.fire('seeked'); el.fire('canplay'); el.fire('loadedmetadata');
  expect(el.sent).toEqual(once);
  await new Promise(r => setTimeout(r, 0)); // the queued duplicate was swallowed; a later timeupdate still reports
  el.fire('timeupdate');
  expect(el.sent).toEqual([...once, 'timeupdate\n0']);
});

test('a player with metadata but no data yet makes its opening seek on attaching', () => {
  // The JS target attaches after a frame and a dynamic import; a cached
  // source can have metadata by then (readyState 1), past the
  // `loadedmetadata` the glue seeks at. It seeks now, so the app hears
  // the opening seek as on the wasm host (synthetic-media in a full run).
  const el = audio({ src: 'a.mp3', currentTime: '0' }, { readyState: 1, handlers: ['loadedmetadata', 'seeking', 'seeked'] });
  expect(el.seeks).toEqual([0]);
  expect(el.sent).toEqual(['loadedmetadata\n']);
  el.fire('seeking'); el.fire('seeked');
  expect(el.sent).toEqual(['loadedmetadata\n', 'seeking\n', 'seeked\n']);
  // No bound time: no seek, as the wasm host makes none.
  expect(audio({ src: 'a.mp3' }, { readyState: 1 }).seeks).toEqual([]);
});

test('attaching reports what the element already has, and does not report again only what the host took', () => {
  // The wasm page attaches once its data executor settles: a duration the
  // element reached before then is reported on attaching, and its queued
  // event is not reported again unless the duration changed since.
  const handlers = ['loadedmetadata', 'durationchange', 'error'];
  const loaded = audio({ src: 'a.mp3' }, { readyState: 1, duration: 10, handlers });
  expect(loaded.sent).toEqual(['loadedmetadata\n', 'durationchange\n10']);
  loaded.fire('durationchange');
  expect(loaded.sent).toEqual(['loadedmetadata\n', 'durationchange\n10']);
  const grown = audio({ src: 'a.mp3' }, { readyState: 1, duration: 10, handlers });
  grown.duration = 20; grown.fire('durationchange');
  expect(grown.sent).toEqual(['loadedmetadata\n', 'durationchange\n10', 'durationchange\n20']);
  // A report the host refused leaves its event to be reported when it comes.
  let ready = false;
  const early = audio({ src: 'a.mp3' }, { readyState: 1, duration: 10, handlers, takes: () => ready });
  ready = true; early.fire('durationchange');
  expect(early.sent).toEqual(['durationchange\n10']);
  // A source that failed after its metadata reports its error on attaching.
  const failed = audio({ src: 'a.mp3' }, { readyState: 1, error: { code: 3 }, handlers });
  failed.fire('error'); // its queued event
  expect(failed.sent).toEqual(['loadedmetadata\n', 'error\ndecode']);
});

test('attaching does not rewind a playhead the element moved itself', () => {
  // An autoplaying element can play while the data executor loads; the bound
  // opening time is not assigned over that. One it has not played to still seeks.
  expect(audio({ src: 'a.mp3', currentTime: '0' }, { played: 1, position: 0.5 }).seeks).toEqual([0.5]);
  expect(audio({ src: 'a.mp3', currentTime: '0' }, { position: 0.5 }).seeks).toEqual([0.5, 0]);
  expect(audio({ src: 'a.mp3', currentTime: '5' }, { played: 1, position: 0.5 }).seeks).toEqual([0.5, 5]);
  // Nor when its data ran out after playing (readyState 1, the metadata-only opening seek).
  expect(audio({ src: 'a.mp3', currentTime: '0' }, { readyState: 1, played: 1, position: 0.5 }).seeks).toEqual([0.5]);
});

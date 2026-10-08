// The web hosts' media glue (media-glue.js) over a stand-in element: the
// commands by HTML's method names (podcast F8, F18) and a failure that
// landed before the glue had the element (F19).
import { test, expect } from 'bun:test';

globalThis.exact ??= {};
globalThis.IntersectionObserver ??= class { observe() {} disconnect() {} };
await import('../media-glue.js');

/** An `<audio>` as the glue drives it: seeks, loads and plays are counted. */
function audio(props, { readyState = 4, error = null, handlers = ['error', 'seeked', 'loadedmetadata'], later } = {}) {
  const listeners = {}, el = {
    localName: 'audio', readyState, error, isConnected: true, paused: true, seeks: [], loads: 0, plays: 0, attrs: new Set(),
    exactMedia: { props, handlers }, duration: NaN,
    set currentTime(t) { this.seeks.push(t); }, get currentTime() { return this.seeks.at(-1) ?? 0; },
    addEventListener(name, f) { (listeners[name] ??= []).push(f); },
    toggleAttribute(name, on) { if (on) this.attrs.add(name); else this.attrs.delete(name); },
    load() { this.loads++; this.readyState = 0; this.error = null; this.paused = true; },
    play() { this.plays++; this.paused = false; return Promise.resolve(); },
    pause() { this.paused = true; },
    fire(name) { for (const f of listeners[name] ?? []) f(); },
  };
  const sent = [];
  globalThis.exact.installMedia(el, text => { sent.push(text); el.reply?.(text); }, later);
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

// The wasm page takes reports once its data executor is ready, which may come
// after the opening durationchange (lost under load: duration 0 and remaining
// -0:00 for the session, Video Player).
function waiting(props, handlers = ['durationchange', 'timeupdate', 'loadedmetadata']) {
  let open, ready = false;
  const when = new Promise(r => { open = r; });
  const el = audio(props, { readyState: 0, handlers, later: () => ready ? null : when });
  el.open = async (activated = true) => { ready = activated; open(); await when; await null; };
  return el;
}

test('what never comes again is reported once the host can take it, as the element has it then', async () => {
  const el = waiting({ src: 'a.mp4' }, ['durationchange', 'timeupdate', 'loadedmetadata', 'play', 'canplay', 'error']);
  el.readyState = 1; el.fire('loadedmetadata');
  el.duration = 10; el.fire('durationchange'); el.fire('timeupdate'); el.fire('play');
  el.duration = 12; el.fire('durationchange');
  el.readyState = 4; el.fire('canplay');
  expect(el.sent).toEqual([]);
  await el.open();
  // Once each, as it is now; a time and a play are not resent (they come
  // again, and a stale one could undo a later press or seek).
  expect(el.sent).toEqual(['loadedmetadata\n', 'durationchange\n12', 'canplay\n']);
  el.fire('timeupdate');
  expect(el.sent.at(-1)).toEqual('timeupdate\n0');

  const failed = waiting({ src: 'a.mp4' }, ['error']);
  failed.error = { code: 4 }; failed.fire('error');
  await failed.open();
  expect(failed.sent).toEqual(['error\nsrc-not-supported']);
});

test('nothing is reported for an element retired, or replaced by a load, before the host could take it', async () => {
  const removed = waiting({ src: 'a.mp4' });
  removed.duration = 10; removed.fire('durationchange'); globalThis.exact.removeMedia(removed);
  await removed.open();
  expect(removed.sent).toEqual([]);

  // The host's write lands in `props` before the glue applies it (an `app:/`
  // source resolves later still): the new source reports its own.
  const replaced = waiting({ src: 'a.mp4' });
  replaced.duration = 10; replaced.fire('durationchange');
  replaced.exactMedia.props.src = 'app:/data/b.mp4'; globalThis.exact.installMedia(replaced);
  await replaced.open();
  expect(replaced.sent).toEqual([]);

  // A's facts, then A empties and B has its metadata before its events come.
  const emptied = waiting({ src: 'a.mp4' });
  emptied.duration = 10; emptied.fire('durationchange'); emptied.fire('emptied'); emptied.duration = 20;
  await emptied.open();
  expect(emptied.sent).toEqual([]);

  // A report's own handler asks for a load: what follows it is not sent.
  const loading = waiting({ src: 'a.mp4' });
  loading.readyState = 1; loading.fire('loadedmetadata'); loading.duration = 10; loading.fire('durationchange');
  loading.reply = text => { if (text.startsWith('loadedmetadata')) (loading.exactMedia.commands ??= []).push(['load']); };
  await loading.open();
  expect(loading.sent).toEqual(['loadedmetadata\n']);
});

test('a host that never becomes ready is not waited on again', async () => {
  const el = waiting({ src: 'a.mp4' });
  el.duration = 10; el.fire('durationchange');
  await el.open(false); // the promise settles, the data executor never came
  for (let i = 0; i < 5; i++) await null;
  expect(el.sent).toEqual([]);
});

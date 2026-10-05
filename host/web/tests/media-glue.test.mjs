// The web hosts' media glue (media-glue.js) over a stand-in element: the
// commands by HTML's method names (podcast F8, F18) and a failure that
// landed before the glue had the element (F19).
import { test, expect } from 'bun:test';

globalThis.exact ??= {};
globalThis.IntersectionObserver ??= class { observe() {} disconnect() {} };
await import('../media-glue.js');

/** An `<audio>` as the glue drives it: seeks, loads and plays are counted. */
function audio(props, { readyState = 4, error = null } = {}) {
  const listeners = {}, el = {
    localName: 'audio', readyState, error, isConnected: true, paused: true, seeks: [], loads: 0, plays: 0, attrs: new Set(),
    exactMedia: { props, handlers: ['error', 'seeked', 'loadedmetadata'] }, duration: NaN,
    set currentTime(t) { this.seeks.push(t); }, get currentTime() { return this.seeks.at(-1) ?? 0; },
    addEventListener(name, f) { (listeners[name] ??= []).push(f); },
    toggleAttribute(name, on) { if (on) this.attrs.add(name); else this.attrs.delete(name); },
    load() { this.loads++; this.readyState = 0; this.error = null; this.paused = true; },
    play() { this.plays++; this.paused = false; return Promise.resolve(); },
    pause() { this.paused = true; },
    fire(name) { for (const f of listeners[name] ?? []) f(); },
  };
  const sent = [];
  globalThis.exact.installMedia(el, text => sent.push(text));
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
  expect(el.sent).toEqual(['loadedmetadata\n']);
  el.fire('loadedmetadata');
  expect(el.sent).toEqual(['loadedmetadata\n']);
  el.fire('emptied'); // a new load
  el.fire('loadedmetadata');
  expect(el.sent).toEqual(['loadedmetadata\n', 'loadedmetadata\n']);
});

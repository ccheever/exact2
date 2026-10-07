// The JS target's media hold (host/web-js/media.js). The wasm host does not
// count a media element that never becomes playable. A element with no
// source, or one that stalls or empties, must release `inflight`. An opening
// seek still holds until it finishes. The DOM and the glue are stand-ins.
import { test, expect } from 'bun:test';
import { copyFileSync, mkdtempSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { resolve } from 'node:path';

const dir = mkdtempSync(resolve(tmpdir(), 'exact-js-media-hold-'));
copyFileSync(resolve(new URL('../../web-js/media.js', import.meta.url).pathname), resolve(dir, 'media.js'));
writeFileSync(resolve(dir, 'rt.js'), 'export const { onEnd, inflight, journal, data, clock } = globalThis.rtStandIn;\n');
writeFileSync(resolve(dir, 'media-glue.js'), 'globalThis.exact ??= {}; globalThis.exact.installMedia = () => {};\n');
const inflight = { n: 0 };
globalThis.rtStandIn = { onEnd() {}, inflight, journal: [], data: { appId: 'com.example' }, clock: { now: 0, timers: [], agent: false, epoch: 0 } };
globalThis.requestAnimationFrame = (fn) => { queueMicrotask(() => fn(0)); return 1; };

function element(fields) {
  const listeners = new Map();
  const attrs = new Map();
  return {
    isConnected: true,
    readyState: 0,
    seeking: false,
    error: null,
    currentSrc: '',
    ...fields,
    getAttribute: (k) => attrs.get(k) ?? null,
    setAttribute: (k, v) => attrs.set(k, String(v)),
    removeAttribute: (k) => attrs.delete(k),
    pause() {},
    load() {},
    dispatchEvent() {},
    addEventListener(name, fn) {
      const list = listeners.get(name) ?? [];
      list.push(fn);
      listeners.set(name, list);
    },
    removeEventListener(name, fn) {
      listeners.set(name, (listeners.get(name) ?? []).filter((f) => f !== fn));
    },
    fire(type) {
      for (const fn of [...(listeners.get(type) ?? [])]) fn({ type });
    },
  };
}

async function flushed() {
  for (let i = 0; i < 40 && typeof globalThis.exact?.installMedia !== 'function'; i++) await new Promise((r) => setTimeout(r, 1));
  await new Promise((r) => setTimeout(r, 0));
  await new Promise((r) => setTimeout(r, 0));
}

const { media } = await import(resolve(dir, 'media.js'));

test('a media element with no source does not hold inflight', async () => {
  const start = inflight.n;
  media(element(), {});
  await flushed();
  expect(inflight.n).toBe(start);
});

test('stalled and emptied release a load that never becomes playable', async () => {
  const stalled = element();
  const before = inflight.n;
  media(stalled, { src: 'clip.mp4' });
  await flushed();
  expect(inflight.n).toBe(before + 1);
  stalled.fire('stalled');
  expect(inflight.n).toBe(before);

  const emptied = element();
  media(emptied, { src: 'other.mp4' });
  await flushed();
  expect(inflight.n).toBe(before + 1);
  emptied.fire('emptied');
  expect(inflight.n).toBe(before);
});

test('an opening seek holds until it finishes and a playable element does not', async () => {
  const seeking = element({ seeking: true, readyState: 1 });
  const before = inflight.n;
  media(seeking, { src: 'seek.mp4' });
  await flushed();
  expect(inflight.n).toBe(before + 1);
  seeking.fire('seeked');
  expect(inflight.n).toBe(before + 1);
  seeking.readyState = 4;
  seeking.seeking = false;
  seeking.fire('seeked');
  expect(inflight.n).toBe(before);

  const ready = element({ readyState: 4, seeking: false });
  media(ready, { src: 'ready.mp4' });
  await flushed();
  expect(inflight.n).toBe(before);
});

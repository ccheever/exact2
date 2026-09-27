import { test, expect } from 'bun:test';
import { readFileSync } from 'node:fs';
import { runInNewContext } from 'node:vm';

const source = readFileSync(new URL('./native-glue.js', import.meta.url), 'utf8');
function host(table) {
  const scripts = [], timers = new Map(), listeners = new Set();
  const exact = {}, context = {
    exact, URL, TextDecoder, TextEncoder, Uint8Array, atob, console, queueMicrotask, performance,
    document: { baseURI: 'https://example.test/', createElement: () => ({ remove() {} }), head: { append: script => scripts.push(script) } },
    addEventListener: (_, listener) => listeners.add(listener), removeEventListener: (_, listener) => listeners.delete(listener),
    setTimeout: callback => { const id = timers.size + 1; timers.set(id, callback); return id; }, clearTimeout: id => timers.delete(id),
  };
  if (table) exact.nativeArtifact = Promise.resolve(table);
  runInNewContext(source, context);
  return { exact, scripts, timers, listeners };
}

test('an announcement-only page module connects without a native.later export', async () => {
  const heard = [];
  const h = host({ connect({ changed }) { changed('meter'); } });
  const page = await h.exact.pageNative(true, { changed: topic => heard.push(topic), ready: () => true, generation: () => 1 });
  page.drain();
  expect(heard).toEqual(['meter']);
  await expect(page.later(btoa('{}'))).rejects.toThrow('exports no later');
});

test('connect receives the agent flag and the page clock (LLP 1067.000 Q7)', async () => {
  let seen = null;
  const h = host({ connect(context) { seen = context; } });
  await h.exact.pageNative(true, { changed() {}, ready: () => true, generation: () => 1, agent: true, now: () => 1234 });
  expect(seen.agent).toBe(true);
  expect(seen.now()).toBe(1234);
  const plain = host({ connect(context) { seen = context; } });
  await plain.exact.pageNative(true, { changed() {}, ready: () => true, generation: () => 1 });
  expect(seen.agent).toBe(false);
  expect(typeof seen.now()).toBe('number');
});

test('a rejected or timed-out artifact can load again and cleans up its load hooks', async () => {
  for (const failure of ['error', 'timeout']) {
    const h = host();
    const first = h.exact.pageNative(true, { changed() {}, ready: () => true, generation: () => 1 });
    if (failure === 'error') h.scripts[0].onerror();
    else [...h.timers.values()][0]();
    await expect(first).rejects.toThrow('did not load');
    expect(h.exact.nativeArtifact).toBeNull();
    expect(h.listeners.size).toBe(0);
    expect(h.timers.size).toBe(0);
    const second = h.exact.pageNative(true, { changed() {}, ready: () => true, generation: () => 1 });
    expect(h.scripts.length).toBe(2);
    h.exact.nativeTable(1, { later: () => ({ ok: true }) });
    expect(await (await second).later(btoa('{}'))).toBe('{"ok":true}');
  }
});


test('announcements coalesce until the host drains and retired generations cannot re-ask', async () => {
  let announce, ready = false, generation = 1;
  const heard = [];
  const h = host({ connect({ changed }) { announce = changed; } });
  const page = await h.exact.pageNative(true, { changed: topic => heard.push(topic), ready: () => ready, generation: () => generation });
  announce('meter'); announce('meter'); announce('connectivity');
  expect(heard).toEqual([]);
  expect(h.timers.size).toBe(1);
  page.drain();
  expect(heard).toEqual([]);
  ready = true; page.drain();
  expect(heard).toEqual(['meter', 'connectivity']);
  announce('retired'); generation++;
  announce('current'); page.drain();
  expect(heard).toEqual(['meter', 'connectivity', 'current']);
});

test('the page loader clears its rejected cache and activation connects without a request', async () => {
  const glue = readFileSync(new URL('./glue.js', import.meta.url), 'utf8');
  const loaderSource = glue.slice(glue.indexOf('let pageNativeModule'), glue.indexOf('let rustLoader'));
  let attempts = 0;
  const native = { drain() {} };
  const load = Function('loadAfterPaint', 'pageNative', 'inputReady', 'incarnation', 'afterNativePaint', 'agentMode', 'now', `${loaderSource}; return loadPageNative;`)(
    async () => { if (++attempts === 1) throw Error('transient load failure'); return async () => native; }, true, true, 1, async () => {}, false, () => 0);
  await expect(load()).rejects.toThrow('transient load failure');
  expect(await load()).toBe(native);
  expect(await load()).toBe(native);
  expect(attempts).toBe(2);
  const activationSource = glue.slice(glue.indexOf('function activateData()'), glue.indexOf('// Boot the app'));
  const calls = [];
  const activate = Function('wasm', 'readOut', 'page', 'applyBatch', 'setInputReady', 'collections', 'root', 'pageNative', 'loadPageNative', 'pieces', 'log',
    `${activationSource}; return activateData;`)(
      { exact_data_ready: () => 0 }, () => '{}', null, () => {}, () => calls.push('ready'), { dataReady() {} }, { dataset: {} }, true,
      async () => { calls.push('connect'); return { drain: () => calls.push('drain') }; }, { pending: () => null }, () => {});
  await activate();
  expect(calls).toEqual(['ready', 'connect', 'drain']);
});

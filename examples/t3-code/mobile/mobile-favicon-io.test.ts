import { expect, test } from 'bun:test';
import { withMobileFaviconIO } from './mobile-favicon-io';
import { type MobileFaviconIO } from './mobile-favicon-cache';
import { type Native } from './shared/protocol';

const dataUrl = 'data:image/png;base64,YQ==';
const ok = (value: unknown = {}) => ({ ok: true, generation: 0, value });
const key = { environmentId: 'environment', kind: 'project-favicon' as const, key: 'resource' };
function mock(perform: (request: any) => unknown) {
  const calls: any[] = [];
  const native: Native = { available: true, watch() {}, async later(request) { calls.push(request); return perform(request); } };
  return { native, calls };
}
function deferred<T>() {
  let resolve!: (value: T) => void, reject!: (reason: unknown) => void;
  const promise = new Promise<T>((yes, no) => { resolve = yes; reject = no; });
  return { promise, resolve, reject };
}
test('favicon IO delegates bounded global cache operations and scoped clearing', async () => {
  const { native, calls } = mock(request => ok(request.action === 'list' ? { rows: [] }
    : request.action === 'read' ? { record: null } : request.action === 'ticket' ? { ticket: 'ticket' }
    : request.action === 'write' ? { written: true, stale: false } : { removed: 0 }));
  await withMobileFaviconIO(native, async io => {
    expect(await io.list(null, 128)).toEqual([]);
    expect(await io.read(key)).toBeNull();
    expect(await io.ticket(key)).toBe('ticket');
    expect(await io.write(key, 'ticket', '{}')).toBe(true);
    await io.remove(key, '{}'); await io.clear('environment'); await io.clear();
  });
  expect(calls.map(row => row.action)).toEqual(['list', 'read', 'ticket', 'write', 'remove', 'clear', 'clearKind']);
  expect(calls[0]).toEqual({ op: 'mobileClientCache', action: 'list', kind: 'project-favicon', after: null, limit: 128 });
  expect(calls[4].expectedPayload).toBe('{}');
  expect(calls[5]).toEqual({ op: 'mobileClientCache', action: 'clear', kind: 'project-favicon', environmentId: 'environment' });
  expect(calls[6]).toEqual({ op: 'mobileClientCache', action: 'clearKind', kind: 'project-favicon' });
});
test('loads use unique request IDs and return only bounded supported image data', async () => {
  const { native, calls } = mock(() => ok({ dataUrl }));
  for (let index = 0; index < 2; index++) await withMobileFaviconIO(native, async io => {
    expect(await io.load('https://image.invalid/a.png?token=private', new AbortController().signal)).toBe(dataUrl);
  });
  expect(calls[0].requestId).not.toBe(calls[1].requestId);
  expect(Object.keys(calls[0]).sort()).toEqual(['action', 'op', 'requestId', 'url']);
  for (const invalid of ['', 'https://remote.invalid/a.png', 'data:text/html;base64,YQ==', dataUrl + 'A'.repeat(32768)]) {
    const fixture = mock(() => ok({ dataUrl: invalid }));
    await expect(withMobileFaviconIO(fixture.native, io => io.load('https://image.invalid', new AbortController().signal)))
      .rejects.toThrow('invalid image data');
  }
});
test('abort before admission performs no native work', async () => {
  const { native, calls } = mock(() => ok({ dataUrl })), controller = new AbortController(); controller.abort();
  await expect(withMobileFaviconIO(native, io => io.load('https://image.invalid', controller.signal))).rejects.toMatchObject({ kind: 'Cancelled' });
  expect(calls).toHaveLength(0);
});
test('abort cancels the exact request, awaits its acknowledgment, and removes the listener', async () => {
  const load = deferred<unknown>(), cancel = deferred<unknown>(), controller = new AbortController();
  const { native, calls } = mock(request => request.action === 'load' ? load.promise : cancel.promise);
  let finished = false;
  const answer = withMobileFaviconIO(native, io => io.load('https://image.invalid', controller.signal));
  const outcome = answer.then(() => { finished = true; }, error => { finished = true; return error; });
  controller.abort(); controller.abort();
  expect(calls.map(row => row.action)).toEqual(['load', 'cancel']);
  expect(calls[0].requestId).toBe(calls[1].requestId);
  load.resolve(ok({ dataUrl })); await Promise.resolve(); await Promise.resolve();
  expect(finished).toBe(false);
  cancel.resolve(ok()); expect(await outcome).toMatchObject({ kind: 'Cancelled' });
  expect(calls).toHaveLength(2);
});
test('lost answer propagates and refuses later cache or abort calls', async () => {
  const controller = new AbortController();
  const { native, calls } = mock(() => { throw { name: 'FetchError', kind: 'Aborted' }; });
  await withMobileFaviconIO(native, async io => {
    await expect(io.load('https://image.invalid', controller.signal)).rejects.toMatchObject({ kind: 'superseded' });
    controller.abort();
    await expect(io.ticket(key)).rejects.toMatchObject({ kind: 'superseded' });
  });
  expect(calls).toHaveLength(1);
});
test('a lost cancellation acknowledgment propagates even if image finished successfully', async () => {
  const load = deferred<unknown>(), controller = new AbortController();
  const { native } = mock(request => {
    if (request.action === 'load') return load.promise;
    throw { name: 'FetchError', kind: 'Aborted' };
  });
  const answer = withMobileFaviconIO(native, io => io.load('https://image.invalid', controller.signal));
  controller.abort(); load.resolve(ok({ dataUrl }));
  await expect(answer).rejects.toMatchObject({ kind: 'superseded' });
});
test('completed and failed brackets refuse an escaped adapter and detach abort listeners', async () => {
  for (const fail of [false, true]) {
    const { native, calls } = mock(() => ok({ dataUrl })), controller = new AbortController();
    let escaped!: MobileFaviconIO;
    const answer = withMobileFaviconIO(native, async io => {
      escaped = io; await io.load('https://image.invalid', controller.signal);
      if (fail) throw new Error('caller failed');
    });
    if (fail) await expect(answer).rejects.toThrow('caller failed'); else await answer;
    controller.abort();
    expect(() => escaped.ticket(key)).toThrow('superseded');
    expect(calls).toHaveLength(1);
  }
});

import { mobileFaviconCatalogIO } from './mobile-favicon-io';
import { MobileFaviconCache, mobileFaviconResourceKey, mobileFaviconRevision } from './mobile-favicon-cache';
import { mobileCacheCompareMetadata, type MobileCacheRecord } from './mobile-client-cache';

function provenanceFixture() {
  const rows = new Map<string, MobileCacheRecord>(), removed: (string | undefined)[] = [];
  const identities = new Map([['env', '["env","https://home.invalid"]']]);
  let clock = 0, revision = 0, alive = true;
  const keyOf = (key: { kind: string; key: string }) => `${key.kind}:${key.key}`;
  const base: MobileFaviconIO = {
    scopeRevision: () => `${revision}`,
    async list(after, limit) { return [...rows.values()].filter(row => row.kind === 'project-favicon').sort(mobileCacheCompareMetadata)
      .filter(row => !after || mobileCacheCompareMetadata(row, after) > 0).slice(0, limit).map(({ payload: _payload, ...row }) => row); },
    async read(key) { return rows.get(keyOf(key)) ?? null; },
    async ticket() { return `${revision}`; },
    async write(key, ticket, payload) { if (ticket !== `${revision}`) return false;
      rows.set(keyOf(key), { ...key, schemaVersion: 1, payload, updatedAt: ++clock }); return true; },
    async remove(key, payload) { removed.push(payload); if (payload === undefined || rows.get(keyOf(key))?.payload === payload) rows.delete(keyOf(key)); },
    async clear() { revision++; rows.clear(); }, async load() { return dataUrl; },
  };
  const io = mobileFaviconCatalogIO(base, { identity: env => identities.get(env) ?? '', current: () => alive });
  const target = { environmentId: 'env', cwd: '/work' }, key = { environmentId: 'env', kind: 'project-favicon' as const, key: mobileFaviconResourceKey(target) };
  const raw = JSON.stringify({ ...target, faviconPath: null, revision: mobileFaviconRevision(target, '/icon.png'), dataUrl });
  return { rows, identities, removed, base, io, target, key, raw, keyOf, stop: () => { alive = false; } };
}

test('catalog augmentation preserves schema1 and exact conditional cleanup for new and hydrated records', async () => {
  const f = provenanceFixture();
  await f.io.write(f.key, await f.io.ticket(f.key), f.raw);
  const written = f.rows.get(f.keyOf(f.key))!;
  expect(written.schemaVersion).toBe(1); expect(JSON.parse(written.payload)).toEqual({ ...JSON.parse(f.raw), catalogIdentity: f.identities.get('env') });
  expect((await f.io.read(f.key))?.payload).toBe(written.payload);
  await f.io.remove(f.key, f.raw); expect(f.removed[0]).toBe(written.payload); expect(f.rows.size).toBe(0);
  await f.io.write(f.key, await f.io.ticket(f.key), f.raw);
  await f.io.remove(f.key, written.payload); expect(f.removed[1]).toBe(written.payload); expect(f.rows.size).toBe(0);
});

test.each(['missing', 'replaced', 'forgotten', 'corrupt'])('cold hydration rejects %s provenance only for the affected favicon row', async kind => {
  const f = provenanceFixture(), cache = new MobileFaviconCache();
  const payload = kind === 'corrupt' ? '{broken' : kind === 'missing' ? f.raw
    : JSON.stringify({ ...JSON.parse(f.raw), catalogIdentity: '["env","https://old.invalid"]' });
  f.rows.set(f.keyOf(f.key), { ...f.key, schemaVersion: 1, payload, updatedAt: 1 });
  const unrelated: MobileCacheRecord = { environmentId: 'env', kind: 'shell', key: 'snapshot', schemaVersion: 1, payload: '{"environmentId":"env"}', updatedAt: 2 };
  f.rows.set(f.keyOf(unrelated), unrelated);
  if (kind === 'forgotten') f.identities.delete('env');
  expect(await cache.hydrate(f.io, () => true)).toBe(true);
  expect(cache.peek(f.target, f.io)).toBeNull(); expect(f.rows.size).toBe(1);
  expect(f.rows.get(f.keyOf(unrelated))).toBe(unrelated); expect(f.removed).toEqual([payload]);
});

test('global128-entry eviction deletes freshly augmented rows rather than leaking conditional mismatches', async () => {
  const f = provenanceFixture(), cache = new MobileFaviconCache(); await cache.hydrate(f.io, () => true);
  for (let index = 0; index < 129; index++) await cache.resolve(f.io, { ...f.target, cwd: `/work/${index}` }, `/icon-${index}.png`,
    { current: () => true, signal: new AbortController().signal });
  expect(f.rows.size).toBe(128); expect(f.removed).toHaveLength(1);
  expect(JSON.parse(f.removed[0]!).catalogIdentity).toBe(f.identities.get('env'));
  const restarted = new MobileFaviconCache(); expect(await restarted.hydrate(f.io, () => true)).toBe(true);
  expect(restarted.peek({ ...f.target, cwd: '/work/128' }, f.io)).toBe(dataUrl);
});

test('matching disabled saved provenance may hydrate, but memory disappears as soon as catalog identity changes', async () => {
  const f = provenanceFixture(), cache = new MobileFaviconCache();
  await f.io.write(f.key, await f.io.ticket(f.key), f.raw);
  await cache.hydrate(f.io, () => true); expect(cache.peek(f.target, f.io)).toBe(dataUrl);
  f.identities.set('env', '["env","https://replacement.invalid"]');
  expect(cache.peek(f.target, f.io)).toBeNull();
  expect(await f.io.read(f.key)).toBeNull(); expect(f.rows.size).toBe(0);
});

test('a catalog replacement during disk read cannot return or remove a newer row', async () => {
  const f = provenanceFixture(); await f.io.write(f.key, await f.io.ticket(f.key), f.raw);
  const entered = Promise.withResolvers<void>(), release = Promise.withResolvers<void>(), read = f.base.read;
  f.base.read = async key => { const value = await read(key); entered.resolve(); await release.promise; return value; };
  const pending = f.io.read(f.key); await entered.promise;
  f.identities.set('env', '["env","https://new.invalid"]'); await f.io.write(f.key, await f.io.ticket(f.key), f.raw);
  release.resolve(); expect(await pending).toBeNull(); expect(f.rows.size).toBe(1); expect(f.removed).toHaveLength(0);
});

test('catalog decorator rejects invalid entry identity and refuses work after its answer ends', async () => {
  const f = provenanceFixture();
  await expect(f.io.write(f.key, '0', JSON.stringify({ ...JSON.parse(f.raw), environmentId: 'other' }))).rejects.toThrow('Invalid cached favicon');
  f.stop(); await expect(f.io.list(null, 128)).rejects.toMatchObject({ kind: 'superseded' });
  expect(f.rows.size).toBe(0);
});

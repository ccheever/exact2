import { describe, expect, test } from 'bun:test';
import { ClientError } from './shared/protocol';
import type { MobileCacheKey, MobileCacheRecord } from './mobile-client-cache';
import {
  MobileFaviconCache, decodeMobileFaviconEntry, mobileFaviconDataUrl, mobileFaviconMissing,
  mobileFaviconResourceKey, mobileFaviconRevision, type MobileFaviconEntry,
  type MobileFaviconIO, type MobileFaviconMetadata, type MobileFaviconTarget,
} from './mobile-favicon-cache';

const png = 'data:image/png;base64,QQ==';
const target = (cwd = '/project', environmentId = 'env'): MobileFaviconTarget => ({ environmentId, cwd });
const entry = (t = target(), dataUrl = png, url = '/old.png'): MobileFaviconEntry =>
  ({ ...t, faviconPath: t.faviconPath || null, revision: mobileFaviconRevision(t, url), dataUrl });
const owned = () => true;
const options = () => ({ current: owned, signal: new AbortController().signal });
function deferred<T>() {
  let resolve!: (value: T) => void, reject!: (error: unknown) => void;
  const promise = new Promise<T>((yes, no) => { resolve = yes; reject = no; });
  return { promise, resolve, reject };
}
function memory() {
  const rows = new Map<string, MobileCacheRecord>(), revisions = new Map<string, number>();
  const events: string[] = [], keyEpochs = new Map<string, number>();
  let global = 0, tick = 0;
  const identity = (key: MobileCacheKey) => JSON.stringify([key.environmentId, key.key]);
  const scopeRevision = (id: string) => `${global}:${revisions.get(id) ?? 0}`;
  const keyFor = (e: MobileFaviconEntry): MobileCacheKey =>
    ({ environmentId: e.environmentId, kind: 'project-favicon', key: mobileFaviconResourceKey(e) });
  const seed = (e: MobileFaviconEntry, updatedAt = ++tick) => {
    const key = keyFor(e), row = { ...key, schemaVersion: 1, payload: JSON.stringify(e), updatedAt };
    rows.set(identity(key), row); tick = Math.max(tick, updatedAt); return row;
  };
  const compare = (a: MobileFaviconMetadata, b: MobileFaviconMetadata) => a.updatedAt - b.updatedAt
    || (a.environmentId < b.environmentId ? -1 : a.environmentId > b.environmentId ? 1 : 0)
    || (a.key < b.key ? -1 : a.key > b.key ? 1 : 0);
  const io: MobileFaviconIO = {
    scopeRevision,
    async list(after, limit) {
      events.push('list');
      return [...rows.values()].sort(compare).filter(row => !after || compare(row, after) > 0).slice(0, limit)
        .map(({ payload: _payload, ...row }) => row);
    },
    async read(key) { events.push('read'); return rows.get(identity(key)) ?? null; },
    async ticket(key) { events.push('ticket'); return `${scopeRevision(key.environmentId)}:${keyEpochs.get(identity(key)) ?? 0}`; },
    async write(key, ticket, payload) {
      events.push('write');
      if (ticket !== `${scopeRevision(key.environmentId)}:${keyEpochs.get(identity(key)) ?? 0}`) return false;
      rows.set(identity(key), { ...key, schemaVersion: 1, payload, updatedAt: ++tick }); return true;
    },
    async remove(key, expectedPayload) {
      events.push('remove');
      if (expectedPayload === undefined) {
        keyEpochs.set(identity(key), (keyEpochs.get(identity(key)) ?? 0) + 1);
        revisions.set(key.environmentId, (revisions.get(key.environmentId) ?? 0) + 1);
        rows.delete(identity(key));
      } else if (rows.get(identity(key))?.payload === expectedPayload) rows.delete(identity(key));
    },
    async clear(environmentId) {
      events.push('clear');
      if (environmentId === undefined) { global++; revisions.clear(); }
      else revisions.set(environmentId, (revisions.get(environmentId) ?? 0) + 1);
      for (const [key, row] of rows) if (environmentId === undefined || row.environmentId === environmentId) rows.delete(key);
    },
    async load() { events.push('load'); return png; },
  };
  return { rows, io, events, seed, keyFor, scopeRevision };
}

// Explicit source vectors; the decoder accepts alphabet/length, not image bytes.
describe('mobile favicon source identity and decoder', () => {
  test('keeps Unicode, whitespace and empty paths exactly; empty and null path share a resource', () => {
    expect(mobileFaviconResourceKey({ environmentId: 'env', cwd: ' /中文 ', faviconPath: '' })).toBe('["env"," /中文 ",null]');
    expect(mobileFaviconResourceKey({ environmentId: 'env', cwd: '', faviconPath: ' ' })).toBe('["env",""," "]');
    expect(mobileFaviconResourceKey(target())).toBe(mobileFaviconResourceKey({ ...target(), faviconPath: null }));
  });
  test('query rotation and URL roots do not alter filename revision', () => {
    expect(mobileFaviconRevision(target(), 'https://a.test/icon.png?token=old')).toBe('["env","/project","icon.png"]');
    expect(mobileFaviconRevision(target(), 'https://b.test/a/icon.png?token=new')).toBe(mobileFaviconRevision(target(), '/icon.png'));
    expect(mobileFaviconRevision(target(), 'http://[')).toBe('["env","/project","http://["]');
    expect(mobileFaviconRevision(target(), '/icon/')).toBe('["env","/project",""]');
  });
  test('only exact final missing marker signifies absence', () => {
    expect(mobileFaviconMissing('/icons/project-favicon-missing?token=x')).toBe(true);
    for (const url of ['/project-favicon-missing.png', '/project-favicon-missing/', '/project-favicon-missing/x', 'http://[']) {
      expect(mobileFaviconMissing(url)).toBe(false);
    }
  });
  test('schema trims only environment, permits empty strings and strips unknown keys', () => {
    expect(decodeMobileFaviconEntry({ ...entry(), environmentId: ' env ', cwd: '', faviconPath: '', revision: '', extra: 1 }))
      .toEqual({ environmentId: 'env', cwd: '', faviconPath: '', revision: '', dataUrl: png });
    for (const patch of [{ environmentId: ' ' }, { cwd: null }, { faviconPath: undefined }, { revision: 1 }, { dataUrl: '' }]) {
      expect(decodeMobileFaviconEntry({ ...entry(), ...patch })).toBeNull();
    }
  });
  test('matches source MIME/alphabet/32768 length without pretending to decode images', () => {
    for (const mime of ['png', 'jpeg', 'gif', 'webp', 'avif', 'svg+xml', 'x-icon', 'vnd.microsoft.icon']) {
      expect(mobileFaviconDataUrl(`data:image/${mime};base64,A==`)).toBe(true);
    }
    for (const value of ['', 'data:image/png;base64,', 'data:image/jpg;base64,QQ==', 'data:image/png;base64,Q Q=',
      'data:image/png;base64,QQ===', 'data:image/png;base64,QQ-_', 'data:image/PNG;base64,QQ==']) expect(mobileFaviconDataUrl(value)).toBe(false);
    const header = 'data:image/png;base64,';
    expect(mobileFaviconDataUrl(header + 'A'.repeat(32768 - header.length))).toBe(true);
    expect(mobileFaviconDataUrl(header + 'A'.repeat(32769 - header.length))).toBe(false);
  });
});

describe('mobile favicon caller-owned persistence', () => {
  test('requires explicit hydration; admits no load/write while hydration is pending', async () => {
    const m = memory(), cache = new MobileFaviconCache(), page = deferred<MobileFaviconMetadata[]>();
    m.io.list = () => page.promise;
    const work = cache.hydrate(m.io, owned);
    expect(await cache.hydrate(m.io, owned)).toBe(false);
    expect(await cache.resolve(m.io, target(), '/new.png', options())).toBe('/new.png');
    expect(m.events).toEqual([]);
    page.resolve([]); expect(await work).toBe(true);
  });
  test('takes native ticket before download, persists normalized path and only inline bytes', async () => {
    const m = memory(), cache = new MobileFaviconCache();
    await cache.hydrate(m.io, owned); m.events.length = 0;
    expect(await cache.resolve(m.io, { ...target(), faviconPath: '' }, '/new.png?secret=token', options())).toBe(png);
    expect(m.events).toEqual(['ticket', 'load', 'write']);
    const saved = JSON.parse([...m.rows.values()][0]!.payload);
    expect(saved).toEqual(entry(target(), png, '/new.png'));
    expect(JSON.stringify(saved)).not.toContain('secret');
  });
  test('null, aborted and peek do not promote; matching nonnull URL promotes only memory', async () => {
    const m = memory(), cache = new MobileFaviconCache();
    for (let i = 0; i < 128; i++) m.seed(entry(target(`/p${i}`)));
    await cache.hydrate(m.io, owned);
    expect(cache.peek(target('/p0'), m.io)).toBe(png);
    await cache.resolve(m.io, target('/p0'), null, options());
    const signal = new AbortController(); signal.abort();
    await cache.resolve(m.io, target('/p0'), '/old.png', { current: owned, signal: signal.signal });
    await cache.resolve(m.io, target('/p1'), '/old.png?renewed=1', options());
    await cache.resolve(m.io, target('/new'), '/new.png', options());
    expect(cache.peek(target('/p0'), m.io)).toBeNull();
    expect(cache.peek(target('/p1'), m.io)).toBe(png);
    expect([...m.rows.values()].find(row => row.key === mobileFaviconResourceKey(target('/p1')))?.updatedAt).toBe(2);
  });
  test('hydrates and trims globally across pages without OFFSET skips; restart order uses updatedAt', async () => {
    const m = memory(), cache = new MobileFaviconCache();
    for (let i = 0; i < 300; i++) m.seed(entry(target(`/p${i}`, i % 2 ? 'a' : 'b')));
    expect(await cache.hydrate(m.io, owned)).toBe(true);
    expect(m.events.filter(x => x === 'read')).toHaveLength(300);
    expect(m.rows.size).toBe(128);
    expect(cache.peek(target('/p171', 'a'), m.io)).toBeNull();
    expect(cache.peek(target('/p172', 'b'), m.io)).toBe(png);
    await cache.resolve(m.io, target('/p172', 'b'), '/old.png', options());
    const restarted = new MobileFaviconCache(); await restarted.hydrate(m.io, owned);
    await restarted.resolve(m.io, target('/new'), '/new.png', options());
    expect(restarted.peek(target('/p172', 'b'), m.io)).toBeNull();
    expect(restarted.peek(target('/p173', 'a'), m.io)).toBe(png);
  });
  test('1MiB limit counts only image data and includes mixed environments at exact boundary', async () => {
    const m = memory(), cache = new MobileFaviconCache(), header = 'data:image/png;base64,';
    const large = header + 'A'.repeat(32768 - header.length);
    for (let i = 0; i < 33; i++) m.seed(entry(target(`/p${i}`, i % 2 ? 'a' : 'b'), large));
    await cache.hydrate(m.io, owned);
    expect(m.rows.size).toBe(32);
    expect(cache.peek(target('/p0', 'b'), m.io)).toBeNull();
    expect(cache.peek(target('/p1', 'a'), m.io)).toBe(large);
    expect([...m.rows.values()].reduce((bytes, row) => bytes + JSON.parse(row.payload).dataUrl.length, 0)).toBe(1048576);
  });
  test('corrupt records are removed conditionally; concurrent replacement survives', async () => {
    const m = memory(), cache = new MobileFaviconCache(), original = m.seed(entry());
    original.payload = '{broken';
    const remove = m.io.remove;
    m.io.remove = async (key, expected) => { m.seed(entry(target(), png, '/replacement.png')); await remove(key, expected); };
    expect(await cache.hydrate(m.io, owned)).toBe(true);
    expect(cache.peek(target(), m.io)).toBeNull();
    expect(m.rows.size).toBe(1);
    expect(JSON.parse([...m.rows.values()][0]!.payload).revision).toContain('replacement.png');
  });
  test('wrong schema, environment and resource key are not installed', async () => {
    for (const corrupt of [(row: MobileCacheRecord) => { row.schemaVersion = 9; },
      (row: MobileCacheRecord) => { row.payload = JSON.stringify(entry(target('/other'))); },
      (row: MobileCacheRecord) => { row.payload = JSON.stringify(entry(target('/project', 'other'))); }]) {
      const m = memory(), cache = new MobileFaviconCache(); corrupt(m.seed(entry()));
      expect(await cache.hydrate(m.io, owned)).toBe(true); expect(m.rows.size).toBe(0);
      expect(cache.peek(target(), m.io)).toBeNull();
    }
  });
  test('missing marker removes an older cached image while null URL retains it', async () => {
    const m = memory(), cache = new MobileFaviconCache(); m.seed(entry()); await cache.hydrate(m.io, owned);
    expect(await cache.resolve(m.io, target(), null, options())).toBe(png);
    expect(await cache.resolve(m.io, target(), '/project-favicon-missing', options())).toBeNull();
    expect(m.rows.size).toBe(0); expect(m.events).not.toContain('load');
  });
  test('failed or invalid download preserves cached image, otherwise uses current remote URL', async () => {
    for (const fail of [async () => { throw new Error('offline'); }, async () => 'data:text/plain;base64,QQ==']) {
      const m = memory(), cache = new MobileFaviconCache(); m.seed(entry()); await cache.hydrate(m.io, owned); m.io.load = fail;
      expect(await cache.resolve(m.io, target(), '/new.png', options())).toBe(png);
      expect(await cache.resolve(m.io, target('/none'), '/new.png', options())).toBe('/new.png');
      expect(m.rows.size).toBe(1);
    }
  });
  test('unavailable persistence preserves successfully loaded image in bounded memory', async () => {
    const m = memory(), cache = new MobileFaviconCache(); await cache.hydrate(m.io, owned);
    m.io.write = async () => { throw new Error('disk full'); };
    expect(await cache.resolve(m.io, target(), '/new.png', options())).toBe(png);
    expect(m.rows.size).toBe(0); expect(cache.peek(target(), m.io)).toBe(png);
  });
  test('environment clear removes only matching rows and blocks downloads until it settles', async () => {
    const m = memory(), cache = new MobileFaviconCache(); m.seed(entry()); m.seed(entry(target('/other', 'other')));
    await cache.hydrate(m.io, owned);
    const cleared = deferred<void>(), clear = m.io.clear;
    m.io.clear = async id => { await clear(id); await cleared.promise; };
    const work = cache.clear(m.io, 'env');
    expect(cache.peek(target(), m.io)).toBeNull(); expect(cache.peek(target('/other', 'other'), m.io)).toBe(png);
    expect(await cache.resolve(m.io, target(), '/new.png', options())).toBe('/new.png');
    expect(m.events).not.toContain('load');
    cleared.resolve(); expect(await work).toBe(true); expect(m.rows.size).toBe(1);
    await cache.hydrate(m.io, owned);
    expect(await cache.resolve(m.io, target(), '/fresh.png', options())).toBe(png);
  });
  test('clear during native load discards late bytes and refuses post-clear write', async () => {
    for (const environmentId of ['env', undefined]) {
      const m = memory(), cache = new MobileFaviconCache(), loaded = deferred<string>(), started = deferred<void>();
      await cache.hydrate(m.io, owned);
      m.io.load = async () => { started.resolve(); return loaded.promise; };
      const work = cache.resolve(m.io, target(), '/new.png', options()); await started.promise;
      await cache.clear(m.io, environmentId); loaded.resolve(png);
      expect(await work).toBeNull(); expect(m.rows.size).toBe(0); expect(m.events).not.toContain('write');
    }
  });
  test('external cache clear invalidates memory and a delayed native read', async () => {
    const m = memory(), cache = new MobileFaviconCache(), row = m.seed(entry()), read = deferred<MobileCacheRecord | null>();
    m.io.read = () => read.promise;
    const work = cache.hydrate(m.io, owned);
    await Promise.resolve(); await m.io.clear('env'); read.resolve(row);
    expect(await work).toBe(true); expect(cache.peek(target(), m.io)).toBeNull();
    await cache.resolve(m.io, target(), '/new.png', options()); expect(cache.peek(target(), m.io)).toBe(png);
    await m.io.clear('env'); expect(cache.peek(target(), m.io)).toBeNull();
  });
  test('own clear during delayed metadata page prevents late hydration adoption', async () => {
    const m = memory(), cache = new MobileFaviconCache(), row = m.seed(entry()), page = deferred<MobileFaviconMetadata[]>();
    m.io.list = () => page.promise;
    const work = cache.hydrate(m.io, owned); await cache.clear(m.io); page.resolve([row]);
    expect(await work).toBe(false); expect(cache.peek(target(), m.io)).toBeNull(); expect(m.events).not.toContain('read');
  });
  test('caller loss during load does not write; a superseded answer propagates without cleanup calls', async () => {
    const m = memory(), cache = new MobileFaviconCache(); await cache.hydrate(m.io, owned);
    let current = true;
    m.io.load = async () => { current = false; return png; };
    expect(await cache.resolve(m.io, target(), '/new.png', { ...options(), current: () => current })).toBeNull();
    expect(m.events).not.toContain('write');
    current = true; m.io.load = async () => { throw new ClientError('gone', 'superseded'); }; m.events.length = 0;
    await expect(cache.resolve(m.io, target(), '/new.png', options())).rejects.toThrow('gone');
    expect(m.events).toEqual(['ticket']);
  });
  test('newer icon resolve wins over older delayed load', async () => {
    const m = memory(), cache = new MobileFaviconCache(), older = deferred<string>(), started = deferred<void>();
    await cache.hydrate(m.io, owned);
    m.io.load = async url => { if (url === '/a.png') { started.resolve(); return older.promise; } return 'data:image/png;base64,Qg=='; };
    const first = cache.resolve(m.io, target(), '/a.png', options()); await started.promise;
    expect(await cache.resolve(m.io, target(), '/b.png', options())).toBe('data:image/png;base64,Qg==');
    older.resolve(png); expect(await first).toBe('data:image/png;base64,Qg==');
    expect(JSON.parse([...m.rows.values()][0]!.payload).revision).toContain('b.png');
  });
  test('clear while write is delayed rejects its old ticket and never adopts the bytes', async () => {
    const m = memory(), cache = new MobileFaviconCache(), started = deferred<void>(), release = deferred<void>();
    await cache.hydrate(m.io, owned); const write = m.io.write;
    m.io.write = async (...args) => { started.resolve(); await release.promise; return write(...args); };
    const work = cache.resolve(m.io, target(), '/new.png', options()); await started.promise;
    await cache.clear(m.io); release.resolve(); expect(await work).toBeNull(); expect(m.rows.size).toBe(0);
    expect(cache.peek(target(), m.io)).toBeNull();
  });
  test('abandonment after successful write retries hydration/trim on next owned refresh', async () => {
    const m = memory(), cache = new MobileFaviconCache();
    for (let i = 0; i < 128; i++) m.seed(entry(target(`/p${i}`)));
    await cache.hydrate(m.io, owned); const write = m.io.write;
    m.io.write = async (...args) => { await write(...args); throw new ClientError('gone', 'superseded'); };
    await expect(cache.resolve(m.io, target('/new'), '/new.png', options())).rejects.toThrow('gone');
    expect(m.rows.size).toBe(129);
    expect(await cache.hydrate(m.io, owned)).toBe(true); expect(m.rows.size).toBe(128);
    expect(cache.peek(target('/p0'), m.io)).toBeNull(); expect(cache.peek(target('/new'), m.io)).toBe(png);
  });
  test('failed clear cannot resurrect disk rows before a successful clear recovery', async () => {
    const m = memory(), cache = new MobileFaviconCache(); m.seed(entry()); await cache.hydrate(m.io, owned);
    const clear = m.io.clear; m.io.clear = async () => { throw new Error('locked'); };
    expect(await cache.clear(m.io, 'env')).toBe(false);
    expect(await cache.hydrate(m.io, owned)).toBe(true); expect(cache.peek(target(), m.io)).toBeNull();
    const loads = m.events.filter(event => event === 'load').length;
    expect(await cache.resolve(m.io, target(), '/fresh.png', options())).toBe('/fresh.png');
    expect(m.events.filter(event => event === 'load')).toHaveLength(loads);
    expect(await cache.resolve(m.io, target('/other', 'other'), '/fresh.png', options())).toBe(png);
    m.io.clear = clear; expect(await cache.clear(m.io, 'env')).toBe(true);
    expect(await cache.hydrate(m.io, owned)).toBe(true); expect(m.rows.size).toBe(1);
    expect(cache.peek(target('/other', 'other'), m.io)).toBe(png);
  });
});

describe('favicon concurrent owner boundaries', () => {
  test('authoritative missing marker invalidates a pending write even without a memory entry', async () => {
    const m = memory(), cache = new MobileFaviconCache(), started = deferred<void>(), release = deferred<void>();
    await cache.hydrate(m.io, owned); const write = m.io.write;
    m.io.write = async (...args) => { started.resolve(); await release.promise; return write(...args); };
    const earlier = cache.resolve(m.io, target(), '/new.png', options()); await started.promise;
    expect(cache.peek(target(), m.io)).toBeNull();
    expect(await cache.resolve(m.io, target(), '/project-favicon-missing', options())).toBeNull();
    release.resolve(); expect(await earlier).toBeNull(); expect(m.rows.size).toBe(0);
    const restart = new MobileFaviconCache(); await restart.hydrate(m.io, owned);
    expect(restart.peek(target(), m.io)).toBeNull();
  });
  test('rehydration waits for all admitted resolves after another resolve is abandoned', async () => {
    const m = memory(), cache = new MobileFaviconCache(), started = deferred<void>(), release = deferred<string>();
    await cache.hydrate(m.io, owned);
    m.io.load = async url => {
      if (url === '/pending.png') { started.resolve(); return release.promise; }
      throw new ClientError('gone', 'superseded');
    };
    const pending = cache.resolve(m.io, target('/pending'), '/pending.png', options()); await started.promise;
    await expect(cache.resolve(m.io, target('/abandoned'), '/abandoned.png', options())).rejects.toThrow('gone');
    const listCount = m.events.filter(event => event === 'list').length;
    expect(await cache.hydrate(m.io, owned)).toBe(false);
    expect(m.events.filter(event => event === 'list')).toHaveLength(listCount);
    release.resolve(png); expect(await pending).toBe(png);
    expect(await cache.hydrate(m.io, owned)).toBe(true);
    expect(cache.peek(target('/pending'), m.io)).toBe(png);
  });
  test('late successful clear cannot reopen hydration after a newer failed clear', async () => {
    const m = memory(), cache = new MobileFaviconCache(), release = deferred<void>();
    m.seed(entry()); await cache.hydrate(m.io, owned);
    let calls = 0;
    m.io.clear = async () => { if (++calls === 1) await release.promise; else throw new Error('clear failed'); };
    const first = cache.clear(m.io, 'env');
    expect(await cache.clear(m.io, 'env')).toBe(false);
    release.resolve(); expect(await first).toBe(true);
    expect(await cache.hydrate(m.io, owned)).toBe(true);
    expect(cache.peek(target(), m.io)).toBeNull();
    expect(await cache.resolve(m.io, target(), '/fresh.png', options())).toBe('/fresh.png');
    expect(m.events).not.toContain('load');
  });
});

describe('favicon enumeration and eviction edge cases', () => {
  test('keyset pagination handles tied timestamps while deleting previous pages', async () => {
    const m = memory(), cache = new MobileFaviconCache();
    for (let i = 0; i < 260; i++) m.seed(entry(target(`/p${String(i).padStart(3, '0')}`, i % 2 ? 'b' : 'a')), 1);
    expect(await cache.hydrate(m.io, owned)).toBe(true);
    expect(m.events.filter(event => event === 'read')).toHaveLength(260);
    expect(m.rows.size).toBe(128);
    expect([...m.rows.values()].every(row => row.environmentId === 'b')).toBe(true);
    expect(cache.peek(target('/p003', 'b'), m.io)).toBeNull();
    expect(cache.peek(target('/p005', 'b'), m.io)).toBe(png);
  });
  test('a replacement written during eviction survives conditional payload removal', async () => {
    const m = memory(), cache = new MobileFaviconCache();
    for (let i = 0; i < 129; i++) m.seed(entry(target(`/p${i}`)));
    const remove = m.io.remove;
    m.io.remove = async (key, payload) => {
      if (key.key === mobileFaviconResourceKey(target('/p0'))) m.seed(entry(target('/p0'), png, '/newer.png'));
      return remove(key, payload);
    };
    expect(await cache.hydrate(m.io, owned)).toBe(true);
    const replacement = [...m.rows.values()].find(row => row.key === mobileFaviconResourceKey(target('/p0')));
    expect(replacement).toBeDefined(); expect(JSON.parse(replacement!.payload).revision).toContain('newer.png');
    // A concurrent external replacement may leave 129 durable rows. The owner
    // promises bounded memory, not atomic disk pruning across external writers.
    expect(m.rows.size).toBe(129); expect(cache.peek(target('/p0'), m.io)).toBeNull();
  });
  test('external clear after a row was read prevents final hydration adoption', async () => {
    const m = memory(), cache = new MobileFaviconCache(); m.seed(entry()); m.seed(entry(target('/later', 'other')));
    const read = m.io.read;
    m.io.read = async key => {
      if (key.environmentId === 'other') await m.io.clear('env');
      return read(key);
    };
    expect(await cache.hydrate(m.io, owned)).toBe(true);
    expect(cache.peek(target(), m.io)).toBeNull(); expect(cache.peek(target('/later', 'other'), m.io)).toBe(png);
    expect(m.events).not.toContain('remove');
  });
  test('out-of-order metadata and changed row timestamps require a fresh hydration pass', async () => {
    for (const changed of [false, true]) {
      const m = memory(), cache = new MobileFaviconCache();
      const a = m.seed(entry(target('/a'))), b = m.seed(entry(target('/b')));
      const list = m.io.list;
      m.io.list = async () => {
        if (changed) return [{ ...a, updatedAt: a.updatedAt - 1 }];
        return [b, a];
      };
      expect(await cache.hydrate(m.io, owned)).toBe(false); expect(cache.peek(target('/a'), m.io)).toBeNull();
      m.io.list = list; expect(await cache.hydrate(m.io, owned)).toBe(true); expect(cache.peek(target('/a'), m.io)).toBe(png);
    }
  });
});

test('concurrent resolves select byte-budget evictions before awaiting disk cleanup', async () => {
  const m = memory(), cache = new MobileFaviconCache(), header = 'data:image/png;base64,';
  const medium = header + 'A'.repeat(8958 - header.length), large = header + 'A'.repeat(32768 - header.length);
  for (let i = 0; i < 127; i++) m.seed(entry(target(`/p${i}`), i < 10 ? png : medium));
  await cache.hydrate(m.io, owned); m.io.load = async () => large;
  const remove = m.io.remove, started = deferred<void>(), release = deferred<void>();
  let first = true;
  m.io.remove = async (...args) => {
    if (first) { first = false; started.resolve(); await release.promise; }
    return remove(...args);
  };
  const a = cache.resolve(m.io, target('/new-a'), '/a.png', options()); await started.promise;
  expect(await cache.resolve(m.io, target('/new-b'), '/b.png', options())).toBe(large);
  release.resolve(); expect(await a).toBe(large);
  expect(cache.peek(target('/p17'), m.io)).toBeNull();
  expect(cache.peek(target('/p18'), m.io)).toBe(medium);
  expect(m.rows.size).toBe(111);
  expect([...m.rows.values()].reduce((sum, row) => sum + JSON.parse(row.payload).dataUrl.length, 0)).toBeLessThanOrEqual(1048576);
});

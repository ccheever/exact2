import { expect, test } from 'bun:test';
import { MobileFaviconQueries, MOBILE_FAVICON_STALE_MS, MOBILE_FAVICON_REFRESH_MS, MOBILE_FAVICON_IDLE_MS,
  type MobileFaviconEndpoint, type MobileFaviconDemand } from './mobile-favicon-query';
import { MobileFaviconCache, mobileFaviconResourceKey, type MobileFaviconIO } from './mobile-favicon-cache';
import { mobileFaviconCatalogIO } from './mobile-favicon-io';
import { mobileCacheCompareMetadata, type MobileCacheRecord } from './mobile-client-cache';

const png = 'data:image/png;base64,QQ==';
function fixture() {
  const query = new MobileFaviconQueries(), cache = new MobileFaviconCache(), rows = new Map<string, MobileCacheRecord>();
  const calls: string[] = [], loads: string[] = [], removals: (string | undefined)[] = [];
  const target = { environmentId: 'env', cwd: '/work', faviconPath: null }, key = mobileFaviconResourceKey(target);
  const demand: MobileFaviconDemand = { ...target, mountId: 'home:row' };
  let revision = 0, tick = 0;
  const state = { now: 1000, origin: 'https://home.invalid', catalog: '["env","https://home.invalid"]', generation: 1,
    phase: 'connected' as MobileFaviconEndpoint['phase'], clones: [] as MobileFaviconEndpoint['clones'], alive: true,
    reply: async (): Promise<unknown> => ({ relativeUrl: '/icon.png?token=secret' }),
    load: async () => png };
  const base: MobileFaviconIO = {
    scopeRevision: () => `${revision}`,
    async list(after, limit) { return [...rows.values()].sort(mobileCacheCompareMetadata)
      .filter(row => row.kind === 'project-favicon' && (!after || mobileCacheCompareMetadata(row, after) > 0))
      .slice(0, limit).map(({ payload: _payload, ...row }) => row); },
    async read(key) { return rows.get(key.key) ?? null; },
    async ticket() { return `${revision}`; },
    async write(key, ticket, payload) { if (ticket !== `${revision}`) return false;
      rows.set(key.key, { ...key, payload, schemaVersion: 1, updatedAt: ++tick }); return true; },
    async remove(key, expected) { removals.push(expected); if (expected === undefined || rows.get(key.key)?.payload === expected) rows.delete(key.key); },
    async clear() { revision++; rows.clear(); },
    async load(url) { loads.push(url); return state.load(); },
  };
  const io = mobileFaviconCatalogIO(base, { identity: env => env === 'env' ? state.catalog : '', current: () => state.alive });
  const endpoint = (): MobileFaviconEndpoint => {
    const catalogIdentity = state.catalog, scopeRevision = io.scopeRevision!('env'), origin = state.origin, generation = state.generation;
    return { environmentId: 'env', catalogIdentity, scopeRevision, origin, generation, phase: state.phase, clones: state.clones,
      current: () => state.alive && catalogIdentity === state.catalog && scopeRevision === io.scopeRevision!('env')
        && origin === state.origin && generation === state.generation,
      async request(resource) { calls.push(JSON.stringify(resource)); return state.reply(); } };
  };
  const run = (demands: MobileFaviconDemand[] = [demand], refresh?: string[]) => query.prepare({ demands,
    endpoints: [endpoint()], now: () => state.now, current: () => state.alive, cache, io, refresh });
  return { query, cache, rows, target, key, demand, state, io, base, endpoint, run, calls, loads, removals };
}

test('first demand requests the real resource, resolves relative URL and persists only image provenance', async () => {
  const f = fixture(), view = await f.run();
  expect(JSON.parse(f.calls[0]!)).toEqual({ _tag: 'project-favicon', cwd: '/work' });
  expect(f.loads).toEqual(['https://home.invalid/icon.png?token=secret']);
  expect(view.items[0]).toMatchObject({ key: f.key, url: png, state: 'success', error: '' });
  const row = f.rows.get(f.key)!;
  expect(row.schemaVersion).toBe(1); expect(JSON.parse(row.payload).catalogIdentity).toBe(f.state.catalog);
  expect(row.payload).not.toContain('token'); expect(row.payload).not.toContain('relativeUrl');
  expect(view.nextDeadline).toBe(1000 + MOBILE_FAVICON_REFRESH_MS);
});

test.each([MOBILE_FAVICON_STALE_MS - 1, MOBILE_FAVICON_STALE_MS, MOBILE_FAVICON_STALE_MS + 1])('remount freshness at %ims uses successful RPC time', async age => {
  const f = fixture(); await f.run(); f.state.now += age;
  await f.run([]); await f.run();
  expect(f.calls).toHaveLength(age < MOBILE_FAVICON_STALE_MS ? 1 : 2);
  expect(f.loads).toHaveLength(1);
});

test('root clock reads do not turn five-minute staleness into polling or rearm the thirty-minute deadline', async () => {
  const f = fixture(); await f.run();
  f.state.now += MOBILE_FAVICON_STALE_MS; await f.run();
  expect(f.calls).toHaveLength(1);
  f.state.now = 1000 + MOBILE_FAVICON_REFRESH_MS - 1; await f.run(); expect(f.calls).toHaveLength(1);
  f.state.now++; f.state.reply = async () => { f.state.now += 250; return { relativeUrl: '/icon.png?token=rotated' }; };
  const view = await f.run(); expect(f.calls).toHaveLength(2); expect(f.loads).toHaveLength(1);
  expect(view.nextDeadline).toBe(f.state.now + MOBILE_FAVICON_REFRESH_MS);
});

test('real inherited replies are reused without eager image downloads or duplicate queries', async () => {
  const f = fixture(), captured = f.endpoint();
  const ticket = f.query.beginObservation(f.target, captured, f.state.now);
  expect(f.query.observeReply(ticket, { ok: true, value: { relativeUrl: '/observed.png' } }, f.state.now, captured.current)).toBe(true);
  await f.run([]); expect(f.calls).toHaveLength(0); expect(f.loads).toHaveLength(0);
  expect((await f.run()).items[0]?.url).toBe(png);
  expect(f.calls).toHaveLength(0); expect(f.loads).toEqual(['https://home.invalid/observed.png']);
});

test('a failed lookup retains display and does not refresh the previous success timestamp', async () => {
  const f = fixture(); await f.run(); f.state.now += 100;
  f.state.reply = async () => { throw new Error('Permission denied'); };
  const failed = await f.run([f.demand], [f.key]);
  expect(failed.items[0]).toMatchObject({ url: png, state: 'failure', error: 'Permission denied' });
  f.state.now = 1000 + MOBILE_FAVICON_STALE_MS; await f.run([]); await f.run();
  expect(f.calls).toHaveLength(3); expect(f.loads).toHaveLength(1);
});

test('failure without success retries on remount, while waiting never starts another request', async () => {
  const f = fixture(); f.state.reply = async () => { throw new Error('Failed'); };
  await f.run(); await f.run([]); await f.run(); expect(f.calls).toHaveLength(2);
  f.state.phase = 'waiting'; await f.run(); f.state.now += MOBILE_FAVICON_REFRESH_MS;
  const waiting = await f.run(); expect(waiting.items[0]?.state).toBe('waiting'); expect(f.calls).toHaveLength(2);
  f.state.phase = 'connected'; await f.run(); expect(f.calls).toHaveLength(3);
});

test('offline failures preserve cached image without issuing RPC, and same-generation reconnect refreshes', async () => {
  const f = fixture(); await f.run(); f.state.phase = 'unavailable';
  expect((await f.run()).items[0]).toMatchObject({ url: png, state: 'failure' });
  expect(f.calls).toHaveLength(1); f.state.phase = 'connected'; await f.run(); expect(f.calls).toHaveLength(2);
});

test('only first matching clone phase changes refresh; initial completion and disappearance count', async () => {
  const f = fixture(); await f.run();
  f.state.clones = [{ destinationPath: '/elsewhere', phase: 'done' }]; await f.run(); expect(f.calls).toHaveLength(1);
  f.state.clones = [{ destinationPath: '/work', phase: 'done' }]; await f.run(); expect(f.calls).toHaveLength(2);
  f.state.clones = [{ destinationPath: '/work', phase: 'done' }, { destinationPath: '/work', phase: 'running' }];
  await f.run(); expect(f.calls).toHaveLength(2);
  f.state.clones = [{ destinationPath: '/work', phase: 'running' }]; await f.run(); expect(f.calls).toHaveLength(3);
  f.state.clones = []; await f.run(); expect(f.calls).toHaveLength(4);
});

test('simultaneous surface demands keep ownership when one departs', async () => {
  const f = fixture(), sidebar = { ...f.demand, mountId: 'sidebar:row' };
  await f.run([f.demand, sidebar]); f.state.now += MOBILE_FAVICON_STALE_MS;
  await f.run([sidebar]); expect(f.calls).toHaveLength(1);
  await f.run([]); await f.run([sidebar]); expect(f.calls).toHaveLength(2);
});

test('retained evaluated deadline survives demand departure, while idle TTL disposes runtime only', async () => {
  const f = fixture(); await f.run(); await f.run([]);
  f.state.now += MOBILE_FAVICON_REFRESH_MS; await f.run([]); expect(f.calls).toHaveLength(2);
  f.state.now = 1000 + MOBILE_FAVICON_IDLE_MS; const expired = await f.run([]);
  expect(f.calls).toHaveLength(2); expect(expired.nextDeadline).toBe(0); expect(f.rows.size).toBe(1);
  await f.run(); expect(f.calls).toHaveLength(3); expect(f.loads).toHaveLength(1);
});

test.each(['generation', 'origin', 'catalog', 'clear', 'last demand', 'answer'])('held lookup cannot adopt after %s changes', async changed => {
  const f = fixture(), entered = Promise.withResolvers<void>(), release = Promise.withResolvers<void>();
  f.state.reply = async () => { entered.resolve(); await release.promise; return { relativeUrl: '/late.png' }; };
  const pending = f.run(); await entered.promise;
  if (changed === 'generation') f.state.generation++;
  if (changed === 'origin') f.state.origin = 'https://other.invalid';
  if (changed === 'catalog') f.state.catalog = '["env","https://replacement.invalid"]';
  if (changed === 'clear') await f.io.clear('env');
  if (changed === 'last demand') f.query.reconcile([], f.state.now);
  if (changed === 'answer') f.state.alive = false;
  release.resolve(); await pending;
  expect(f.rows.size).toBe(0); expect(f.loads).toHaveLength(0);
  expect(f.query.display(f.target, f.endpoint(), f.cache)).toBeNull();
});

test('missing marker hides an old image before its awaited native deletion finishes', async () => {
  const f = fixture(); await f.run();
  const entered = Promise.withResolvers<void>(), release = Promise.withResolvers<void>(), remove = f.base.remove;
  f.base.remove = async (key, payload) => { if (payload === undefined) { entered.resolve(); await release.promise; } return remove(key, payload); };
  f.state.reply = async () => ({ relativeUrl: '/project-favicon-missing' });
  const pending = f.run([f.demand], [f.key]); await entered.promise;
  expect(f.query.display(f.target, f.endpoint(), f.cache)).toBeNull();
  release.resolve(); expect((await pending).items[0]?.url).toBe(''); expect(f.rows.size).toBe(0);
});

test('failed image loading is not repeated by unrelated root clock evaluations', async () => {
  const f = fixture(); f.state.load = async () => { throw new Error('decode failed'); };
  expect((await f.run()).items[0]?.url).toBe('https://home.invalid/icon.png?token=secret');
  f.state.now += 60_000; await f.run(); expect(f.loads).toHaveLength(1);
});

test('native letGo propagates and does not become a successful observed URL', async () => {
  const f = fixture(); f.state.reply = async () => { throw { name: 'FetchError', kind: 'Aborted' }; };
  await expect(f.run()).rejects.toMatchObject({ kind: 'Aborted' });
  expect(f.rows.size).toBe(0); expect(f.query.display(f.target, f.endpoint(), f.cache)).toBeNull();
});

test('a concurrent reader of the retained image cannot consume the new query completion', async () => {
  const f = fixture(); await f.run();
  const entered = Promise.withResolvers<void>(), release = Promise.withResolvers<void>();
  f.state.reply = async () => { entered.resolve(); await release.promise; return { relativeUrl: '/changed.png' }; };
  f.state.load = async () => 'data:image/png;base64,Qg==';
  const changing = f.run([f.demand], [f.key]); await entered.promise;
  expect((await f.run()).items[0]?.url).toBe(png);
  release.resolve(); expect((await changing).items[0]?.url).toBe('data:image/png;base64,Qg==');
  expect(f.loads).toEqual(['https://home.invalid/icon.png?token=secret', 'https://home.invalid/changed.png']);
});

test('an abandoned observer token cannot later adopt a reply', () => {
  const f = fixture(), endpoint = f.endpoint(), token = f.query.beginObservation(f.target, endpoint, f.state.now);
  f.query.abandonObservation(token);
  expect(f.query.observeReply(token, { ok: true, value: { relativeUrl: '/late.png' } }, f.state.now, endpoint.current)).toBe(false);
  expect(f.query.display(f.target, endpoint, f.cache)).toBeNull();
});

test.each(['clear', 'catalog', 'answer'])('a held refresh cannot project previously cached data after %s invalidates the captured view', async changed => {
  const f = fixture(); await f.run();
  const entered = Promise.withResolvers<void>(), release = Promise.withResolvers<void>();
  f.state.reply = async () => { entered.resolve(); await release.promise; return { relativeUrl: '/late.png' }; };
  const pending = f.run([f.demand], [f.key]); await entered.promise;
  if (changed === 'clear') await f.io.clear('env');
  if (changed === 'catalog') f.state.catalog = '["env","https://replacement.invalid"]';
  if (changed === 'answer') f.state.alive = false;
  release.resolve(); const view = await pending;
  expect(view.items.every(row => row.url === '')).toBe(true);
  if (changed === 'answer') expect(view.items).toEqual([]);
});

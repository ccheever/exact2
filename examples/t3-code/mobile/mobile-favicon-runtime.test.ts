import { afterEach, expect, test } from 'bun:test';
import { noteNow } from './shared/composer-controls';
import { MobileDraftClient } from './mobile-draft-recovery';
import { mobilePrepareFavicons, mobileFaviconDisplay, mobileObserveFaviconRpc } from './mobile-favicon-runtime';
import { mobileFaviconQueries } from './mobile-favicon-query';
import { mobileFaviconCache } from './mobile-cache-controls';
import { withMobileFaviconIO } from './mobile-favicon-io';
import { mobileFaviconResourceKey } from './mobile-favicon-cache';
import { mobileCacheClear, mobileCacheCompareMetadata, type MobileCacheRecord } from './mobile-client-cache';
import { fleet, environmentKey } from './shared/settings-b-fleet';
import { obj, initialShell, type Obj } from './shared/domain';
import { ClientError, type Native } from './shared/protocol';

afterEach(() => { fleet.saved = []; fleet.entries.clear(); });
let count = 0;
const png = 'data:image/png;base64,QQ==';
async function fixture() {
  const client = new MobileDraftClient(), a = `icon-runtime-${++count}-a`, b = `icon-runtime-${count}-b`;
  client.adoptStatus({ state: 'connected', origin: 'https://a-route.invalid', environmentId: a, message: '' }, 2);
  client.shell = initialShell(); client.shellLive = true; client.configLive = true; client.shellLoaded = true;
  fleet.saved = [{ environmentId: a, origin: 'https://a-home.invalid' }, { environmentId: b, origin: 'https://b-home.invalid' }];
  const fleetKey = environmentKey('https://b-home.invalid', b);
  const entry = { key: fleetKey, environmentId: b, origin: 'https://b-route.invalid', generation: 7, synchronized: 7,
    phase: 'connected' as const, message: '', traceId: '', lastEvent: 0, subscriptions: {}, config: {}, shell: initialShell(),
    scopes: [], error: '', requested: true };
  fleet.entries.set(fleetKey, entry);
  const rows = new Map<string, MobileCacheRecord>(), calls: Obj[] = [], watches: string[] = [];
  let epoch = 0, updatedAt = 0, now = Date.now();
  // Production snapshot admits its Contract clock before inherited RPCs run.
  noteNow(client, now);
  const rowKey = (request: Obj) => JSON.stringify([request.environmentId, request.kind, request.key]);
  const state = { current: true, response: async (_request: Obj): Promise<Obj> => ({ relativeUrl: '/project-icon.png?token=private' }) };
  const native: Native = { available: true, watch(topic) { watches.push(topic); }, async later(input) {
    const r = obj(input); calls.push(r);
    if (r.op === 'request') return { ok: true, generation: r.generation, value: await state.response(r) };
    if (r.op === 'mobileFaviconImage') return { ok: true, generation: 0, value: { dataUrl: png } };
    if (r.op !== 'mobileClientCache') throw new Error(`Unexpected ${r.op}`);
    let value: Obj;
    if (r.action === 'clear' || r.action === 'clearKind') {
      epoch++; let removed = 0;
      for (const [key, row] of rows) if ((!r.environmentId || row.environmentId === r.environmentId) && (!r.kind || row.kind === r.kind)) { rows.delete(key); removed++; }
      value = { removed };
    } else if (r.action === 'ticket') value = { ticket: `${epoch}` };
    else if (r.action === 'write') {
      const accepted = r.ticket === `${epoch}`;
      if (accepted) rows.set(rowKey(r), { environmentId: String(r.environmentId), kind: 'project-favicon', key: String(r.key),
        schemaVersion: 1, updatedAt: ++updatedAt, payload: String(r.payload) });
      value = { written: accepted, stale: !accepted };
    } else if (r.action === 'list') value = { rows: [...rows.values()].sort(mobileCacheCompareMetadata)
      .filter(row => !r.after || mobileCacheCompareMetadata(row, r.after as any) > 0).slice(0, Number(r.limit)).map(({ payload: _payload, ...row }) => row) };
    else if (r.action === 'read') value = { record: rows.get(rowKey(r)) ?? null };
    else if (r.action === 'remove') { if (r.expectedPayload === undefined || rows.get(rowKey(r))?.payload === r.expectedPayload) rows.delete(rowKey(r)); value = { removed: 1 }; }
    else throw new Error(`Unexpected ${r.action}`);
    return { ok: true, generation: 0, value };
  } };
  await withMobileFaviconIO(native, io => mobileFaviconCache.clear(io)); calls.length = 0;
  const demand = (environmentId = a) => ({ mountId: `home:${environmentId}`, environmentId, cwd: '/project', faviconPath: '/custom.png' });
  const run = (demands = [demand(a), demand(b)], refresh?: string[]) => mobilePrepareFavicons({ client, native, demands,
    now: () => now, current: () => state.current, refresh });
  return { client, a, b, entry, fleetKey, rows, calls, watches, state, native, demand, run, advance: (ms: number) => { now += ms; } };
}

test('runtime uses actual focused/fleet generations and routes but persists canonical home provenance', async () => {
  const f = await fixture(), view = await f.run();
  expect(view.items.map(row => row.url)).toEqual([png, png]);
  const requests = f.calls.filter(r => r.op === 'request'); expect(requests).toHaveLength(2);
  expect(requests[0]).toMatchObject({ method: 'assets.createUrl', generation: 2,
    payload: { resource: { _tag: 'project-favicon', cwd: '/project', path: '/custom.png' } } });
  expect(requests[0]!.fleet).toBeUndefined();
  expect(requests[1]).toMatchObject({ fleet: f.fleetKey, generation: 7, method: 'assets.createUrl' });
  expect(f.calls.filter(r => r.op === 'mobileFaviconImage').map(r => r.url)).toEqual([
    'https://a-route.invalid/project-icon.png?token=private', 'https://b-route.invalid/project-icon.png?token=private']);
  expect([...f.rows.values()].map(row => JSON.parse(row.payload).catalogIdentity)).toEqual([
    JSON.stringify([f.a, 'https://a-home.invalid']), JSON.stringify([f.b, 'https://b-home.invalid'])]);
  expect(f.watches).toEqual(['t3.status', 't3.fleet']);
  expect(f.client.scopes).toEqual([]); expect(f.entry.scopes).toEqual([]);
});

test.each(['focused generation', 'fleet generation', 'fleet replacement', 'catalog', 'clear', 'disabled', 'offline', 'answer'])('held real request cannot publish after %s changes', async changed => {
  const f = await fixture(), remote = changed.startsWith('fleet'), env = remote ? f.b : f.a;
  const entered = Promise.withResolvers<void>(), release = Promise.withResolvers<void>();
  f.state.response = async () => { entered.resolve(); await release.promise; return { relativeUrl: '/late.png' }; };
  const pending = f.run([f.demand(env)]); await entered.promise;
  if (changed === 'focused generation') f.client.generation++;
  if (changed === 'fleet generation') f.entry.generation++;
  if (changed === 'fleet replacement') fleet.entries.set(f.fleetKey, { ...f.entry });
  if (changed === 'catalog') fleet.saved = fleet.saved.map(row => row.environmentId === env ? { ...row, origin: 'https://replacement.invalid' } : row);
  if (changed === 'clear') await mobileCacheClear(f.native, { environmentId: env, kind: 'project-favicon' });
  if (changed === 'disabled') fleet.saved = fleet.saved.map(row => row.environmentId === env ? { ...row, enabled: false } : row);
  if (changed === 'offline') f.client.connection = 'error';
  if (changed === 'answer') f.state.current = false;
  release.resolve(); const result = await pending.then(value => value, error => error);
  expect(result).toBeInstanceOf(ClientError); expect(result.kind).toBe('superseded');
  expect(f.calls.some(r => r.op === 'mobileFaviconImage')).toBe(false); expect(f.rows.size).toBe(0);
  expect(mobileFaviconDisplay(f.client, f.demand(env))).toBeNull();
});

test('disabled/offline environments retain cached display and issue no additional request', async () => {
  const f = await fixture(); await f.run();
  f.client.connection = 'error'; fleet.saved = fleet.saved.map(row => row.environmentId === f.b ? { ...row, enabled: false } : row);
  f.calls.length = 0; const view = await f.run();
  expect(view.items.map(row => row.url)).toEqual([png, png]);
  expect(view.items.every(row => row.state === 'failure')).toBe(true);
  expect(f.calls.some(r => r.op === 'request' || r.op === 'mobileFaviconImage')).toBe(false);
  expect(f.rows.size).toBe(2);
});

test('inherited MobileDraftClient rpc returns actual reply and suppresses duplicate demand lookup', async () => {
  const f = await fixture(), answer = { relativeUrl: '/inherited.png', expiresAt: 'real-expiration' };
  f.state.response = async () => answer;
  const previous = mobileFaviconQueries.version;
  const result = await f.client.rpc(f.native, 'assets.createUrl', { resource: { _tag: 'project-favicon', cwd: '/project', path: '/custom.png' } });
  expect(result).toBe(answer); expect(mobileFaviconQueries.version).toBeGreaterThan(previous);
  expect(f.calls.filter(r => r.op === 'request')).toHaveLength(1); expect(f.calls.some(r => r.op === 'mobileFaviconImage')).toBe(false);
  expect((await f.run([f.demand()])).items[0]?.url).toBe(png);
  expect(f.calls.filter(r => r.op === 'request')).toHaveLength(1);
  expect(f.calls.filter(r => r.op === 'mobileFaviconImage')[0]?.url).toBe('https://a-route.invalid/inherited.png');
});

test('observer preserves exact failures and lets non-favicon RPC pass through without observation', async () => {
  const f = await fixture(), error = new Error('Actual permission failure');
  await expect(mobileObserveFaviconRpc(f.client, 'assets.createUrl', { resource: { _tag: 'project-favicon', cwd: '/project' } },
    async () => { throw error; })).rejects.toBe(error);
  const revision = mobileFaviconQueries.version, value = { ordinary: true };
  expect(await mobileObserveFaviconRpc(f.client, 'other.method', {}, async () => value)).toBe(value);
  expect(mobileFaviconQueries.version).toBe(revision);
});

test('observer discards stale catalog result but still returns the real successful RPC value', async () => {
  const f = await fixture(), value = { relativeUrl: '/stale.png' };
  const result = await mobileObserveFaviconRpc(f.client, 'assets.createUrl', { resource: { _tag: 'project-favicon', cwd: '/project' } }, async () => {
    fleet.saved = fleet.saved.map(row => row.environmentId === f.a ? { ...row, origin: 'https://new-home.invalid' } : row);
    return value;
  });
  expect(result).toBe(value);
  expect(mobileFaviconDisplay(f.client, { environmentId: f.a, cwd: '/project' })).toBeNull();
});

test('route demand reconciliation invalidates old work before catalog cleanup awaits', async () => {
  const f = await fixture(), entered = Promise.withResolvers<void>(), release = Promise.withResolvers<void>();
  f.state.response = async () => { entered.resolve(); await release.promise; return { relativeUrl: '/departed.png' }; };
  const pending = f.run([f.demand()]); await entered.promise;
  await f.run([]); release.resolve(); await pending;
  expect(f.rows.size).toBe(0); expect(f.calls.some(r => r.op === 'mobileFaviconImage')).toBe(false);
});

test('manual refresh key reaches the same captured endpoint owner', async () => {
  const f = await fixture(); await f.run([f.demand()]);
  await f.run([f.demand()], [mobileFaviconResourceKey(f.demand())]);
  expect(f.calls.filter(r => r.op === 'request')).toHaveLength(2);
});

test('actual adopted focused and fleet clone streams refresh only their matching environment', async () => {
  const { liveEvent, liveFleetEvent, PROJECT_CLONES_KEY } = await import('./shared/live-streams');
  const f = await fixture(); await f.run(); f.calls.length = 0;
  liveEvent(f.client, { key: PROJECT_CLONES_KEY, generation: 2, subscriptionId: '2-1', value: [{ destinationPath: '/project', phase: 'done' }] });
  await f.run(); expect(f.calls.filter(r => r.op === 'request').map(r => r.fleet)).toEqual([undefined]);
  f.calls.length = 0;
  liveFleetEvent(f.entry, { key: PROJECT_CLONES_KEY, generation: 7, subscriptionId: '7-1', value: [{ destinationPath: '/project', phase: 'done' }] });
  await f.run(); expect(f.calls.filter(r => r.op === 'request').map(r => r.fleet)).toEqual([f.fleetKey]);
  f.calls.length = 0;
  liveFleetEvent(f.entry, { key: PROJECT_CLONES_KEY, generation: 7, subscriptionId: '7-1', value: [{ destinationPath: '/project', phase: 'done', progress: 50 }] });
  await f.run(); expect(f.calls.some(r => r.op === 'request')).toBe(false);
});

test('MobileDraftClient observer preserves an actual transport error object', async () => {
  const f = await fixture(), error = new ClientError('Actual transport denial', 'Permission');
  f.state.response = async () => { throw error; };
  await expect(f.client.rpc(f.native, 'assets.createUrl', { resource: { _tag: 'project-favicon', cwd: '/project' } })).rejects.toBe(error);
  expect(f.calls.filter(r => r.op === 'request')).toHaveLength(1); expect(f.rows.size).toBe(0);
});

test('mobileSnapshot exposes the separate favicon revision', async () => {
  const { mobileSnapshot } = await import('./client');
  const snapshot = await mobileSnapshot(null, { fs: { async mkdir() {}, async readFile() { return new ArrayBuffer(0); }, async atomicWriteFile() {} } });
  expect(snapshot.faviconRevision).toBe(mobileFaviconQueries.version);
});

test('multiple missing icons retain their result without invalidating siblings or querying again', async () => {
  const f = await fixture(), one = { ...f.demand(), mountId: 'missing-one', cwd: '/missing-one' },
    two = { ...f.demand(), mountId: 'missing-two', cwd: '/missing-two' }, normal = f.demand();
  f.state.response = async request => ({ relativeUrl: String(obj(obj(request.payload).resource).cwd).startsWith('/missing-')
    ? '/project-favicon-missing' : '/present.png' });
  const view = await f.run([one, two, normal]);
  expect(view.items.map(row => row.url)).toEqual(['', '', png]);
  expect(f.calls.filter(r => r.op === 'request')).toHaveLength(3);
  expect(f.calls.filter(r => r.action === 'remove')).toHaveLength(2);
  expect(f.calls.filter(r => r.op === 'mobileFaviconImage')).toHaveLength(1);
  f.calls.length = 0; f.advance(60_000);
  expect((await f.run([one, two, normal])).items.map(row => row.url)).toEqual(['', '', png]);
  expect(f.calls.some(r => r.op === 'request' || r.op === 'mobileFaviconImage' || r.action === 'remove')).toBe(false);
});

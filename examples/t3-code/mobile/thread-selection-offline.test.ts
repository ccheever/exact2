import { afterEach, expect, test } from 'bun:test';
import { MobileDraftClient } from './mobile-draft-recovery';
import { mobileThreadSelection } from './thread-selection';
import { mobileHomeSources, projectMobileHome } from './home';
import { fleet } from './shared/settings-b-fleet';
import { obj, str, type Obj } from './shared/domain';
import type { Files, Native } from './shared/protocol';
import { mobileCacheSync } from './mobile-client-cache-sync';
import { mobileCacheFleetSync } from './mobile-client-cache-fleet';
import { encodeMobileCatalogPayload, mobileCacheCatalogIdentity } from './mobile-client-cache-catalog';
import { mobileCacheClear } from './mobile-client-cache';
import { encodeMobileShellCache, encodeMobileThreadCache } from './mobile-client-cache-codec';

afterEach(() => { fleet.entries.clear(); fleet.saved = []; });
let count = 0;
function fixture() {
  const client = new MobileDraftClient(), calls: Obj[] = [], writes: string[] = [];
  const a = `offline-selection-${++count}-a`, b = `offline-selection-${count}-b`;
  client.adoptStatus({ state: 'disconnected', origin: 'https://a.invalid', environmentId: a, message: '' }, 1);
  fleet.saved = [{ environmentId: a, origin: 'https://a.invalid' }, { environmentId: b, origin: 'https://b.invalid' }];
  const records = new Map<string, Obj>();
  for (const environmentId of [a, b]) {
    const threads = ['one', 'two'].map(id => ({ id, projectId: 'project', title: `${environmentId} ${id}`,
      createdAt: '2026-10-08T12:00:00Z', modelSelection: { instanceId: id, model: `model-${id}` } }));
    const wrap = (payload: string) => encodeMobileCatalogPayload(mobileCacheCatalogIdentity(fleet.saved, environmentId), payload);
    const payload = encodeMobileShellCache(environmentId, { sequence: 1, projects: [{ id: 'project', title: 'Project' }], threads });
    records.set(JSON.stringify([environmentId, 'shell', 'snapshot']), { environmentId, kind: 'shell', key: 'snapshot', schemaVersion: 1, payload: wrap(payload), updatedAt: 1 });
    for (const thread of threads) {
      const projection: Obj = { thread };
      for (const family of ['runs', 'attempts', 'nodes', 'subagents', 'providerSessions', 'providerThreads', 'providerTurns', 'runtimeRequests',
        'messages', 'plans', 'turnItems', 'checkpointScopes', 'checkpoints', 'contextHandoffs', 'contextTransfers', 'visibleTurnItems']) projection[family] = [];
      records.set(JSON.stringify([environmentId, 'thread', thread.id]), { environmentId, kind: 'thread', key: thread.id, schemaVersion: 1, updatedAt: 1,
        payload: wrap(encodeMobileThreadCache(environmentId, thread.id, { projection, sequence: 1, hasMore: false, historyCursor: null, latestLocalTurnOrdinal: null })) });
    }
  }
  const good = (value: unknown, generation = client.generation) => ({ ok: true, value, generation });
  const native: Native = { available: true, watch() {}, async later(input) {
    const r = obj(input); calls.push(r);
    if (r.op === 'mobileSelectSavedEnvironment') return good({ state: 'connecting', origin: 'https://b.invalid', environmentId: b, message: '' }, 2);
    if (r.op === 'fleetStop') return good({});
    if (r.op !== 'mobileClientCache') throw new Error(`Unexpected request ${r.op}`);
    if (r.action === 'read') return good({ record: records.get(JSON.stringify([r.environmentId, r.kind, r.key])) ?? null });
    if (r.action === 'clear') {
      let removed = 0;
      for (const [key, row] of records) if (!r.environmentId || row.environmentId === r.environmentId) { records.delete(key); removed++; }
      return good({ removed });
    }
    throw new Error(`Unexpected action ${r.action}`);
  } };
  const storage: Files = { fs: { async mkdir() {}, async readFile() { return new ArrayBuffer(0); }, async atomicWriteFile(_path, data) {
    writes.push(new TextDecoder().decode(data));
  } } };
  return { client, a, b, native, storage, calls, writes, async sync() {
    await mobileCacheSync(client, native, () => fleet.saved); await mobileCacheFleetSync(fleet, native, client);
  } };
}

test('a real Home background cache row cold-selects its saved environment with no synthetic live authority', async () => {
  const f = fixture(); await f.sync();
  const row = projectMobileHome(mobileHomeSources(f.client), Date.parse('2026-10-09T12:00:00Z')).items
    .find(row => row.environmentId === f.b && row.threadId === 'two');
  expect(row?.id).toBe(`fleet:${f.b}:two`);
  expect((await mobileThreadSelection(f.client, row!.id, f.native, f.storage)).message).toBe('');
  expect(f.client.environmentId).toBe(f.b); expect(f.client.threadId).toBe('two');
  expect(obj(f.client.thread?.projection.thread).id).toBe('two');
  expect(f.client.modelId).toBe('model-two');
  expect(f.client.connection).toBe('connecting'); expect(f.client.ready).toBe(false); expect(f.client.writable).toBe(false);
  expect(f.client.scopes).toEqual([]); expect(f.client.subscriptions).toEqual({}); expect(fleet.entries.size).toBe(0);
  expect(f.calls.filter(r => r.op !== 'mobileClientCache')).toEqual([
    { op: 'mobileSelectSavedEnvironment', origin: 'https://b.invalid', environmentId: f.b, generation: 1 },
  ]);
  expect(f.writes).toHaveLength(1);
});

test.each(['clear', 'forget', 'replace'])('a %s before tapping a displayed local row cannot select or persist it', async change => {
  const f = fixture(); await f.sync();
  if (change === 'clear') await mobileCacheClear(f.native, { environmentId: f.a });
  if (change === 'forget') fleet.saved = fleet.saved.filter(row => row.environmentId !== f.a);
  if (change === 'replace') fleet.saved = fleet.saved.map(row => row.environmentId === f.a ? { ...row, origin: 'https://replacement.invalid' } : row);
  expect((await mobileThreadSelection(f.client, 'one', f.native, f.storage)).message).toBe('This thread is unavailable offline.');
  expect(f.client.threadId).toBe(''); expect(f.writes).toEqual([]);
});

test.each(['forget', 'replace', 'clear', 'new selection'])('a held transcript read cannot publish or persist after %s', async change => {
  const f = fixture(); await f.sync();
  const entered = Promise.withResolvers<void>(), release = Promise.withResolvers<void>(), original = f.native.later;
  let intercepted = false;
  f.native.later = async input => {
    const value = await original(input), r = obj(input);
    if (!intercepted && r.action === 'read' && r.kind === 'thread' && r.key === 'one') { intercepted = true; entered.resolve(); await release.promise; }
    return value;
  };
  const pending = mobileThreadSelection(f.client, 'one', f.native, f.storage);
  const outcome = pending.then(() => 'published', error => str(obj(error).kind));
  await entered.promise;
  if (change === 'forget') fleet.saved = fleet.saved.filter(row => row.environmentId !== f.a);
  if (change === 'replace') fleet.saved = fleet.saved.map(row => row.environmentId === f.a ? { ...row, origin: 'https://replacement.invalid' } : row);
  if (change === 'clear') await mobileCacheClear(f.native, { environmentId: f.a });
  if (change === 'new selection') expect((await mobileThreadSelection(f.client, 'two', f.native, f.storage)).message).toBe('');
  release.resolve();
  expect(await outcome).toBe('superseded');
  expect(f.writes).toHaveLength(change === 'new selection' ? 1 : 0);
  if (change === 'new selection') { expect(f.client.threadId).toBe('two'); expect(f.client.modelId).toBe('model-two'); }
});


test('cached thread adoption can correct a stale saved project without superseding its selection', async () => {
  const f = fixture(); f.client.threadId = 'one'; f.client.projectId = 'old-project';
  expect((await mobileThreadSelection(f.client, 'one', f.native, f.storage)).message).toBe('');
  expect(f.client.projectId).toBe('project'); expect(f.client.modelId).toBe('model-one'); expect(f.writes).toHaveLength(1);
});

test.each(['connecting', 'error'])('the native focus status can arrive before its reply (%s)', async state => {
  const f = fixture(); await f.sync();
  const original = f.native.later;
  f.native.later = async input => {
    const value = await original(input);
    if (obj(input).op === 'mobileSelectSavedEnvironment')
      f.client.adoptStatus({ ...obj(obj(value).value), state }, 2);
    return value;
  };
  expect((await mobileThreadSelection(f.client, `fleet:${f.b}:two`, f.native, f.storage)).message).toBe('');
  expect(f.client.environmentId).toBe(f.b); expect(f.client.threadId).toBe('two');
  expect(f.client.modelId).toBe('model-two'); expect(f.client.connection).toBe(state);
  expect(f.writes).toHaveLength(1);
});

test.each(['fleetStop', 'cache read'])('an offline status phase change during %s keeps the selection owner', async boundary => {
  const f = fixture(); await f.sync();
  const original = f.native.later, key = `https://b.invalid\n${f.b}`;
  let changed = false;
  f.native.later = async input => {
    const r = obj(input), value = await original(input);
    if (r.op === 'mobileSelectSavedEnvironment' && boundary === 'fleetStop')
      fleet.entries.set(key, { key, origin: 'https://b.invalid', environmentId: f.b, phase: 'error', message: '',
        traceId: '', generation: 1, synchronized: -1, lastEvent: 0, subscriptions: {}, config: {},
        shell: f.client.shell, scopes: [], error: '', requested: true });
    if (!changed && (boundary === 'fleetStop' ? r.op === 'fleetStop'
      : r.action === 'read' && r.environmentId === f.b && f.client.environmentId === f.b)) {
      changed = true;
      f.client.adoptStatus({ state: 'error', origin: 'https://b.invalid', environmentId: f.b, message: 'Offline' }, 2);
    }
    return value;
  };
  expect((await mobileThreadSelection(f.client, `fleet:${f.b}:two`, f.native, f.storage)).message).toBe('');
  expect(changed).toBe(true); expect(f.client.connection).toBe('error');
  expect(f.client.threadId).toBe('two'); expect(f.client.modelId).toBe('model-two'); expect(f.writes).toHaveLength(1);
  expect(fleet.entries.has(key)).toBe(false);
});

test.each(['new selection', 'clear', 'replace', 'new generation', 'thread owner'])('focus notification before reply still rejects %s', async change => {
  const f = fixture(); await f.sync();
  const original = f.native.later;
  f.native.later = async input => {
    const value = await original(input);
    if (obj(input).op === 'mobileSelectSavedEnvironment') {
      const status = obj(obj(value).value);
      f.client.adoptStatus(status, 2);
      if (change === 'new selection') expect((await mobileThreadSelection(f.client, 'one', f.native, f.storage)).message).toBe('');
      if (change === 'clear') await mobileCacheClear(f.native, { environmentId: f.b });
      if (change === 'replace') fleet.saved = fleet.saved.map(row => row.environmentId === f.b ? { ...row, origin: 'https://replacement.invalid' } : row);
      if (change === 'new generation') f.client.adoptStatus(status, 3);
      if (change === 'thread owner') f.client.threadEpoch++;
    }
    return value;
  };
  const outcome = await mobileThreadSelection(f.client, `fleet:${f.b}:two`, f.native, f.storage)
    .then(() => 'published', error => str(obj(error).kind));
  expect(outcome).toBe('superseded'); expect(f.writes).toHaveLength(change === 'new selection' ? 1 : 0);
  expect(f.client.threadId).toBe(change === 'new selection' ? 'one' : '');
});

test('reconnect during a cache read hands selection to the shared live reader', async () => {
  const f = fixture(); await f.sync();
  const original = f.native.later, liveShell = f.client.shell;
  const projection: Obj = { thread: { ...liveShell.threads.find(row => row.id === 'two'), title: 'Live thread' } };
  for (const family of ['runs', 'attempts', 'nodes', 'subagents', 'providerSessions', 'providerThreads', 'providerTurns', 'runtimeRequests',
    'messages', 'plans', 'turnItems', 'checkpointScopes', 'checkpoints', 'contextHandoffs', 'contextTransfers', 'visibleTurnItems']) projection[family] = [];
  let connected = false, liveReads = 0;
  f.native.later = async input => {
    const r = obj(input);
    if (r.op === 'devicePresentation') return { ok: true, generation: 2, value: {} };
    if (r.op === 'http') { liveReads++; return { ok: true, generation: 2, value: { snapshotSequence: 2, projection } }; }
    if (r.op === 'subscribe') return { ok: true, generation: 2, value: { id: 'live-thread-sub' } };
    const value = await original(input);
    if (!connected && r.action === 'read' && r.environmentId === f.b && f.client.environmentId === f.b) {
      connected = true;
      f.client.adoptStatus({ state: 'connected', origin: 'https://b.invalid', environmentId: f.b, message: '' }, 2);
      f.client.shell = liveShell; f.client.shellLoaded = true; f.client.shellLive = true;
    }
    return value;
  };
  expect((await mobileThreadSelection(f.client, `fleet:${f.b}:two`, f.native, f.storage)).message).toBe('');
  expect(liveReads).toBe(1); expect(f.client.threadId).toBe('two');
  expect(obj(f.client.thread?.projection.thread).title).toBe('Live thread');
  expect(f.client.threadLive).toBe(true); expect(f.client.subscriptions.thread).toBe('live-thread-sub');
  expect(f.writes).toHaveLength(1);
});

import { expect, test } from 'bun:test';
import { T3Client } from './shared/client';
import { initialShell, obj, type Obj } from './shared/domain';
import type { Native } from './shared/protocol';
import { mobileCacheAdoptThreadPresentation, mobileCacheBeforeStatus, mobileCacheSync } from './mobile-client-cache-sync';
import { mobileCacheClear } from './mobile-client-cache';
import { mobileThreadComposer } from './thread';
import { stage } from './shared/composer-controls';
import { encodeMobileConfigCache as configPayload, encodeMobileShellCache as shellPayload, encodeMobileThreadCache as threadPayload } from './mobile-client-cache-codec';

import { encodeMobileCatalogPayload } from './mobile-client-cache-catalog';
const identity = (environmentId: string) => JSON.stringify([environmentId, 'https://cache.invalid']);
const encodeMobileShellCache = (...args: Parameters<typeof shellPayload>) => encodeMobileCatalogPayload(identity(args[0]), shellPayload(...args));
const encodeMobileConfigCache = (...args: Parameters<typeof configPayload>) => encodeMobileCatalogPayload(identity(args[0]), configPayload(...args));
const encodeMobileThreadCache = (...args: Parameters<typeof threadPayload>) => encodeMobileCatalogPayload(identity(args[0]), threadPayload(...args));

let nextEnvironment = 0;
function fixture() {
  const client = new T3Client(), environmentId = `cache-sync-${++nextEnvironment}`, calls: Obj[] = [];
  client.environmentId = environmentId; client.origin = 'https://cache.invalid'; client.generation = 1;
  client.connection = 'disconnected'; client.threadId = 'thread'; client.projectId = 'project';
  let saved: Obj[] = [{ environmentId, origin: client.origin, enabled: true }];
  const shell = { sequence: 10, projects: [{ id: 'project', title: 'Cached project' }], threads: [{ id: 'thread', projectId: 'project', title: 'Cached thread' }] };
  const config: Obj = { environment: { environmentId, capabilities: { serverResolvedCommandContext: true } }, providers: [] };
  const projection: Obj = { thread: shell.threads[0] };
  for (const family of ['runs', 'attempts', 'nodes', 'subagents', 'providerSessions', 'providerThreads', 'providerTurns', 'runtimeRequests',
    'messages', 'plans', 'turnItems', 'checkpointScopes', 'checkpoints', 'contextHandoffs', 'contextTransfers', 'visibleTurnItems']) projection[family] = [];
  const thread = { projection, sequence: 20, historyCursor: 'opaque', hasMore: true, latestLocalTurnOrdinal: 100 };
  const payloads: Record<string, string> = { shell: encodeMobileShellCache(environmentId, shell),
    'server-config': encodeMobileConfigCache(environmentId, config), thread: encodeMobileThreadCache(environmentId, 'thread', thread) };
  const good = (value: unknown) => ({ ok: true, generation: 1, value });
  const native: Native = { available: true, watch() {}, async later(input) {
    const request = obj(input); calls.push(request);
    if (request.action === 'read') return good({ record: payloads[String(request.kind)] === undefined ? null : { environmentId, kind: request.kind, key: request.key,
      schemaVersion: 1, payload: payloads[String(request.kind)], updatedAt: 100 } });
    if (request.action === 'remove') {
      const kind = String(request.kind), removed = payloads[kind] === request.expectedPayload;
      if (removed) delete payloads[kind];
      return good({ removed: removed ? 1 : 0 });
    }
    if (request.action === 'ticket') return good({ ticket: 'ticket' });
    if (request.action === 'write') return good({ written: true, stale: false });
    if (request.action === 'clear') {
      const removed = Object.keys(payloads).length;
      for (const kind of Object.keys(payloads)) delete payloads[kind];
      return good({ removed });
    }
    throw new Error('Unexpected cache operation');
  } };
  return { client, environmentId, calls, shell, thread, config, native, payloads,
    catalog: () => saved, forget: () => { saved = []; },
    sync: () => mobileCacheSync(client, native, () => saved) };
}
function delay(f: ReturnType<typeof fixture>, action: string, kind: string) {
  const entered = Promise.withResolvers<void>(), finish = Promise.withResolvers<void>(), original = f.native.later;
  let intercepted = false;
  f.native.later = async input => {
    const request = obj(input);
    if (!intercepted && request.action === action && request.kind === kind) {
      intercepted = true; entered.resolve(); await finish.promise;
    }
    return original(input);
  };
  return { entered: entered.promise, resolve: () => finish.resolve() };
}
function connect(f: ReturnType<typeof fixture>) {
  const status = { state: 'connected', origin: f.client.origin, environmentId: f.environmentId, message: '' };
  mobileCacheBeforeStatus(f.client, status, 2);
  f.client.adoptStatus(status, 2);
}

test('offline adoption preserves read data and leaves every command authority false', async () => {
  const f = fixture(); await f.sync();
  expect(f.client.shell).toEqual(f.shell); expect(f.client.config).toEqual(f.config); expect(f.client.thread).toEqual(f.thread);
  expect(f.client.shellLoaded).toBe(true);
  expect(f.client.connection).toBe('disconnected'); expect(f.client.scopes).toEqual([]);
  expect(f.client.configLive).toBe(false); expect(f.client.shellLive).toBe(false); expect(f.client.threadLive).toBe(false);
  expect(f.client.ready).toBe(false); expect(f.client.writable).toBe(false); expect(f.client.subscriptions).toEqual({});
  const count = f.calls.length; await f.sync(); expect(f.calls).toHaveLength(count);
});

test('connected failed synchronization does not load cache as successful RPC', async () => {
  const f = fixture(); f.client.connection = 'connected'; f.client.error = 'Thread read permission denied.';
  await f.sync();
  expect(f.calls).toEqual([]); expect(f.client.shell).toEqual(initialShell()); expect(f.client.thread).toBeNull();
  expect(f.client.error).toBe('Thread read permission denied.'); expect(f.client.ready).toBe(false);
});

test('reconnect discards exact cached objects before shared synchronization', async () => {
  const f = fixture(); await f.sync(); connect(f);
  expect(f.client.shell).toEqual(initialShell()); expect(f.client.shellLoaded).toBe(false);
  expect(f.client.thread).toBeNull(); expect(f.client.config).toEqual({});
  expect(f.client.ready).toBe(false);
});

test('reconnect cache cleanup preserves a newer live object', async () => {
  const f = fixture(); await f.sync();
  const liveShell = { ...f.shell, sequence: 200 }, liveThread = { ...f.thread, sequence: 201 }, liveConfig = { ...f.config, cwd: '/new' };
  f.client.shell = liveShell; f.client.thread = liveThread; f.client.config = liveConfig;
  connect(f);
  expect(f.client.shell).toBe(liveShell); expect(f.client.thread).toBe(liveThread); expect(f.client.config).toBe(liveConfig);
  expect(f.client.shellLoaded).toBe(true);
});

test.each(['route', 'environment', 'generation', 'revision', 'catalog', 'reconnect'])('pending multi-read cannot adopt after %s changes', async change => {
  const f = fixture(), held = delay(f, 'read', 'server-config'), syncing = f.sync();
  await held.entered;
  // Shell arrived, but the grouped adoption has not committed any partial data.
  expect(f.client.shellLoaded).toBe(false); expect(f.client.shell).toEqual(initialShell());
  if (change === 'route') { f.client.threadId = 'other'; f.client.threadEpoch++; }
  if (change === 'environment') f.client.environmentId = 'other';
  if (change === 'generation') f.client.generation++;
  if (change === 'revision') f.client.revision++;
  if (change === 'catalog') f.forget();
  if (change === 'reconnect') f.client.connection = 'connected';
  held.resolve(); await syncing;
  expect(f.client.shell).toEqual(initialShell()); expect(f.client.thread).toBeNull(); expect(f.client.config).toEqual({});
});

test('clear invalidates a read already awaiting a later record', async () => {
  const f = fixture(), held = delay(f, 'read', 'server-config'), syncing = f.sync(); await held.entered;
  await mobileCacheClear(f.native, { environmentId: f.environmentId }); held.resolve(); await syncing;
  expect(f.client.shell).toEqual(initialShell()); expect(f.client.thread).toBeNull(); expect(f.client.config).toEqual({});
});

test('newer read owns adoption and late old completion cannot replace it', async () => {
  const f = fixture(), held = delay(f, 'read', 'server-config'), old = f.sync(); await held.entered;
  await f.sync(); const adoptedShell = f.client.shell, adoptedThread = f.client.thread, revision = f.client.revision;
  held.resolve(); await old;
  expect(f.client.shell).toBe(adoptedShell); expect(f.client.thread).toBe(adoptedThread); expect(f.client.revision).toBe(revision);
});

test.each(['failure', 'let-go'])('later read %s leaves no partial cached adoption', async failure => {
  const f = fixture(), original = f.native.later;
  f.native.later = async input => {
    if (obj(input).action === 'read' && obj(input).kind === 'server-config') {
      if (failure === 'let-go') throw { name: 'FetchError', kind: 'Aborted' };
      return { ok: false, generation: 1, error: { kind: 'Disk', message: 'Cache read failed' } };
    }
    return original(input);
  };
  if (failure === 'let-go') await expect(f.sync()).rejects.toMatchObject({ kind: 'superseded' });
  else await f.sync();
  expect(f.client.shell).toEqual(initialShell()); expect(f.client.thread).toBeNull(); expect(f.client.config).toEqual({});
  expect(f.client.error).toBe('');
});

test('clear discards old display on next status without deleting newly adopted objects', async () => {
  const f = fixture(); await f.sync();
  await mobileCacheClear(f.native, { environmentId: f.environmentId });
  mobileCacheBeforeStatus(f.client, { state: 'disconnected', origin: f.client.origin, environmentId: f.environmentId, message: '' }, f.client.generation);
  expect(f.client.shell).toEqual(initialShell()); expect(f.client.thread).toBeNull(); expect(f.client.config).toEqual({});
});

test('capture waits for a ticket and writes immutable live data only', async () => {
  const f = fixture(); f.client.connection = 'connected'; f.client.shellLive = true; f.client.shellLoaded = true; f.client.shell = f.shell;
  // Config/thread remain nonlive and must never be persisted.
  f.client.config = f.config; f.client.thread = f.thread;
  const held = delay(f, 'ticket', 'shell'), syncing = f.sync(); await held.entered;
  f.shell.projects[0]!.title = 'Latest before ticket'; held.resolve(); await syncing;
  const writes = f.calls.filter(call => call.action === 'write'); expect(writes).toHaveLength(1);
  const written = String(writes[0]!.payload);
  expect(JSON.parse(JSON.parse(written).payload).snapshot.projects[0].title).toBe('Latest before ticket');
  f.shell.projects[0]!.title = 'Changed after capture';
  expect(JSON.parse(JSON.parse(written).payload).snapshot.projects[0].title).toBe('Latest before ticket');
});

test('scope change while waiting for ticket refuses write', async () => {
  const f = fixture(); f.client.connection = 'connected'; f.client.shellLive = true; f.client.shellLoaded = true; f.client.shell = f.shell;
  const held = delay(f, 'ticket', 'shell'), syncing = f.sync(); await held.entered;
  f.client.generation++; held.resolve(); await syncing;
  expect(f.calls.filter(call => call.action === 'write')).toEqual([]);
});

test.each(['forgotten', 'replaced', 'ambiguous', 'cleared'])('retained offline display is discarded after catalog is %s', async change => {
  const f = fixture(); await f.sync();
  expect(f.client.shellLoaded).toBe(true);
  if (change === 'forgotten') f.forget();
  if (change === 'replaced') f.catalog()[0]!.origin = 'https://different.invalid';
  if (change === 'ambiguous') f.catalog().push({ ...f.catalog()[0]! });
  if (change === 'cleared') await mobileCacheClear(f.native, { environmentId: f.environmentId });
  // Refuse new reads so this assertion observes retention reconciliation only.
  await mobileCacheSync(f.client, null, f.catalog);
  expect(f.client.shell).toEqual(initialShell()); expect(f.client.shellLoaded).toBe(false);
  expect(f.client.thread).toBeNull(); expect(f.client.config).toEqual({});
});
test.each(['replaced', 'ambiguous'])('pending restore loses ownership when saved identity is %s', async change => {
  const f = fixture(), held = delay(f, 'read', 'server-config'), old = f.sync(); await held.entered;
  if (change === 'replaced') f.catalog()[0]!.origin = 'https://different.invalid';
  else f.catalog().push({ ...f.catalog()[0]! });
  held.resolve(); await old;
  expect(f.client.shellLoaded).toBe(false); expect(f.client.thread).toBeNull(); expect(f.client.config).toEqual({});
});
test('malformed status cannot discard a valid cached display', async () => {
  const f = fixture(); await f.sync(); const shell = f.client.shell;
  mobileCacheBeforeStatus(f.client, { state: 'connected' }, 2);
  expect(f.client.shell).toBe(shell); expect(f.client.shellLoaded).toBe(true);
});

test('successful deletion cleanup cannot be followed by saving the same deleted projection', async () => {
  const f = fixture(); f.client.connection = 'connected'; f.client.shell = f.shell; f.client.shellLoaded = true;
  f.client.config = f.config; f.client.thread = f.thread;
  f.client.shellLive = f.client.configLive = f.client.threadLive = true;
  f.thread.projection.thread = { ...obj(f.thread.projection.thread), deletedAt: '2026-10-08T00:00:00Z' };
  const original = f.native.later;
  f.native.later = async input => {
    if (obj(input).action === 'remove') { f.calls.push(obj(input)); return { ok: true, generation: 1, value: { removed: 1 } }; }
    return original(input);
  };
  await f.sync(); await f.sync();
  expect(f.calls.filter(call => call.action === 'remove')).toEqual([{ op: 'mobileClientCache', action: 'remove',
    environmentId: f.environmentId, kind: 'thread', key: 'thread' }]);
  expect(f.calls.some(call => call.kind === 'thread' && call.action === 'write')).toBe(false);
});


test.each(['replaced', 'forgotten', 'ambiguous'])('catalog %s clears durable bytes before a new display can read them', async change => {
  const f = fixture(); await f.sync();
  const original = { ...f.catalog()[0]! };
  if (change === 'replaced') f.catalog()[0]!.origin = 'https://replacement.invalid';
  if (change === 'forgotten') f.forget();
  if (change === 'ambiguous') f.catalog().push({ ...original, origin: 'https://ambiguous.invalid' });
  await f.sync();
  expect(f.client.shellLoaded).toBe(false); expect(f.client.thread).toBeNull(); expect(f.client.config).toEqual({});
  expect(f.calls.filter(call => call.action === 'clear')).toEqual([{ op: 'mobileClientCache', action: 'clear', environmentId: f.environmentId }]);
  expect(f.payloads).toEqual({});
  if (change === 'forgotten') f.catalog().push(original);
  if (change === 'ambiguous') f.catalog().pop();
  await f.sync(); expect(f.client.shellLoaded).toBe(false); expect(f.client.thread).toBeNull();
});

test('replacement clear failure blocks old disk bytes until a later acknowledged clear', async () => {
  const f = fixture(); await f.sync();
  f.catalog()[0]!.origin = 'https://replacement.invalid';
  const original = f.native.later;
  let fail = true;
  f.native.later = async request => {
    if (obj(request).action === 'clear' && fail) throw new Error('Disk is unavailable');
    return original(request);
  };
  const reads = f.calls.filter(call => call.action === 'read').length;
  await f.sync(); await f.sync();
  expect(Object.keys(f.payloads)).toHaveLength(3); expect(f.client.shellLoaded).toBe(false);
  expect(f.calls.filter(call => call.action === 'read')).toHaveLength(reads);
  fail = false; await f.sync();
  expect(f.payloads).toEqual({}); expect(f.client.shellLoaded).toBe(false);
});

test('pending restore and later retry cannot adopt bytes from a replaced saved identity', async () => {
  const f = fixture(), held = delay(f, 'read', 'server-config'), restoring = f.sync(); await held.entered;
  f.catalog()[0]!.origin = 'https://replacement.invalid'; held.resolve(); await restoring;
  expect(f.client.shellLoaded).toBe(false);
  await f.sync(); expect(f.payloads).toEqual({}); expect(f.client.shellLoaded).toBe(false);
});

test('an active learned route change preserves the unchanged saved identity cache', async () => {
  const f = fixture(); f.client.origin = 'https://learned-route.invalid'; await f.sync();
  expect(f.client.shellLoaded).toBe(true); expect(f.client.thread).toEqual(f.thread);
  expect(f.calls.some(call => call.action === 'clear')).toBe(false);
});

test('cached threads adopt their own composer selection and existing staged choices take precedence', async () => {
  const f = fixture();
  const first = { ...obj(f.thread.projection.thread), modelSelection: { instanceId: 'provider-a', model: 'model-a', options: [{ id: 'effort', value: 'low' }] },
    runtimeMode: 'full-access', interactionMode: 'plan' };
  f.thread.projection.thread = first;
  f.payloads.thread = encodeMobileThreadCache(f.environmentId, 'thread', f.thread);
  await f.sync();
  expect(mobileThreadComposer(f.client).modelLabel).toBe('model-a');
  expect(f.client.providerId).toBe('provider-a'); expect(f.client.modelOptions).toEqual([{ id: 'effort', value: 'low' }]);
  expect(f.client.runtimeMode).toBe('full-access'); expect(f.client.interactionMode).toBe('plan');
  const second = { ...f.thread, projection: { ...f.thread.projection, thread: { ...first, id: 'second',
    modelSelection: { instanceId: 'provider-b', model: 'model-b', options: [{ id: 'effort', value: 'high' }] },
    runtimeMode: 'approval-required', interactionMode: 'default' } } };
  f.client.shell.threads.push({ id: 'second', projectId: 'project', title: 'Second' });
  f.payloads.thread = encodeMobileThreadCache(f.environmentId, 'second', second);
  f.client.threadId = 'second'; f.client.threadEpoch++; f.client.thread = null;
  mobileCacheAdoptThreadPresentation(f.client, null); await f.sync();
  expect(mobileThreadComposer(f.client).modelLabel).toBe('model-b'); expect(f.client.providerId).toBe('provider-b');
  expect(f.client.modelOptions).toEqual([{ id: 'effort', value: 'high' }]);
  expect(f.client.runtimeMode).toBe('approval-required'); expect(f.client.interactionMode).toBe('default');
  stage(f.client, { modelId: 'staged-model', runtimeMode: 'full-access', interactionMode: 'plan' });
  f.client.thread = null; await f.sync();
  expect(mobileThreadComposer(f.client).modelLabel).toBe('staged-model');
  expect(f.client.runtimeMode).toBe('full-access'); expect(f.client.interactionMode).toBe('plan');
  f.client.threadId = 'uncached'; f.client.thread = null; mobileCacheAdoptThreadPresentation(f.client, null);
  expect(mobileThreadComposer(f.client).modelLabel).toBe(''); expect(f.client.modelOptions).toEqual([]);
  expect(f.client.runtimeMode).toBe('approval-required'); expect(f.client.interactionMode).toBe('default');
  expect(f.client.ready).toBe(false); expect(f.client.writable).toBe(false); expect(f.client.subscriptions).toEqual({});
});

test('warm catalog replacement cannot rewrite old live snapshots until a new adoption or generation', async () => {
  const f = fixture(); f.client.connection = 'connected'; f.client.shell = f.shell; f.client.shellLoaded = true;
  f.client.config = f.config; f.client.thread = f.thread; f.client.shellLive = f.client.configLive = f.client.threadLive = true;
  await f.sync(); const before = f.calls.filter(call => call.action === 'write').length;
  f.catalog()[0]!.origin = 'https://replacement.invalid'; await f.sync(); await f.sync();
  expect(f.calls.filter(call => call.action === 'write')).toHaveLength(before);
  expect(f.payloads).toEqual({});
  f.client.shell = { ...f.shell, sequence: 99 }; await f.sync();
  expect(f.calls.filter(call => call.action === 'write').slice(before).map(call => call.kind)).toEqual(['shell']);
  f.client.generation++; await f.sync();
  expect(f.calls.filter(call => call.action === 'write').slice(-3).map(call => call.kind)).toEqual(['shell', 'server-config', 'thread']);
});


test.each(['other identity', 'unwrapped'])('cold focused restore refuses %s disk provenance', async provenance => {
  const f = fixture();
  if (provenance === 'other identity') f.catalog()[0]!.origin = 'https://replacement.invalid';
  else {
    f.payloads.shell = shellPayload(f.environmentId, f.shell);
    f.payloads['server-config'] = configPayload(f.environmentId, f.config);
    f.payloads.thread = threadPayload(f.environmentId, 'thread', f.thread);
  }
  await f.sync();
  expect(f.client.shellLoaded).toBe(false); expect(f.client.thread).toBeNull(); expect(f.client.config).toEqual({});
  expect(f.payloads.shell).toBeUndefined(); expect(f.payloads['server-config']).toBeUndefined();
  expect(f.calls.filter(call => call.action === 'remove').map(call => call.kind)).toEqual(['shell', 'server-config']);
});

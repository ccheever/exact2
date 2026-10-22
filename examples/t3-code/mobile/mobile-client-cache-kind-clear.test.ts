import { expect, test } from 'bun:test';
import { T3Client } from './shared/client';
import { initialShell, obj, type Obj } from './shared/domain';
import type { Native } from './shared/protocol';
import { EnvironmentFleet, type FocusedHost } from './shared/settings-b-fleet';
import { encodeMobileCatalogPayload, mobileCacheCatalogIdentity } from './mobile-client-cache-catalog';
import { mobileCacheClearKind, type MobileCacheKind } from './mobile-client-cache';
import { encodeMobileConfigCache, encodeMobileShellCache, encodeMobileThreadCache } from './mobile-client-cache-codec';
import { mobileCacheBeforeStatus, mobileCacheSync } from './mobile-client-cache-sync';
import { mobileCacheFleetDisplays, mobileCacheFleetSync } from './mobile-client-cache-fleet';

let sequence = 0;
function fixture() {
  const environmentId = `kind-clear-${++sequence}`, origin = 'https://cache.invalid';
  const shell = { sequence: 1, projects: [{ id: 'project', title: 'Project' }],
    threads: [{ id: 'thread', projectId: 'project', title: 'Thread' }] };
  const config = { environment: { environmentId }, providers: [] };
  const projection: Obj = { thread: shell.threads[0] };
  for (const family of ['runs', 'attempts', 'nodes', 'subagents', 'providerSessions', 'providerThreads', 'providerTurns',
    'runtimeRequests', 'messages', 'plans', 'turnItems', 'checkpointScopes', 'checkpoints', 'contextHandoffs',
    'contextTransfers', 'visibleTurnItems']) projection[family] = [];
  const payloads = new Map<MobileCacheKind, string>([
    ['shell', encodeMobileShellCache(environmentId, shell)],
    ['server-config', encodeMobileConfigCache(environmentId, config)],
    ['thread', encodeMobileThreadCache(environmentId, 'thread', {
      projection, sequence: 1, historyCursor: null, hasMore: false, latestLocalTurnOrdinal: null,
    })],
  ]);
  const saved = [{ environmentId, origin, enabled: true }];
  for (const [kind, payload] of payloads) payloads.set(kind, encodeMobileCatalogPayload(mobileCacheCatalogIdentity(saved, environmentId), payload));
  const native: Native = { available: true, watch() {}, async later(input) {
    const request = obj(input), kind = request.kind as MobileCacheKind;
    if (request.action === 'read') {
      const payload = payloads.get(kind);
      return { ok: true, generation: 1, value: { record: payload === undefined ? null : {
        environmentId, kind, key: request.key, payload, schemaVersion: 1, updatedAt: 100,
      } } };
    }
    if (request.action === 'clearKind') return { ok: true, generation: 1,
      value: { removed: Number(payloads.delete(kind)) } };
    throw new Error('Unexpected cache operation');
  } };
  const client = new T3Client();
  client.environmentId = environmentId; client.origin = origin; client.generation = 1;
  client.connection = 'disconnected'; client.threadId = 'thread'; client.projectId = 'project';
  const fleet = new EnvironmentFleet(); fleet.saved = saved;
  const focused: FocusedHost = { environmentId: 'other', origin: 'https://other.invalid', connection: 'disconnected' };
  return { client, environmentId, native, fleet, focused,
    sync: () => mobileCacheSync(client, native, () => saved),
    fleetSync: () => mobileCacheFleetSync(fleet, native, focused),
    displays: () => mobileCacheFleetDisplays(fleet, focused) };
}

/** Capture the old native result before clear, then deliver it afterward. */
function holdRead(native: Native, kind: MobileCacheKind) {
  const entered = Promise.withResolvers<void>(), finish = Promise.withResolvers<void>(), original = native.later;
  native.later = async input => {
    const result = await original(input), request = obj(input);
    if (request.action === 'read' && request.kind === kind) { entered.resolve(); await finish.promise; }
    return result;
  };
  return { entered: entered.promise, release: () => finish.resolve() };
}

test.each(['shell', 'thread', 'server-config'] as const)('kind clear prevents focused adoption of an already returned %s record', async kind => {
  const f = fixture(), held = holdRead(f.native, kind), pending = f.sync();
  await held.entered;
  try { await mobileCacheClearKind(f.native, kind); } finally { held.release(); }
  await pending;
  expect(f.client.shell).toEqual(initialShell()); expect(f.client.shellLoaded).toBe(false);
  expect(f.client.thread).toBeNull(); expect(f.client.config).toEqual({});
  expect(f.client.ready).toBe(false); expect(f.client.writable).toBe(false);
});

test.each(['shell', 'thread', 'server-config'] as const)('kind clear discards a retained focused display after clearing %s', async kind => {
  const f = fixture(); await f.sync();
  expect(f.client.thread).not.toBeNull(); expect(f.client.shellLoaded).toBe(true);
  await mobileCacheClearKind(f.native, kind);
  mobileCacheBeforeStatus(f.client, { state: 'disconnected', origin: f.client.origin,
    environmentId: f.environmentId, message: '' }, f.client.generation);
  expect(f.client.shell).toEqual(initialShell()); expect(f.client.shellLoaded).toBe(false);
  expect(f.client.thread).toBeNull(); expect(f.client.config).toEqual({});
});

test.each(['shell', 'server-config'] as const)('kind clear prevents fleet adoption of an already returned %s record', async kind => {
  const f = fixture(), held = holdRead(f.native, kind), pending = f.fleetSync();
  await held.entered;
  try { await mobileCacheClearKind(f.native, kind); } finally { held.release(); }
  await pending;
  expect(f.displays()).toEqual([]); expect(f.fleet.entries.size).toBe(0);
});

test.each(['shell', 'server-config'] as const)('kind clear immediately hides retained fleet display after clearing %s', async kind => {
  const f = fixture(); await f.fleetSync(); expect(f.displays()).toHaveLength(1);
  await mobileCacheClearKind(f.native, kind);
  expect(f.displays()).toEqual([]); expect(f.fleet.entries.size).toBe(0);
});

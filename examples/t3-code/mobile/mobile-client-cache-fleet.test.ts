import { expect, test } from 'bun:test';
import { initialShell, obj, type Obj } from './shared/domain';
import { EnvironmentFleet, environmentKey, type FleetEntry, type FocusedHost } from './shared/settings-b-fleet';
import type { Native } from './shared/protocol';
import { mobileCacheClear } from './mobile-client-cache';
import { encodeMobileShellCache as shellPayload, encodeMobileConfigCache as configPayload } from './mobile-client-cache-codec';
import { mobileCacheFleetDisplays, mobileCacheFleetSync } from './mobile-client-cache-fleet';

import { encodeMobileCatalogPayload } from './mobile-client-cache-catalog';
const encodeMobileShellCache = (environmentId: string, shell: Parameters<typeof shellPayload>[1], origin = 'https://fleet.invalid') =>
  encodeMobileCatalogPayload(JSON.stringify([environmentId, origin]), shellPayload(environmentId, shell));
const encodeMobileConfigCache = (environmentId: string, config: Obj, origin = 'https://fleet.invalid') =>
  encodeMobileCatalogPayload(JSON.stringify([environmentId, origin]), configPayload(environmentId, config));

let next = 0;
function fixture() {
  const fleet = new EnvironmentFleet(), environmentId = `fleet-cache-${++next}`, origin = 'https://fleet.invalid';
  const focused: FocusedHost = { environmentId: 'focused', origin: 'https://focused.invalid', connection: 'disconnected' };
  const calls: Obj[] = [], payloads = new Map<string, string>();
  const shell = { sequence: 10, projects: [{ id: 'project', title: 'Saved project' }], threads: [{ id: 'thread', projectId: 'project', title: 'Saved thread' }] };
  const config = { environment: { environmentId, label: 'Saved server' }, providers: [] };
  payloads.set(`${environmentId}:shell`, encodeMobileShellCache(environmentId, shell));
  payloads.set(`${environmentId}:server-config`, encodeMobileConfigCache(environmentId, config));
  fleet.saved = [{ environmentId, origin, enabled: true }];
  const good = (value: unknown) => ({ ok: true, generation: 1, value });
  let epoch = 0;
  const native: Native = { available: true, watch() {}, async later(input) {
    const request = obj(input); calls.push(request);
    const cacheKey = `${request.environmentId}:${request.kind}`;
    if (request.action === 'read') {
      const payload = payloads.get(cacheKey);
      return good({ record: payload === undefined ? null : { environmentId: request.environmentId, kind: request.kind,
        key: request.key, payload, schemaVersion: 1, updatedAt: 100 } });
    }
    if (request.action === 'remove') {
      const removed = payloads.get(cacheKey) === request.expectedPayload;
      if (removed) payloads.delete(cacheKey);
      return good({ removed: removed ? 1 : 0 });
    }
    if (request.action === 'ticket') return good({ ticket: String(epoch) });
    if (request.action === 'write') {
      const written = request.ticket === String(epoch);
      if (written) payloads.set(cacheKey, String(request.payload));
      return good({ written, stale: !written });
    }
    if (request.action === 'clear') {
      epoch++;
      for (const key of payloads.keys()) if (!request.environmentId || key.startsWith(`${request.environmentId}:`)) payloads.delete(key);
      return good({ removed: 2 });
    }
    throw new Error('Unexpected request');
  } };
  const entry: FleetEntry = { key: environmentKey(origin, environmentId), origin, environmentId, phase: 'available',
    message: '', traceId: '', generation: 3, synchronized: -1, lastEvent: 0, subscriptions: {}, config: {}, shell: initialShell(),
    scopes: [], error: '', requested: false };
  fleet.entries.set(entry.key, entry);
  return { fleet, focused, environmentId, origin, calls, payloads, shell, config, native, entry,
    sync: () => mobileCacheFleetSync(fleet, native, focused), display: () => mobileCacheFleetDisplays(fleet, focused),
    connect() { entry.phase = 'connected'; entry.synchronized = entry.generation; entry.shell = shell; entry.config = config; } };
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

test('background offline display never mutates fleet transport or reducer authority', async () => {
  const f = fixture(), before = JSON.stringify(f.entry); await f.sync();
  expect(f.display()).toEqual([{ environmentId: f.environmentId, origin: f.origin, enabled: true, connected: false, shell: f.shell, config: f.config }]);
  expect(JSON.stringify(f.entry)).toBe(before); expect(f.fleet.revision).toBe(0);
  const calls = f.calls.length; await f.sync(); expect(f.calls).toHaveLength(calls);
});

test('disabled environment keeps display data without starting a transport; forget removes visibility', async () => {
  const f = fixture(); f.fleet.saved[0]!.enabled = false; f.fleet.entries.clear(); await f.sync();
  expect(f.display()[0]!.enabled).toBe(false); expect(f.fleet.entries.size).toBe(0);
  f.fleet.saved[0]!.enabled = true; expect(f.display()[0]!.enabled).toBe(true);
  f.fleet.saved = []; expect(f.display()).toEqual([]);
  expect(f.calls.every(call => call.action === 'read')).toBe(true);
});

test('focus owns its environment and connected unsynchronized/denied entries suppress cached display', async () => {
  const f = fixture(); await f.sync();
  f.focused.environmentId = f.environmentId; expect(f.display()).toEqual([]);
  f.focused.environmentId = 'focused'; f.entry.phase = 'connected'; f.entry.error = 'Permission denied';
  const count = f.calls.length; await f.sync(); expect(f.display()).toEqual([]); expect(f.calls).toHaveLength(count);
  expect(f.entry.error).toBe('Permission denied'); expect(f.entry.synchronized).toBe(-1);
});

test.each(['catalog', 'forgotten', 'origin', 'disabled', 'generation', 'live', 'shell', 'config', 'entry', 'revision', 'focus'])
  ('pending hydration loses ownership after %s change', async change => {
    const f = fixture(), held = delay(f, 'read', 'server-config'), pending = f.sync(); await held.entered;
    expect(f.display()).toEqual([]);
    if (change === 'catalog') f.fleet.saved = [...f.fleet.saved];
    if (change === 'forgotten') f.fleet.saved = [];
    if (change === 'origin') f.fleet.saved[0]!.origin = 'https://other.invalid';
    if (change === 'disabled') f.fleet.saved[0]!.enabled = false;
    if (change === 'generation') f.entry.generation++;
    if (change === 'live') f.connect();
    if (change === 'shell') f.entry.shell = f.shell;
    if (change === 'config') f.entry.config = f.config;
    if (change === 'entry') f.fleet.entries.set(f.entry.key, { ...f.entry });
    if (change === 'revision') f.fleet.revision++;
    if (change === 'focus') f.focused.environmentId = f.environmentId;
    held.resolve(); await pending; expect(f.display()).toEqual([]);
  });

test.each(['environment', 'all'])('clear %s invalidates pending reads and already retained display', async scope => {
  const f = fixture(), held = delay(f, 'read', 'server-config'), pending = f.sync(); await held.entered;
  await mobileCacheClear(f.native, scope === 'environment' ? { environmentId: f.environmentId } : {});
  held.resolve(); await pending; expect(f.display()).toEqual([]);
  f.payloads.set(`${f.environmentId}:shell`, encodeMobileShellCache(f.environmentId, f.shell));
  await f.sync(); expect(f.display()).toHaveLength(1);
  await mobileCacheClear(f.native, scope === 'environment' ? { environmentId: f.environmentId } : {});
  expect(f.display()).toEqual([]);
});

test('newer hydration owns the view and earlier completion cannot replace it', async () => {
  const f = fixture(), held = delay(f, 'read', 'server-config'), old = f.sync(); await held.entered;
  f.payloads.set(`${f.environmentId}:shell`, encodeMobileShellCache(f.environmentId, { ...f.shell, sequence: 22 }));
  await f.sync(); expect(f.display()[0]!.shell.sequence).toBe(22);
  held.resolve(); await old; expect(f.display()[0]!.shell.sequence).toBe(22);
});

test('only synchronized live entries persist, after ticket, as immutable values', async () => {
  const f = fixture(); f.connect();
  const held = delay(f, 'ticket', 'shell'), pending = f.sync(); await held.entered;
  f.shell.projects[0]!.title = 'Latest capture'; held.resolve(); await pending;
  const writes = f.calls.filter(call => call.action === 'write'); expect(writes).toHaveLength(2);
  expect(JSON.parse(JSON.parse(String(writes[0]!.payload)).payload).snapshot.projects[0].title).toBe('Latest capture');
  f.shell.projects[0]!.title = 'Later mutation';
  expect(JSON.parse(JSON.parse(String(writes[0]!.payload)).payload).snapshot.projects[0].title).toBe('Latest capture');
  expect(f.display()).toEqual([]);
});

test.each(['generation', 'catalog', 'busy', 'clear'])('ticket await refuses a changed %s write owner', async change => {
  const f = fixture(); f.connect();
  const held = delay(f, 'ticket', 'shell'), pending = f.sync(); await held.entered;
  if (change === 'generation') f.entry.generation++;
  if (change === 'catalog') f.fleet.saved = [];
  if (change === 'busy') f.entry.busy = true;
  if (change === 'clear') await mobileCacheClear(f.native, { environmentId: f.environmentId });
  held.resolve(); await pending;
  expect(f.calls.filter(call => call.action === 'write')).toEqual([]);
});

test('native epoch rejects a captured write arriving after clear', async () => {
  const f = fixture(); f.connect(); const held = delay(f, 'write', 'shell'), pending = f.sync(); await held.entered;
  await mobileCacheClear(f.native, { environmentId: f.environmentId }); held.resolve(); await pending;
  expect(f.payloads.size).toBe(0); expect(f.calls.filter(call => call.action === 'write')).toHaveLength(1);
});

test('live snapshot replaces earlier cached view after disconnect', async () => {
  const f = fixture(); await f.sync(); f.connect(); f.entry.shell = { ...f.shell, sequence: 99 };
  await f.sync(); f.entry.phase = 'reconnecting'; f.entry.synchronized = -1;
  await f.sync(); expect(f.display()[0]!.shell.sequence).toBe(99);
});

test('same thread IDs remain isolated across environments and one failure does not break the other', async () => {
  const f = fixture(), other = `${f.environmentId}-other`;
  f.fleet.saved.push({ environmentId: other, origin: 'https://other.invalid' });
  f.payloads.set(`${other}:shell`, encodeMobileShellCache(other, { ...f.shell, sequence: 77 }, 'https://other.invalid'));
  f.payloads.set(`${f.environmentId}:shell`, 'corrupt');
  await f.sync(); expect(f.payloads.has(`${f.environmentId}:shell`)).toBe(false);
  expect(f.display()).toHaveLength(1); expect(f.display()[0]!.environmentId).toBe(other);
  expect(f.display()[0]!.shell.threads[0]!.id).toBe('thread'); expect(f.display()[0]!.config).toEqual({});
});

test('let-go during grouped hydration propagates without adopting or calling another environment', async () => {
  const f = fixture(), original = f.native.later;
  f.fleet.saved.push({ environmentId: 'other', origin: 'https://other.invalid' });
  f.native.later = async input => {
    if (obj(input).kind === 'server-config') throw { name: 'FetchError', kind: 'Aborted' };
    return original(input);
  };
  await expect(f.sync()).rejects.toMatchObject({ kind: 'superseded' });
  expect(f.display()).toEqual([]); expect(f.calls).toHaveLength(1);
});

test('ambiguous catalog identities and unavailable native never read or persist', async () => {
  const f = fixture(); f.fleet.saved.push({ ...f.fleet.saved[0], origin: 'https://other.invalid' });
  await f.sync(); expect(f.calls).toEqual([]); expect(f.display()).toEqual([]);
  f.fleet.saved.pop(); f.native.available = false; await f.sync(); expect(f.calls).toEqual([]);
});


test.each(['replacement', 'forget', 'ambiguous'])('fleet catalog %s deletes old bytes even after synchronous projection dropped display', async change => {
  const f = fixture(); await f.sync(); const original = { ...f.fleet.saved[0]! };
  if (change === 'replacement') f.fleet.saved[0]!.origin = 'https://replacement.invalid';
  if (change === 'forget') f.fleet.saved = [];
  if (change === 'ambiguous') f.fleet.saved.push({ ...original, origin: 'https://other.invalid' });
  expect(f.display()).toEqual([]); await f.sync();
  expect(f.payloads.size).toBe(0); expect(f.display()).toEqual([]);
  expect(f.calls.filter(call => call.action === 'clear')).toEqual([{ op: 'mobileClientCache', action: 'clear', environmentId: f.environmentId }]);
  if (change === 'forget') f.fleet.saved = [original];
  if (change === 'ambiguous') f.fleet.saved.pop();
  await f.sync(); expect(f.display()).toEqual([]);
});

test('fleet replacement retains a failed-clear block and retries without rereading old payloads', async () => {
  const f = fixture(); await f.sync(); f.fleet.saved[0]!.origin = 'https://replacement.invalid';
  const original = f.native.later; let fail = true;
  f.native.later = async request => {
    if (obj(request).action === 'clear' && fail) throw new Error('Disk is unavailable');
    return original(request);
  };
  const reads = f.calls.filter(call => call.action === 'read').length;
  await f.sync(); await f.sync(); expect(f.payloads.size).toBe(2); expect(f.display()).toEqual([]);
  expect(f.calls.filter(call => call.action === 'read')).toHaveLength(reads);
  fail = false; await f.sync(); expect(f.payloads.size).toBe(0); expect(f.display()).toEqual([]);
});

test('fleet pending old read and a new pass after replacement cannot resurrect old disk data', async () => {
  const f = fixture(), held = delay(f, 'read', 'server-config'), old = f.sync(); await held.entered;
  f.fleet.saved[0]!.origin = 'https://replacement.invalid'; held.resolve(); await old;
  await f.sync(); expect(f.payloads.size).toBe(0); expect(f.display()).toEqual([]);
});

test('warm fleet replacement cannot rewrite previous live objects until a fresh adoption or generation', async () => {
  const f = fixture(); f.connect(); await f.sync(); const before = f.calls.filter(call => call.action === 'write').length;
  f.fleet.saved[0]!.origin = 'https://replacement.invalid'; f.entry.origin = 'https://replacement.invalid';
  await f.sync(); await f.sync();
  expect(f.payloads.size).toBe(0); expect(f.calls.filter(call => call.action === 'write')).toHaveLength(before);
  f.entry.shell = { ...f.shell, sequence: 99 }; await f.sync();
  expect(f.calls.filter(call => call.action === 'write').slice(before).map(call => call.kind)).toEqual(['shell']);
  f.entry.generation++; f.entry.synchronized = f.entry.generation; await f.sync();
  expect(f.calls.filter(call => call.action === 'write').slice(-2).map(call => call.kind)).toEqual(['shell', 'server-config']);
});


test.each(['other identity', 'unwrapped'])('cold fleet restore refuses %s disk provenance', async provenance => {
  const f = fixture();
  if (provenance === 'other identity') f.fleet.saved[0]!.origin = 'https://replacement.invalid';
  else f.payloads.set(`${f.environmentId}:shell`, shellPayload(f.environmentId, f.shell));
  await f.sync(); expect(f.display()).toEqual([]);
  expect(f.payloads.has(`${f.environmentId}:shell`)).toBe(false);
  expect(f.calls.filter(call => call.action === 'remove').map(call => call.kind)).toEqual(['shell']);
});

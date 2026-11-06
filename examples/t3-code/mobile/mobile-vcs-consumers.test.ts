import { expect, test } from 'bun:test';
import { mobileVcsCachedRead, mobileVcsRequest, mobileVcsDisplayOwner, type MobileVcsScope,
  MOBILE_VCS_INVALIDATING_METHODS } from './mobile-vcs-consumers';
import { mobileVcsCache } from './mobile-vcs-cache';
import { mobileCacheClear } from './mobile-client-cache';
import { obj, type Obj } from './shared/domain';
import { ClientError, type Native } from './shared/protocol';
import { MobileDraftClient } from './mobile-draft-recovery';
import { fleet } from './shared/settings-b-fleet';
import { settingsCall, settingsEndpoint } from './settings-server-source';

let fixtureSerial = 0;
const refs = () => ({ refs: [{ name: 'main', current: true, isDefault: true, worktreePath: '/repo' }],
  isRepo: true, hasPrimaryRemote: true, nextCursor: null, totalCount: 1 });
const input = { cwd: '/repo', limit: 100 };
function fixture() {
  const environmentId = `vcs-consumer-${++fixtureSerial}`, calls: Obj[] = [], rows = new Map<string, Obj>();
  let saved = [{ environmentId, origin: 'https://one.test' }], current = true, epoch = 0;
  const control: { fail?: string; hook?: (request: Obj) => Promise<void> } = {};
  const key = (request: Obj) => JSON.stringify([request.environmentId, request.kind, request.key]);
  const native: Native = { available: true, watch() {}, async later(raw) {
    const request = obj(raw); calls.push(request); await control.hook?.(request);
    if (control.fail && request.action === control.fail) throw new Error('cache failed');
    let value: Obj = {};
    if (request.action === 'ticket') value = { ticket: String(epoch) };
    else if (request.action === 'write') {
      const stale = request.ticket !== String(epoch);
      if (!stale) rows.set(key(request), { environmentId: request.environmentId, kind: request.kind, key: request.key,
        payload: request.payload, schemaVersion: 1, updatedAt: 10 });
      value = { written: !stale, stale };
    } else if (request.action === 'read') value = { record: rows.get(key(request)) ?? null };
    else if (request.action === 'remove') {
      const row = rows.get(key(request)); const removed = row?.payload === request.expectedPayload ? 1 : 0;
      if (removed) rows.delete(key(request)); value = { removed };
    } else if (request.action === 'clear') {
      epoch++; let removed = 0;
      for (const [id, row] of rows) if (row.environmentId === request.environmentId
        && (request.kind === undefined || request.kind === row.kind)) { removed++; rows.delete(id); }
      value = { removed };
    } else if (request.op === 'request') value = request.method === 'vcs.listRefs' ? refs() : { changed: true };
    return { ok: true, generation: 0, value };
  } };
  const scope: MobileVcsScope = { owner: {}, environmentId, saved: () => saved, current: () => current };
  const request = (method = 'vcs.listRefs', payload: Obj = input, send = async () => {
    calls.push({ real: method }); return refs();
  }) => mobileVcsRequest(scope, native, method, payload, send);
  return { scope, native, calls, rows, control, request, offline: () => mobileVcsCachedRead(scope, native, input),
    replace: () => { saved = [{ environmentId, origin: 'https://replacement.test' }]; }, leave: () => { current = false; } };
}

test('actual request runs once, returns original fields and persists catalog-bound refs', async () => {
  const f = fixture(), original = { ...refs(), futureField: 'keep' }; let sent = 0;
  expect(await f.request('vcs.listRefs', input, async () => { sent++; return original; })).toBe(original);
  expect(sent).toBe(1); expect(f.calls.map(row => row.action)).toEqual(['ticket', 'write']);
  const stored = JSON.parse(String([...f.rows.values()][0]!.payload));
  expect(stored.catalogIdentity).toBe(JSON.stringify([f.scope.environmentId, 'https://one.test']));
  expect((await f.offline())?.refs).toEqual(refs());
});
test('cache phase sends no server/auth/status requests and cannot confer authority', async () => {
  const f = fixture(); await f.request(); f.calls.length = 0;
  expect((await f.offline())?.source).toBe('cache');
  expect(f.calls).toHaveLength(1); expect(f.calls[0]?.op).toBe('mobileClientCache');
  expect(f.calls[0]?.action).toBe('read');
});
test('filtered and paginated reads remain actual single requests with no durable replacement', async () => {
  const f = fixture(); await f.request(); const before = [...f.rows.values()][0]!.payload;
  for (const patch of [{ query: 'main' }, { cursor: 0 }, { includeMatchingRemoteRefs: true }, { refKind: 'local' }]) {
    let sends = 0; f.calls.length = 0;
    expect(await f.request('vcs.listRefs', { ...input, ...patch }, async () => { sends++; return refs(); })).toEqual(refs());
    expect(sends).toBe(1); expect(f.calls).toEqual([]);
  }
  expect([...f.rows.values()][0]!.payload).toBe(before);
});
test('cold same-ID catalog replacement rejects and conditionally removes old-home rows', async () => {
  const f = fixture(); await f.request(); f.replace();
  const cold = { ...f.scope, owner: {} };
  expect(await mobileVcsCachedRead(cold, f.native, input)).toBeNull();
  expect(f.rows.size).toBe(0); expect(f.calls.at(-1)?.action).toBe('remove');
});
test('catalog replacement and route departure during a real request prevent persistence', async () => {
  for (const transition of ['replace', 'leave'] as const) {
    const f = fixture(), entered = Promise.withResolvers<void>(), release = Promise.withResolvers<void>();
    const pending = f.request('vcs.listRefs', input, async () => { entered.resolve(); await release.promise; return refs(); });
    await entered.promise; f[transition](); release.resolve();
    await expect(pending).rejects.toMatchObject({ kind: 'superseded' }); expect(f.rows.size).toBe(0);
  }
});
test('all seven source settled mutation methods invalidate, preserving original values and errors', async () => {
  for (const method of MOBILE_VCS_INVALIDATING_METHODS) {
    const f = fixture(); await f.request(); let sends = 0;
    const result = { changed: true };
    expect(await f.request(method, { cwd: '/repo' }, async () => { sends++; return result; })).toBe(result);
    expect(sends).toBe(1); expect(f.rows.size).toBe(0);
    await f.request();
    const uncertain = new ClientError('server may have applied it', 'transport', true);
    await expect(f.request(method, { cwd: '/repo' }, async () => { throw uncertain; })).rejects.toBe(uncertain);
    expect(f.rows.size).toBe(0);
  }
});
test('failed disposable clear never converts a real mutation outcome into a failure', async () => {
  const f = fixture(); await f.request(); f.control.fail = 'clear';
  expect(await f.request('vcs.createRef', {}, async () => ({ made: true }))).toEqual({ made: true });
  const failure = new Error('real failure');
  await expect(f.request('vcs.switchRef', {}, async () => { throw failure; })).rejects.toBe(failure);
  expect(await f.offline()).toBeNull();
  f.control.fail = undefined; expect(await f.offline()).toBeNull(); expect(f.rows.size).toBe(0);
});
test('let-go mutation marks unreadable without another native operation; next owner recovers first', async () => {
  const f = fixture(); await f.request(); const before = f.calls.length;
  const abandoned = { name: 'FetchError', kind: 'Aborted' };
  await expect(f.request('vcs.pull', {}, async () => { throw abandoned; })).rejects.toBe(abandoned);
  expect(f.calls.length).toBe(before);
  expect(await f.offline()).toBeNull(); expect(f.calls[before]?.action).toBe('clear'); expect(f.rows.size).toBe(0);
});
test('answer lost during settled cleanup propagates let-go rather than a stale earlier failure', async () => {
  const f = fixture(); await f.request();
  f.control.hook = async request => { if (request.action === 'clear') throw { name: 'FetchError', kind: 'Aborted' }; };
  await expect(f.request('vcs.pull', {}, async () => { throw new Error('earlier live failure'); })).rejects.toMatchObject({ kind: 'superseded' });
  const count = f.calls.length; expect(f.calls.at(-1)?.action).toBe('clear');
  f.control.hook = undefined; expect(await f.offline()).toBeNull(); expect(f.calls[count]?.action).toBe('clear');
});
test('cache clear or settled mutation supersedes an in-flight list without another request', async () => {
  for (const invalidation of ['clear', 'mutation']) {
    const f = fixture(), entered = Promise.withResolvers<void>(), release = Promise.withResolvers<void>(); let requests = 0;
    const pending = f.request('vcs.listRefs', input, async () => { requests++; entered.resolve(); await release.promise; return refs(); });
    await entered.promise;
    if (invalidation === 'clear') await mobileCacheClear(f.native, { environmentId: f.scope.environmentId });
    else await f.request('vcs.init', {}, async () => ({ initialized: true }));
    release.resolve(); await expect(pending).rejects.toMatchObject({ kind: 'superseded' });
    expect(requests).toBe(1); expect(f.rows.size).toBe(0);
  }
});
test('display ownership changes synchronously on matching clear admission', async () => {
  const f = fixture(); await f.request(); const before = mobileVcsDisplayOwner(f.scope, input);
  const clearing = mobileCacheClear(f.native, { environmentId: f.scope.environmentId, kind: 'vcs-refs' });
  expect(mobileVcsDisplayOwner(f.scope, input)).not.toBe(before); await clearing;
});
test('actual MobileDraftClient.restAccess reaches the adapter once without changing shared consumers', async () => {
  const f = fixture(), saved = fleet.saved, client = new MobileDraftClient();
  fleet.saved = [...f.scope.saved()];
  Object.assign(client, { environmentId: f.scope.environmentId, origin: 'https://one.test', generation: 0, connection: 'connected' });
  try {
    const value = await client.restAccess(f.native).request('vcs.listRefs', input);
    expect(value).toEqual(refs()); expect(f.calls.filter(row => row.method === 'vcs.listRefs')).toHaveLength(1);
    expect(f.rows.size).toBe(1);
    await client.restAccess(f.native).request('vcs.refreshStatus', { cwd: '/repo' });
    expect(f.calls.filter(row => row.method === 'vcs.refreshStatus')).toHaveLength(1); expect(f.rows.size).toBe(0);
  } finally { fleet.saved = saved; }
});
test('actual fleet settings endpoint persists and invalidates without focused double interception', async () => {
  const f = fixture(), saved = fleet.saved, key = `vcs-test-${f.scope.environmentId}`;
  fleet.saved = [...f.scope.saved()];
  const source = { key, environmentId: f.scope.environmentId, focused: false, enabled: true, phase: 'connected',
    origin: 'https://one.test' } as Parameters<typeof settingsEndpoint>[0];
  fleet.entries.set(key, { generation: 0, phase: 'connected', environmentId: source.environmentId } as never);
  try {
    const endpoint = settingsEndpoint(source, f.native);
    expect(await settingsCall(endpoint, { op: 'request', method: 'vcs.listRefs', payload: input })).toEqual(refs());
    expect(f.calls.filter(row => row.method === 'vcs.listRefs')).toHaveLength(1);
    expect(f.calls.find(row => row.method === 'vcs.listRefs')?.fleet).toBe(key); expect(f.rows.size).toBe(1);
    await settingsCall(endpoint, { op: 'request', method: 'vcs.createRef', payload: { cwd: '/repo', refName: 'other' } });
    expect(f.calls.filter(row => row.method === 'vcs.createRef')).toHaveLength(1); expect(f.rows.size).toBe(0);
  } finally { fleet.saved = saved; fleet.entries.delete(key); }
});

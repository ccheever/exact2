import { expect, test } from 'bun:test';
import { MobileVcsCache, decodeMobileVcsRefs, decodeMobileVcsRefsCache, mobileVcsRefsCacheEligible } from './mobile-vcs-cache';
import { mobileCacheClear } from './mobile-client-cache';
import { ClientError, type Native } from './shared/protocol';
import { obj, type Obj } from './shared/domain';
const refs = () => ({ refs: [{ name: 'main', current: true, isDefault: true, worktreePath: '/repo' },
  { name: 'origin/feature', current: false, isDefault: false, worktreePath: null, isRemote: true, remoteName: 'origin' }],
  isRepo: true, hasPrimaryRemote: true, nextCursor: null, totalCount: 2 });
const input = { cwd: '/repo', limit: 100 };
const deferred = () => Promise.withResolvers<void>();
function fixture() {
  const cache = new MobileVcsCache(), calls: Obj[] = [], rows = new Map<string, Obj>(); let epoch = 0;
  const controls: { fail?: string; before?: (request: Obj) => Promise<void>; after?: (request: Obj) => Promise<void> } = {};
  const identity = (row: Obj) => JSON.stringify([row.environmentId, row.kind, row.key]);
  const native: Native = { available: true, watch() {}, async later(raw) {
    const request = obj(raw); calls.push(request); await controls.before?.(request);
    if (controls.fail === request.action) throw new Error('disk failed');
    let value: Obj = {};
    if (request.action === 'ticket') value = { ticket: String(epoch) };
    else if (request.action === 'read') value = { record: rows.get(identity(request)) ?? null };
    else if (request.action === 'write') {
      const stale = request.ticket !== String(epoch);
      if (!stale) rows.set(identity(request), { environmentId: request.environmentId, kind: request.kind, key: request.key,
        payload: request.payload, schemaVersion: request.schemaVersion, updatedAt: 10 });
      value = { written: !stale, stale };
    } else if (request.action === 'remove') {
      const row = rows.get(identity(request));
      const removed = row && row.payload === request.expectedPayload ? 1 : 0;
      if (removed) rows.delete(identity(request));
      value = { removed };
    } else if (request.action === 'clear') {
      epoch++; let removed = 0;
      for (const [key, row] of rows) if ((!request.environmentId || request.environmentId === row.environmentId)
        && (!request.kind || request.kind === row.kind)) { rows.delete(key); removed++; }
      value = { removed };
    }
    await controls.after?.(request); return { ok: true, generation: 0, value };
  } };
  const read = (extra: Obj = {}) => cache.read({ native, environmentId: 'env-a', input, current: () => true, ...extra });
  return { cache, native, calls, rows, controls, read };
}
test('only unfiltered initial100 is cache eligible; refresh does not change eligibility', () => {
  expect(mobileVcsRefsCacheEligible(input)).toBe(true);
  expect(mobileVcsRefsCacheEligible({ ...input, refresh: true })).toBe(true);
  for (const patch of [{ limit: undefined }, { limit: 50 }, { query: '' }, { query: 'main' }, { cursor: 0 },
    { includeMatchingRemoteRefs: false }, { refKind: 'all' }, { refKind: null }])
    expect(mobileVcsRefsCacheEligible({ ...input, ...patch })).toBe(false);
});
test('source result codec validates all fields and matching schema, environment and exact cwd', () => {
  const value = refs(); expect(decodeMobileVcsRefs(value)).toEqual(value);
  const payload = JSON.stringify({ schemaVersion: 1, environmentId: 'env-a', cwd: '/repo', refs: value });
  expect(decodeMobileVcsRefsCache(payload, 'env-a', '/repo')).toEqual(value);
  for (const [environment, cwd] of [['env-b', '/repo'], ['env-a', '/repo/']])
    expect(decodeMobileVcsRefsCache(payload, environment!, cwd!)).toBeNull();
  expect(decodeMobileVcsRefsCache(payload.replace('"schemaVersion":1', '"schemaVersion":3'), 'env-a', '/repo')).toBeNull();
  expect(decodeMobileVcsRefsCache('{', 'env-a', '/repo')).toBeNull();
  for (const patch of [{ totalCount: -1 }, { nextCursor: -1 }, { isRepo: 1 }, { hasPrimaryRemote: undefined }, { refs: null }])
    expect(decodeMobileVcsRefs({ ...value, ...patch })).toBeNull();
  for (const patch of [{ name: '   ' }, { current: undefined }, { isDefault: undefined }, { worktreePath: undefined },
    { worktreePath: '' }, { isRemote: 1 }, { remoteName: '' }])
    expect(decodeMobileVcsRefs({ ...value, refs: [{ ...value.refs[0], ...patch }] })).toBeNull();
});
test('live persists exact source envelope; a fresh owner reads offline without granting live authority', async () => {
  const f = fixture(); expect(await f.read({ liveRead: async () => refs() })).toEqual({ source: 'live', refs: refs() });
  expect(f.calls.map(row => row.action)).toEqual(['ticket', 'write']);
  expect(JSON.parse(String([...f.rows.values()][0]!.payload))).toEqual({ schemaVersion: 1, environmentId: 'env-a', cwd: '/repo', refs: refs() });
  expect(await new MobileVcsCache().read({ native: f.native, environmentId: 'env-a', input, current: () => true })).toEqual({ source: 'cache', refs: refs() });
  expect(await f.read({ environmentId: 'env-b' })).toBeNull();
  expect(await f.read({ input: { ...input, cwd: '/repo/' } })).toBeNull();
});
test('filtered or paginated live reads never replace the offline list', async () => {
  const f = fixture(); await f.read({ liveRead: async () => refs() }); const old = [...f.rows.values()][0]!.payload;
  for (const patch of [{ query: 'a' }, { cursor: 0 }, { includeMatchingRemoteRefs: false }, { refKind: 'local' }, { limit: 200 }]) {
    const count = f.calls.length;
    const next = { ...refs(), refs: [] };
    expect(await f.read({ input: { ...input, ...patch }, liveRead: async () => next })).toEqual({ source: 'live', refs: next });
    expect(await f.read({ input: { ...input, ...patch } })).toBeNull(); expect(f.calls.length).toBe(count);
  }
  expect([...f.rows.values()][0]!.payload).toBe(old);
});
test('actual denial propagates without loading cache; storage failures do not break valid live data', async () => {
  const f = fixture(); await f.read({ liveRead: async () => refs() });
  const denial = new ClientError('Read denied', 'permission');
  await expect(f.read({ liveRead: async () => { throw denial; } })).rejects.toBe(denial);
  expect(f.calls.some(row => row.action === 'read')).toBe(false);
  for (const action of ['ticket', 'write']) { f.controls.fail = action; expect((await f.read({ liveRead: async () => refs() }))?.source).toBe('live'); }
});
test('mutation while live request waits invalidates its result and old ticket', async () => {
  const f = fixture(), entered = deferred(), release = deferred();
  const pending = f.read({ liveRead: async () => { entered.resolve(); await release.promise; return refs(); } });
  await entered.promise; await f.cache.invalidate(f.native, 'env-a'); release.resolve();
  expect(await pending).toBeNull(); expect(f.calls.map(row => row.action)).toEqual(['ticket', 'clear']); expect(f.rows.size).toBe(0);
});
test('clear while a native write waits rejects the pre-clear ticket and display result', async () => {
  const f = fixture(), entered = deferred(), release = deferred();
  f.controls.before = async row => { if (row.action === 'write') { entered.resolve(); await release.promise; } };
  const pending = f.read({ liveRead: async () => refs() }); await entered.promise;
  await f.cache.invalidate(f.native, 'env-a'); release.resolve();
  expect(await pending).toBeNull(); expect(f.rows.size).toBe(0);
});
test('write that reaches storage before clear is removed even when its reply is delayed', async () => {
  const f = fixture(), entered = deferred(), release = deferred();
  f.controls.after = async row => { if (row.action === 'write') { entered.resolve(); await release.promise; } };
  const pending = f.read({ liveRead: async () => refs() }); await entered.promise;
  expect(f.rows.size).toBe(1); await f.cache.invalidate(f.native, 'env-a'); release.resolve();
  expect(await pending).toBeNull(); expect(f.rows.size).toBe(0);
});
test('late cache read loses to mutation and independent Client Storage clear', async () => {
  for (const local of [true, false]) {
    const f = fixture(), entered = deferred(), release = deferred(); await f.read({ liveRead: async () => refs() });
    f.controls.after = async row => { if (row.action === 'read') { entered.resolve(); await release.promise; } };
    const pending = f.read(); await entered.promise;
    if (local) await f.cache.invalidate(f.native, 'env-a'); else await mobileCacheClear(f.native);
    release.resolve(); expect(await pending).toBeNull();
  }
});
test('failed invalidation suppresses disk reads until successful recovery before a new live request', async () => {
  const f = fixture(); await f.read({ liveRead: async () => refs() }); f.controls.fail = 'clear';
  expect(await f.cache.invalidate(f.native, 'env-a')).toBe(false);
  const count = f.calls.length; expect(await f.read()).toBeNull(); expect(f.calls.length).toBe(count);
  expect((await f.read({ liveRead: async () => refs() }))?.source).toBe('live');
  expect(f.calls.slice(count).map(row => row.action)).toEqual(['clear']);
  f.controls.fail = undefined; const before = f.calls.length;
  expect((await f.read({ liveRead: async () => refs() }))?.source).toBe('live');
  expect(f.calls.slice(before).map(row => row.action)).toEqual(['clear', 'ticket', 'write']);
  expect((await f.read())?.source).toBe('cache');
});
test('changed caller owner refuses late ticket, live result, write reply and cache read', async () => {
  for (const action of ['ticket', 'write', 'read']) {
    const f = fixture(); await f.read({ liveRead: async () => refs() }); let current = true, requests = 0;
    f.controls.after = async row => { if (row.action === action) current = false; };
    expect(await f.read({ current: () => current, ...(action === 'read' ? {} : { liveRead: async () => { requests++; return refs(); } }) })).toBeNull();
    if (action === 'ticket') expect(requests).toBe(0);
  }
  const f = fixture(); let current = true;
  expect(await f.read({ current: () => current, liveRead: async () => { current = false; return refs(); } })).toBeNull();
  expect(f.calls.map(row => row.action)).toEqual(['ticket']);
});
test('abandoned answer propagates with no later native or live request', async () => {
  const f = fixture(); let requests = 0;
  f.controls.before = async () => { throw { name: 'FetchError', kind: 'Aborted' }; };
  await expect(f.read({ liveRead: async () => { requests++; return refs(); } })).rejects.toMatchObject({ kind: 'superseded' });
  expect(f.calls.length).toBe(1); expect(requests).toBe(0);
});
test('environment invalidation removes every cwd but preserves another environment', async () => {
  const f = fixture();
  await f.read({ liveRead: async () => refs() });
  await f.read({ input: { ...input, cwd: '/another' }, liveRead: async () => refs() });
  await f.read({ environmentId: 'env-b', liveRead: async () => refs() });
  await f.cache.invalidate(f.native, 'env-a');
  expect(f.rows.size).toBe(1); expect([...f.rows.values()][0]!.environmentId).toBe('env-b');
});
test('an active invalidation prevents fresh disk reads or persistence until it finishes', async () => {
  const f = fixture(), entered = deferred(), release = deferred(); await f.read({ liveRead: async () => refs() });
  f.controls.before = async row => { if (row.action === 'clear') { entered.resolve(); await release.promise; } };
  const clearing = f.cache.invalidate(f.native, 'env-a'); await entered.promise; const count = f.calls.length;
  expect(await f.read()).toBeNull();
  expect((await f.read({ liveRead: async () => refs() }))?.source).toBe('live');
  expect(f.calls.length).toBe(count); release.resolve(); await clearing; expect(f.rows.size).toBe(0);
});
test('corrupt domain rows are conditionally removed and never exposed', async () => {
  const f = fixture(); await f.read({ liveRead: async () => refs() });
  const row = [...f.rows.values()][0]!; row.payload = '{';
  expect(await f.read()).toBeNull(); expect(f.rows.size).toBe(0);
  expect(f.calls.at(-1)?.action).toBe('remove'); expect(f.calls.at(-1)?.expectedPayload).toBe('{');
});

// Verified independently against pinned git.ts VcsListRefsResult and baseSchemas.ts
// TrimmedNonEmptyString using the actual Effect decoder, not an assumed validator.
test('pinned string transforms normalize decoded refs while preserving exact persisted cwd keys', async () => {
  const f = fixture(), value = { ...refs(), refs: [{ name: ' main ', current: true, isDefault: true,
    worktreePath: ' /repo ', isRemote: true, remoteName: '\torigin\n' }] };
  const expected = { ...value, refs: [{ ...value.refs[0], name: 'main', worktreePath: '/repo', remoteName: 'origin' }] };
  expect(decodeMobileVcsRefs(value)).toEqual(expected);
  let requests = 0;
  expect(await f.read({ input: { cwd: ' /repo ', limit: 100 }, liveRead: async () => { requests++; return value; } }))
    .toEqual({ source: 'live', refs: expected });
  expect(requests).toBe(1);
  const row = [...f.rows.values()][0]!;
  expect(row.key).toBe(' /repo ');
  expect(JSON.parse(String(row.payload))).toEqual({ schemaVersion: 1, environmentId: 'env-a', cwd: ' /repo ', refs: expected });
  expect(await f.read({ input: { cwd: ' /repo ', limit: 100 } })).toEqual({ source: 'cache', refs: expected });
  expect(await f.read({ input: { cwd: '/repo', limit: 100 } })).toBeNull();
  expect(await f.read({ input: { cwd: ' \t\n ', limit: 100 }, liveRead: async () => { requests++; return value; } })).toBeNull();
  expect(requests).toBe(1);
});

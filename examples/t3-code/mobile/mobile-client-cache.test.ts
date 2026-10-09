import { expect, test } from 'bun:test';
import { mobileCacheClear, mobileCacheClearKind, mobileCacheInspect, mobileCacheList, mobileCacheRead, mobileCacheReadRevision, mobileCacheTicket, mobileCacheWrite } from './mobile-client-cache';
import { ClientError, type Native } from './shared/protocol';
const key = { environmentId: 'env-a', kind: 'shell' as const, key: 'snapshot' };
function native(value: unknown, calls: unknown[] = []): Native {
  return { available: true, watch() {}, async later(input) { calls.push(input); return { ok: true, generation: 0, value }; } };
}
test('cache requests carry only the dedicated local operation and exact scope', async () => {
  const calls: unknown[] = [];
  expect(await mobileCacheTicket(native({ ticket: 'opaque' }, calls), key)).toBe('opaque');
  expect(await mobileCacheWrite(native({ written: true, stale: false }, calls), key, 'opaque', '{"environmentId":"env-a"}')).toBe(true);
  expect(calls).toEqual([{ op: 'mobileClientCache', action: 'ticket', ...key },
    { op: 'mobileClientCache', action: 'write', ...key, ticket: 'opaque', schemaVersion: 1, payload: '{"environmentId":"env-a"}' }]);
  expect(await mobileCacheWrite(native({ written: false, stale: true }), key, 'old', '{}')).toBe(false);
});
test('read checks identity and app schema before returning a cache record', async () => {
  const row = { ...key, schemaVersion: 1, payload: '{}', updatedAt: 100 };
  expect(await mobileCacheRead(native({ record: row }), key)).toEqual(row);
  expect(await mobileCacheRead(native({ record: null }), key)).toBeNull();
  for (const patch of [{ environmentId: 'other' }, { kind: 'thread' }, { key: 'wrong' }, { schemaVersion: 3 }, { payload: null }, { updatedAt: -1 }])
    await expect(mobileCacheRead(native({ record: { ...row, ...patch } }), key)).rejects.toThrow('invalid reply');
  await expect(mobileCacheRead(native({}), key)).rejects.toThrow('invalid reply');
});
test('inspection validates scoped groups, uniqueness and totals; unavailable is never zero', async () => {
  const row = { environmentId: 'env-a', kind: 'shell', recordCount: 1, payloadBytes: 32 };
  const summary = { rows: [row], recordCount: 1, payloadBytes: 32 };
  expect(await mobileCacheInspect(native(summary), 'env-a')).toEqual(summary);
  expect(await mobileCacheInspect(native({ rows: [], recordCount: 0, payloadBytes: 0 }))).toEqual({ rows: [], recordCount: 0, payloadBytes: 0 });
  for (const broken of [{}, { ...summary, recordCount: 2 }, { ...summary, payloadBytes: 31 },
    { ...summary, rows: [row, row], recordCount: 2, payloadBytes: 64 }, { ...summary, rows: [{ ...row, kind: 'credentials' }] },
    { ...summary, rows: [{ ...row, environmentId: 'env-b' }] }])
    await expect(mobileCacheInspect(native(broken), 'env-a')).rejects.toThrow('invalid reply');
});
test('clear captures requested scope and acknowledges actual removed count', async () => {
  const calls: unknown[] = [];
  expect(await mobileCacheClear(native({ removed: 3 }, calls), { environmentId: 'env-a' })).toBe(3);
  expect(calls).toEqual([{ op: 'mobileClientCache', action: 'clear', environmentId: 'env-a' }]);
  await expect(mobileCacheClear(native({ removed: -1 }))).rejects.toThrow('invalid reply');
});
test('kind clear invalidates combined readers in every environment but preserves unrelated kind readers', async () => {
  const calls: unknown[] = [];
  const before = ['env-a', 'env-b'].map(environmentId => ({ environmentId,
    combined: mobileCacheReadRevision(environmentId), shell: mobileCacheReadRevision(environmentId, 'shell'),
    favicon: mobileCacheReadRevision(environmentId, 'project-favicon') }));
  expect(await mobileCacheClearKind(native({ removed: 2 }, calls), 'shell')).toBe(2);
  expect(calls).toEqual([{ op: 'mobileClientCache', action: 'clearKind', kind: 'shell' }]);
  for (const row of before) {
    expect(mobileCacheReadRevision(row.environmentId)).not.toBe(row.combined);
    expect(mobileCacheReadRevision(row.environmentId, 'shell')).not.toBe(row.shell);
    expect(mobileCacheReadRevision(row.environmentId, 'project-favicon')).toBe(row.favicon);
  }
});
test.each(['failure', 'let-go'])('kind clear invalidates readers on admission despite %s', async failure => {
  const before = mobileCacheReadRevision('env-clear-failure');
  const kindBefore = mobileCacheReadRevision('env-clear-failure', 'thread');
  const handle: Native = { available: true, watch() {}, async later() {
    expect(mobileCacheReadRevision('env-clear-failure')).not.toBe(before);
    expect(mobileCacheReadRevision('env-clear-failure', 'thread')).not.toBe(kindBefore);
    if (failure === 'let-go') throw { name: 'FetchError', kind: 'Aborted' };
    return { ok: false, generation: 0, error: { kind: 'Disk', message: 'Clear failed' } };
  } };
  if (failure === 'let-go') await expect(mobileCacheClearKind(handle, 'thread')).rejects.toMatchObject({ kind: 'superseded' });
  else await expect(mobileCacheClearKind(handle, 'thread')).rejects.toThrow('Clear failed');
  expect(mobileCacheReadRevision('env-clear-failure')).not.toBe(before);
});
test('failure and abandoned answer propagate without another native call', async () => {
  const calls: unknown[] = [];
  const handle: Native = { available: true, watch() {}, async later(input) { calls.push(input); throw { name: 'FetchError', kind: 'Aborted' }; } };
  await expect(mobileCacheRead(handle, key)).rejects.toMatchObject({ kind: 'superseded' });
  expect(calls).toHaveLength(1);
  const failed: Native = { available: true, watch() {}, async later() { return { ok: false, generation: 0, error: { kind: 'Disk', message: 'Cache disk unavailable' } }; } };
  await expect(mobileCacheInspect(failed)).rejects.toThrow('Cache disk unavailable');
  expect(new ClientError('x', 'Cache').kind).toBe('Cache');
});

test('global pages use SQLite Unicode order and reject stale, duplicate or mismatched rows', async () => {
  const make = (environmentId: string, key: string, updatedAt = 100) =>
    ({ environmentId, kind: 'project-favicon' as const, key, updatedAt, schemaVersion: 1 });
  const rows = [make('one', '\uE000'), make('one', '😀'), make('\uE000', 'a'), make('😀', 'a')];
  const calls: unknown[] = [];
  expect(await mobileCacheList(native({ rows }, calls), 'project-favicon', null, 4)).toEqual(rows);
  expect(calls).toEqual([{ op: 'mobileClientCache', action: 'list', kind: 'project-favicon', after: null, limit: 4 }]);
  expect(await mobileCacheList(native({ rows: rows.slice(2) }), 'project-favicon', rows[1], 2)).toEqual(rows.slice(2));
  expect(await mobileCacheList(native({ rows: [] }), 'project-favicon', rows[3], 2)).toEqual([]);
  for (const broken of [[rows[1], rows[0]], [rows[0], rows[0]], [{ ...rows[0], kind: 'thread' }],
    [{ ...rows[0], updatedAt: -1 }], [{ ...rows[0], environmentId: '' }], [{ ...rows[0], schemaVersion: 1.5 }]]) {
    await expect(mobileCacheList(native({ rows: broken }), 'project-favicon')).rejects.toThrow('invalid reply');
  }
  await expect(mobileCacheList(native({ rows }), 'project-favicon', null, 2)).rejects.toThrow('invalid reply');
  await expect(mobileCacheList(native({ rows: [rows[1]] }), 'project-favicon', rows[1])).rejects.toThrow('invalid reply');
  for (const limit of [0, 129, -1, 1.5])
    await expect(mobileCacheList(native({ rows: [] }), 'project-favicon', null, limit)).rejects.toThrow('invalid reply');
});

test('corrupt domain cleanup compares original bytes and ignores ordinary removal failure', async () => {
  const { mobileCacheReadDecoded } = await import('./mobile-client-cache');
  const calls: unknown[] = [], payload = '{"environmentId":"env-a","broken":true}';
  const handle: Native = { available: true, watch() {}, async later(input) {
    calls.push(input);
    if ((input as any).action === 'read') return { ok: true, generation: 0, value: { record: { ...key, schemaVersion: 1, payload, updatedAt: 0 } } };
    return { ok: false, generation: 0, error: { kind: 'Disk', message: 'Cannot remove cache' } };
  } };
  expect(await mobileCacheReadDecoded(handle, key, () => null, () => true)).toBeNull();
  expect(calls).toEqual([{ op: 'mobileClientCache', action: 'read', ...key },
    { op: 'mobileClientCache', action: 'remove', ...key, expectedPayload: payload }]);
  calls.length = 0;
  expect(await mobileCacheReadDecoded(handle, key, () => null, () => false)).toBeNull();
  expect(calls).toHaveLength(1);
});
test('lost corrupt-cleanup answer propagates and conditional removal cannot clear a whole scope', async () => {
  const { mobileCacheReadDecoded, mobileCacheRemove, mobileCacheReadRevision } = await import('./mobile-client-cache');
  const record = { ...key, schemaVersion: 1, payload: '{}', updatedAt: 0 };
  const handle: Native = { available: true, watch() {}, async later(input) {
    if ((input as any).action === 'read') return { ok: true, generation: 0, value: { record } };
    throw { name: 'FetchError', kind: 'Aborted' };
  } };
  await expect(mobileCacheReadDecoded(handle, key, () => null, () => true)).rejects.toMatchObject({ kind: 'superseded' });
  const initial = mobileCacheReadRevision(key.environmentId);
  expect(await mobileCacheRemove(native({ removed: 0 }), key, 'stale payload')).toBe(0);
  expect(mobileCacheReadRevision(key.environmentId)).toBe(initial);
  expect(await mobileCacheRemove(native({ removed: 1 }), key)).toBe(1);
  expect(mobileCacheReadRevision(key.environmentId)).not.toBe(initial);
  await expect(mobileCacheRemove(native({ removed: 2 }), key)).rejects.toThrow('invalid reply');
});

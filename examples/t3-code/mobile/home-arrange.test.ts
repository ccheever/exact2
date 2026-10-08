import { expect, test } from 'bun:test';
import { T3Client } from './shared/client';
import { EnvironmentFleet, type FleetEntry } from './shared/settings-b-fleet';
import { initialShell, obj, str, type Obj } from './shared/domain';
import type { Native } from './shared/protocol';
import { mobileHomeAction, mobileHomeActionsObserve, mobileHomeMenu } from './home-actions';
import { mobileHomeSources, mobileHome, projectMobileHome } from './home';
import { homeApplyPending, homeCreatePending, homeMoveAvailability, homeMovePlan, homeOrderAfterMove, homeOrderKey,
  homeOrderedSection, homeOrderState, homeReconcilePending, mobileHomeOrder, type HomeOrderSource } from './home-order';

const now = Date.parse('2026-10-07T12:00:00Z');
const caps = { serverResolvedCommandContext: true, threadSettlement: true, threadSnooze: true, threadPinning: true,
  threadPinReorder: true, threadActiveReorder: true };
const thread = (id: string, patch: Obj = {}): Obj => ({ id, projectId: 'p', title: id, createdAt: '2026-10-01T00:00:00Z', updatedAt: '2026-10-01T00:00:00Z',
  archivedAt: null, deletedAt: null, lineage: { relationshipToParent: null }, pinnedAt: null, pinOrderKey: null, activeOrderKey: null,
  settledOverride: null, snoozedUntil: null, status: 'idle', latestRunId: null, activeRunId: null, activeProviderThreadId: null, ...patch });
const source = (environmentId: string, threads: Obj[], config = caps): HomeOrderSource => ({ environmentId, config: { environment: { capabilities: config } }, shell: { threads } });
const deferred = () => { let resolve!: (value: unknown) => void; const promise = new Promise(done => { resolve = done; }); return { promise, resolve }; };
function fixture(hook?: (request: Obj) => unknown | Promise<unknown>, keyless = false) {
  const client = new T3Client(), background = new EnvironmentFleet(), calls: Obj[] = []; let serial = 0;
  Object.assign(client, { environmentId: 'one', origin: 'https://one.test', generation: 3, connection: 'connected', configLive: true,
    shellLive: true, threadLive: true, shellLoaded: true, scopes: ['orchestration:operate'], projectId: 'p', threadId: 'a' });
  client.config = { environment: { capabilities: { ...caps } } };
  client.shell = { ...initialShell(), projects: [{ id: 'p', title: 'Project' }], threads: [thread('a', { activeOrderKey: keyless ? null : 'b' }), thread('c', { activeOrderKey: keyless ? null : 'z' })] };
  client.local.drafts['one:a'] = 'Retain draft';
  const remote: FleetEntry = { key: 'two-route', origin: 'https://two.test', environmentId: 'two', phase: 'connected', message: '', traceId: '', generation: 8,
    synchronized: 8, lastEvent: 0, subscriptions: {}, config: { environment: { capabilities: { ...caps } } },
    shell: { ...initialShell(), projects: [{ id: 'p', title: 'Remote project' }], threads: [thread('b', { activeOrderKey: keyless ? null : 'm' })] }, scopes: ['orchestration:operate'], error: '', requested: true };
  background.entries.set(remote.key, remote);
  const native: Native = { available: true, watch() {}, async later(input) {
    const request = obj(input); calls.push(request);
    const answer = await hook?.(request); if (answer !== undefined) return answer;
    return { ok: true, generation: request.fleet ? 8 : 3, value: request.op === 'http' ? { authenticated: true, permissions: ['orchestration:operate'] }
      : request.op === 'ids' ? [`${request.fleet || 'one'}-command-${++serial}`] : {} };
  } };
  mobileHomeActionsObserve('visit', true, false, client);
  const snapshot = () => mobileHomeOrder(client, mobileHomeSources(client, background), now);
  const run = (direction = 'move-down', id = 'a', env = 'one') => mobileHomeAction('visit', env, id, direction, '', now, native, client, background);
  const writes = () => calls.filter(request => request.method === 'orchestration.dispatchCommand');
  const publish = (request: Obj) => {
    const payload = obj(request.payload), shell = request.fleet ? remote.shell : client.shell;
    const row = shell.threads.find(row => row.id === payload.threadId)!;
    row[payload.type === 'thread.pin.reorder' ? 'pinOrderKey' : 'activeOrderKey'] = payload.orderKey;
  };
  return { client, background, remote, calls, native, snapshot, run, writes, publish };
}

import { homeArrangeSnapshot, homeArrangeActionValue, homeDropLifecycle, parseHomeArrangeAction } from './home-arrange';
import { homeWriteReflected } from './home-write-reflected';

const arrange = (f: ReturnType<typeof fixture>) => homeArrangeSnapshot(f.client, mobileHomeSources(f.client, f.background), now);
function drop(f: ReturnType<typeof fixture>, section: string, target = '', placement = 'before', env = 'one', id = 'a', version = arrange(f).version) {
  return mobileHomeAction('visit', env, id, 'arrange', homeArrangeActionValue(version, section, target, placement), now, f.native, f.client, f.background);
}
function publishStep(f: ReturnType<typeof fixture>, request: Obj) {
  const payload = obj(request.payload), shell = request.fleet ? f.remote.shell : f.client.shell;
  const row = shell.threads.find(row => row.id === payload.threadId)!;
  if (payload.type === 'thread.pin') {
    if (row.pinnedAt == null) { row.pinnedAt = '2026-10-07T12:00:00Z'; if (payload.orderKey !== undefined) row.pinOrderKey = payload.orderKey; }
    if (row.settledOverride === 'settled') row.settledOverride = 'active'; row.snoozedUntil = null;
  } else if (payload.type === 'thread.unpin') { row.pinnedAt = null; row.pinOrderKey = null; }
  else if (payload.type === 'thread.unsettle') { row.settledOverride = 'active'; row.unsettledAt = '2026-10-07T12:00:00Z'; }
  else if (payload.type === 'thread.unsnooze') row.snoozedUntil = null;
  else if (payload.type === 'thread.settle') { row.settledOverride = 'settled'; row.pinnedAt = null; row.pinOrderKey = null; row.activeOrderKey = null; }
  else f.publish(request);
}

test('Arrange decoding rejects malformed destinations without allocating identifiers or requesting auth', async () => {
  const f = fixture();
  for (const value of ['bad', '[]', '{}', JSON.stringify({ version: 1, destination: {} }), ...[
    { section: 'snoozed', targetId: null, placement: 'before' }, { section: 'active', targetId: '', placement: 'before' },
    { section: 'active', targetId: 42, placement: 'before' }, { section: 'active', targetId: null, placement: 'middle' },
  ].map(destination => JSON.stringify({ version: 'v', destination }))]) {
    expect(parseHomeArrangeAction(value)).toBeNull();
    expect((await mobileHomeAction('visit', 'one', 'a', 'arrange', value, now, f.native, f.client, f.background)).message).not.toBe('');
  }
  expect(f.calls).toHaveLength(0);
  expect(parseHomeArrangeAction(homeArrangeActionValue('v', 'active', '', 'after'))).toEqual({ version: 'v', destination: { section: 'active', targetId: null, placement: 'after' } });
});

test('Arrange projection contains full parked sections, source accessibility destinations and a stable order version', () => {
  const f = fixture(); f.client.shell.threads.push(thread('settled', { settledOverride: 'settled' }), thread('sleep', { snoozedUntil: '2026-10-08T12:00:00Z' }),
    thread('subagent', { lineage: { relationshipToParent: 'subagent' } }), thread('archived', { archivedAt: 'date' }));
  const first = arrange(f);
  expect(first.active.map(row => row.key)).toEqual(['one:a', 'two:b', 'one:c']); expect(first.settled.map(row => row.key)).toEqual(['one:settled']);
  expect(first.snoozed.map(row => row.key)).toEqual(['one:sleep']); expect(first.settled[0]).toMatchObject({ canPin: true, canActivate: true, canSettle: false });
  f.client.shell.threads[0]!.title = 'Changed title'; expect(arrange(f).version).toBe(first.version);
  f.client.shell.threads[0]!.activeOrderKey = 'c'; expect(arrange(f).version).not.toBe(first.version);
  mobileHomeOrder(f.client, mobileHomeSources(f.client, f.background), now, { workingEnabled: true });
  expect(arrange(f).settled[0]?.canActivate).toBe(false); expect(arrange(f).settled[0]?.canPin).toBe(true);
});

test('same-section arbitrary drop uses canonical pending owner and ignores display filtering', async () => {
  const f = fixture(); expect(await drop(f, 'active', 'one:c', 'after')).toMatchObject({ message: '' });
  expect(homeOrderState(f.client).pending?.orderedIds).toEqual(['two:b', 'one:c', 'one:a']); expect(f.writes()).toHaveLength(1);
  expect(f.writes()[0]?.payload).toMatchObject({ type: 'thread.active.reorder', threadId: 'a' });
  await drop(f, 'active', 'two:b'); expect(f.writes()).toHaveLength(1);
  f.publish(f.writes()[0]!); expect(arrange(f).blocked).toBe(false);
});

test('cross-section fresh Pin consumes its own assignment and preflights every rewrite environment', async () => {
  const f = fixture(request => { if (request.method === 'orchestration.dispatchCommand') publishStep(f, request); });
  Object.assign(f.remote.shell.threads[0]!, { pinnedAt: 'date', pinOrderKey: 'm' });
  expect(await drop(f, 'pinned', 'two:b')).toMatchObject({ message: '' });
  expect(f.writes().map(row => obj(row.payload).type)).toEqual(['thread.pin']);
  expect(f.writes()[0]?.payload).toMatchObject({ threadId: 'a', orderKey: 'g' }); expect(homeOrderState(f.client).pending).toBeNull();
  const g = fixture(request => request.op === 'http' && request.fleet ? { ok: true, generation: 8, value: { authenticated: true, permissions: [] } } : undefined);
  Object.assign(g.remote.shell.threads[0]!, { pinnedAt: 'date', pinOrderKey: null });
  expect((await drop(g, 'pinned', 'two:b')).message).toBe('This connection cannot change threads.'); expect(g.writes()).toHaveLength(0);
});

test('hidden already-pinned row is promoted then gets its own new order key', async () => {
  const f = fixture(request => { if (request.method === 'orchestration.dispatchCommand') publishStep(f, request); });
  Object.assign(f.client.shell.threads[0]!, { pinnedAt: 'old', pinOrderKey: 'z', snoozedUntil: '2026-10-08T12:00:00Z' });
  Object.assign(f.remote.shell.threads[0]!, { pinnedAt: 'date', pinOrderKey: 'm' });
  expect(await drop(f, 'pinned', 'two:b')).toMatchObject({ message: '' });
  expect(f.writes().map(row => obj(row.payload).type)).toEqual(['thread.pin', 'thread.pin.reorder']);
  expect(f.client.shell.threads[0]).toMatchObject({ pinnedAt: 'old', pinOrderKey: 'g', snoozedUntil: null });
});

test('Active drop clears all captured parked states in source order despite own shell updates', async () => {
  const f = fixture(request => { if (request.method === 'orchestration.dispatchCommand') publishStep(f, request); });
  Object.assign(f.client.shell.threads[0]!, { pinnedAt: 'old', pinOrderKey: 'z', settledOverride: 'settled', snoozedUntil: '2026-10-08T12:00:00Z' });
  expect(await drop(f, 'active', 'two:b')).toMatchObject({ message: '' });
  expect(f.writes().map(row => obj(row.payload).type)).toEqual(['thread.unpin', 'thread.unsettle', 'thread.unsnooze', 'thread.active.reorder']);
  expect(f.writes().slice(1, 3).map(row => obj(row.payload).reason)).toEqual(['user', 'user']); expect(homeOrderState(f.client).pending).toBeNull();
});

test('Settled drop sends only Settle and does not require a reorder capability', async () => {
  const f = fixture(); Object.assign(obj(obj(f.client.config.environment).capabilities), { threadActiveReorder: false, threadPinReorder: false });
  expect(await drop(f, 'settled')).toMatchObject({ message: '' }); expect(f.writes().map(row => obj(row.payload).type)).toEqual(['thread.settle']);
  f.client.shell.threads[0]!.settledOverride = 'settled'; await drop(f, 'settled'); expect(f.writes()).toHaveLength(1);
});

test('partial lifecycle refusal preserves earlier canonical results and sends no inverse or later write', async () => {
  let count = 0;
  const f = fixture(request => {
    if (request.method !== 'orchestration.dispatchCommand') return;
    if (++count === 2) return { ok: false, generation: 3, error: { kind: 'server', message: 'Refused' } };
    publishStep(f, request);
  });
  Object.assign(f.client.shell.threads[0]!, { pinnedAt: 'old', settledOverride: 'settled', snoozedUntil: '2026-10-08T12:00:00Z' });
  expect(await drop(f, 'active', 'two:b')).toMatchObject({ message: 'Refused', uncertain: false });
  expect(f.writes().map(row => obj(row.payload).type)).toEqual(['thread.unpin', 'thread.unsettle']);
  expect(f.client.shell.threads[0]!.pinnedAt).toBeNull(); expect(homeOrderState(f.client).busy).toBe(false); expect(homeOrderState(f.client).pending).toBeNull();
});

test('unknown lifecycle blocks another drop and same-row ordinary action until canonical evidence', async () => {
  const f = fixture(request => request.method === 'orchestration.dispatchCommand' ? { ok: false, generation: 3, error: { kind: 'transport', message: 'Unknown', uncertain: true } } : undefined);
  Object.assign(f.client.shell.threads[0]!, { pinnedAt: 'old', pinOrderKey: 'z' });
  expect(await drop(f, 'active', 'two:b')).toMatchObject({ uncertain: true });
  expect(homeOrderState(f.client).unknown).toMatchObject({ kind: 'lifecycle', payload: { type: 'thread.unpin' } });
  await drop(f, 'active', 'two:b'); expect(await mobileHomeAction('visit', 'one', 'a', 'rename', '', now, f.native, f.client, f.background)).toMatchObject({ uncertain: true });
  expect(f.writes()).toHaveLength(1);
  f.client.origin = 'https://other.test'; publishStep(f, f.writes()[0]!); expect(arrange(f).blocked).toBe(true);
  f.client.origin = 'https://one.test'; expect(arrange(f).blocked).toBe(false); expect(f.writes()).toHaveLength(1);
});

test('unknown promotion Pin reconciles preserved old key and wake, not an unsent destination key', async () => {
  const f = fixture(request => request.method === 'orchestration.dispatchCommand' ? Promise.reject(new Error('lost')) : undefined);
  Object.assign(f.client.shell.threads[0]!, { pinnedAt: 'old', pinOrderKey: 'z', settledOverride: 'settled', snoozedUntil: '2026-10-08T12:00:00Z' });
  Object.assign(f.remote.shell.threads[0]!, { pinnedAt: 'date', pinOrderKey: 'm' });
  expect(await drop(f, 'pinned', 'two:b')).toMatchObject({ uncertain: true }); expect(arrange(f).blocked).toBe(true);
  publishStep(f, f.writes()[0]!); expect(f.client.shell.threads[0]!.pinOrderKey).toBe('z');
  expect(arrange(f).blocked).toBe(false); expect(f.writes()).toHaveLength(1);
});

test('stale versions, Working Active targets and changed lifecycle capabilities dispatch nothing', async () => {
  const f = fixture(), version = arrange(f).version; f.client.shell.threads[1]!.activeOrderKey = 'y';
  expect(await drop(f, 'active', 'two:b', 'before', 'one', 'a', version)).toMatchObject({ message: 'The thread order changed. Try the move again.' }); expect(f.calls).toHaveLength(0);
  mobileHomeOrder(f.client, mobileHomeSources(f.client, f.background), now, { workingEnabled: true }); await drop(f, 'active', 'two:b'); expect(f.calls).toHaveLength(0);
  const g = fixture(request => { if (request.op === 'ids') obj(obj(g.client.config.environment).capabilities).threadSnooze = false; });
  Object.assign(g.client.shell.threads[0]!, { snoozedUntil: '2026-10-08T12:00:00Z' });
  expect((await drop(g, 'pinned')).message).toBe('The thread order changed. Try the move again.'); expect(g.writes()).toHaveLength(0);
});

test('Arrange duplicate drops and route departure during grant cannot dispatch stale writes', async () => {
  const gate = deferred(), started = deferred();
  const f = fixture(request => { if (request.op === 'http') { started.resolve(null); return gate.promise; } });
  const pending = drop(f, 'pinned'); await started.promise; await drop(f, 'pinned'); expect(f.calls.filter(row => row.op === 'http')).toHaveLength(1);
  mobileHomeActionsObserve('left', false, false, f.client); gate.resolve({ ok: true, generation: 3, value: { authenticated: true, permissions: ['orchestration:operate'] } });
  await expect(pending).rejects.toMatchObject({ kind: 'superseded' }); expect(f.writes()).toHaveLength(0);
});

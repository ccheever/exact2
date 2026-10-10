import { expect, test } from 'bun:test';
import { homeObserveReturns, type HomeReturnState } from './home-returns';
import { T3Client } from './shared/client';
import { EnvironmentFleet } from './shared/settings-b-fleet';
import { initialShell, type Obj } from './shared/domain';
import { mobileHomeOrder, homeOrderState } from './home-order';
import { mobileHomeSources } from './home';
import { mobileHomeView } from './home-state';
import { homeArrangeSnapshot } from './home-arrange';

const now = Date.parse('2026-10-07T12:00:00Z');
const thread = (id: string, patch: Obj = {}): Obj => ({ id, environmentId: 'one', projectId: 'p', title: id,
  createdAt: '2026-09-01T00:00:00Z', updatedAt: '2026-09-01T00:00:00Z', archivedAt: null, deletedAt: null, lineage: null,
  pinnedAt: null, pinOrderKey: null, activeOrderKey: null, settledOverride: null, snoozedUntil: null,
  latestRunId: 'run', activeRunId: 'run', status: 'running', latestRunRequestedAt: '2026-09-01T00:00:00Z', latestRunCompletedAt: null,
  pendingRuntimeRequest: null, ...patch });
const state = (): HomeReturnState => ({ lastWorkingKeys: null, returnedAt: {} });
function fixture() {
  const client = new T3Client(), background = new EnvironmentFleet();
  Object.assign(client, { environmentId: 'one', origin: 'https://one.test', generation: 3, connection: 'connected', configLive: true,
    shellLive: true, threadLive: true, shellLoaded: true, scopes: ['orchestration:operate'] });
  client.config = { environment: { capabilities: { threadPinning: true, threadSettlement: true, threadSnooze: true, threadPinReorder: true, threadActiveReorder: true } } };
  client.shell = { ...initialShell(), projects: [{ id: 'p', title: 'Project', workspaceRoot: '/project' }], threads: [thread('returned'),
    thread('newer', { latestRunId: null, activeRunId: null, status: 'idle', createdAt: '2026-10-01T00:00:00Z' })] };
  const view = (at: number, query = '', env = '', project = '', enabled = true) => mobileHomeView([0, at, query, 10, true, enabled, true, true, true,
    env, project, '', 'repository', 'home-visit', true, true], client, background);
  return { client, background, view };
}

test('return owner initializes without stamps, records repeated returns once and resets/prunes by full scoped identity', () => {
  const owner = state(), a = thread('same'), b = thread('same', { environmentId: 'two' });
  homeObserveReturns(owner, [a, b], now); expect(owner.returnedAt).toEqual({});
  a.pendingRuntimeRequest = { kind: 'approval' }; homeObserveReturns(owner, [a, b], now + 100);
  expect(owner.returnedAt).toEqual({ 'one:same': now + 100 });
  homeObserveReturns(owner, [a, b], now + 200); expect(owner.returnedAt['one:same']).toBe(now + 100);
  b.pendingRuntimeRequest = { kind: 'user_input' }; homeObserveReturns(owner, [a, b], now + 300);
  expect(owner.returnedAt['two:same']).toBe(now + 300);
  homeObserveReturns(owner, [b], now + 400); expect(owner.returnedAt).toEqual({ 'two:same': now + 300 });
  b.pendingRuntimeRequest = null; homeObserveReturns(owner, [b], now + 500);
  b.pendingRuntimeRequest = { kind: 'approval' }; homeObserveReturns(owner, [b], now + 600);
  expect(owner.returnedAt['two:same']).toBe(now + 600);
  homeObserveReturns(owner, null, now + 700); expect(owner).toEqual(state());
  homeObserveReturns(owner, [b], now + 800); expect(owner.returnedAt).toEqual({});
});

test('phone Home, sidebar and Arrange share raw V2 observed returns across filtered rebuilds', () => {
  const f = fixture(); f.view(now, 'newer', 'missing', 'missing');
  f.client.shell.threads[0]!.pendingRuntimeRequest = { kind: 'approval' };
  f.view(now + 12_345, 'newer', 'missing', 'missing');
  const phone = f.view(now + 12_346), sidebar = f.view(now + 12_347, 'returned');
  expect(phone.items.filter(row => row.kind === 'thread').map(row => row.threadId)).toEqual(['returned', 'newer']);
  expect(sidebar.items.filter(row => row.kind === 'thread').map(row => row.threadId)).toEqual(['returned']);
  expect(phone.arrangement.active.map(row => row.threadId)).toEqual(['returned', 'newer']);
  expect(sidebar.arrangement.active).toEqual(phone.arrangement.active);
  expect(homeOrderState(f.client).inboxReturns.returnedAt['one:returned']).toBe(now + 12_345);
  expect(phone.arrangement.active.every(row => !row.canMoveUp && !row.canMoveDown)).toBe(true);
  expect(Object.keys(phone.arrangement)).not.toContain('returnedAt');
});

test('separate clients, toggled beta and deleted navigation rows do not retain another observation history', () => {
  const f = fixture(), g = fixture(); f.view(now); g.view(now);
  f.client.shell.threads[0]!.pendingRuntimeRequest = { kind: 'approval' }; f.view(now + 1);
  expect(homeOrderState(g.client).inboxReturns.returnedAt).toEqual({});
  f.view(now + 2, '', '', '', false); expect(homeOrderState(f.client).inboxReturns).toEqual(state());
  f.view(now + 3); expect(homeOrderState(f.client).inboxReturns.returnedAt).toEqual({});
  f.client.shell.threads[0]!.pendingRuntimeRequest = null; f.view(now + 4);
  f.client.shell.threads[0]!.pendingRuntimeRequest = { kind: 'user_input' }; f.view(now + 5);
  f.client.shell.threads[0]!.archivedAt = 'date'; f.view(now + 6);
  expect(homeOrderState(f.client).inboxReturns.returnedAt).toEqual({});
});

test('two returns within one minute reorder all projections and invalidate an Arrange version', () => {
  const f = fixture(); f.client.shell.threads = [thread('a'), thread('b')]; f.view(now);
  f.client.shell.threads[0]!.pendingRuntimeRequest = { kind: 'approval' };
  const first = f.view(now + 12_345);
  f.client.shell.threads[1]!.pendingRuntimeRequest = { kind: 'user_input' };
  const second = f.view(now + 23_456);
  expect(second.items.filter(row => row.kind === 'thread').map(row => row.threadId)).toEqual(['b', 'a']);
  expect(second.arrangement.active.map(row => row.threadId)).toEqual(['b', 'a']);
  expect(second.arrangement.version).not.toBe(first.arrangement.version);
  const order = mobileHomeOrder(f.client, mobileHomeSources(f.client, f.background), now + 23_457);
  f.client.shell.threads = []; f.view(now + 23_458);
  expect(order.returnedAt).toEqual({ 'one:a': now + 12_345, 'one:b': now + 23_456 });
  expect(homeArrangeSnapshot(f.client, mobileHomeSources(f.client, f.background), now + 23_459).active).toEqual([]);
});

test('async move checks cannot consume an unrelated return with an older action-start clock', async () => {
  const { mobileHomeAction } = await import('./home-actions');
  const f = fixture(), returned = thread('returned');
  f.client.shell.threads = [thread('pin-a', { pinnedAt: 'date', pinOrderKey: 'b', latestRunId: null }),
    thread('pin-b', { pinnedAt: 'date', pinOrderKey: 'm', latestRunId: null }), returned];
  f.view(now);
  const native = { available: true, watch() {}, async later(request: unknown) {
    const value = request as Obj;
    if (value.op === 'ids') returned.pendingRuntimeRequest = { kind: 'approval' };
    return { ok: true, generation: 3, value: value.path === '/api/auth/session'
      ? { authenticated: true, permissions: ['orchestration:operate'] } : value.op === 'ids' ? ['move-id'] : {} };
  } };
  const result = await mobileHomeAction('home-visit', 'one', 'pin-a', 'move-down', '', now + 1, native, f.client, f.background);
  expect(result.message).toBe(''); expect(returned.pendingRuntimeRequest).toEqual({ kind: 'approval' });
  expect(homeOrderState(f.client).inboxReturns.returnedAt['one:returned']).toBeUndefined();
  f.view(now + 45_000);
  expect(homeOrderState(f.client).inboxReturns.returnedAt['one:returned']).toBe(now + 45_000);
});

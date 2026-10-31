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

test('full sections preserve scoped identity, queue reentry and server capability membership before filtering', () => {
  const input = [source('one', [thread('same', { activeOrderKey: 'm' }), thread('settled', { settledOverride: 'settled' }),
    thread('queued', { settledOverride: 'settled' }), thread('snoozed', { snoozedUntil: new Date(now + 60000).toISOString() }),
    thread('pinned', { pinnedAt: 'date', pinOrderKey: 'b' }), thread('subagent', { lineage: { relationshipToParent: 'subagent' } }), thread('archived', { archivedAt: 'date' })]),
  source('two', [thread('same', { activeOrderKey: 'b' }), thread('legacy', { settledOverride: 'settled', snoozedUntil: new Date(now + 60000).toISOString() })], { ...caps, threadSettlement: false, threadSnooze: false })];
  expect(homeOrderedSection(input, 'active', now, new Set(['one:queued'])).map(homeOrderKey)).toEqual(['two:legacy', 'one:queued', 'two:same', 'one:same']);
  expect(homeOrderedSection(input, 'pinned', now).map(homeOrderKey)).toEqual(['one:pinned']);
});

test('adjacent planning honors boundaries, hidden reservations and capability-only rewrite admission', () => {
  const rows = [thread('a', { environmentId: 'one', activeOrderKey: 'b' }), thread('b', { environmentId: 'two', activeOrderKey: 'm' }), thread('c', { environmentId: 'one', activeOrderKey: 'z' })];
  const input = { ordered: rows, allThreads: [...rows, thread('hidden', { environmentId: 'one', activeOrderKey: 't' })], section: 'active' as const, reorderableEnvironmentIds: new Set(['one']) };
  expect(homeOrderAfterMove(rows.map(homeOrderKey), 'one:a', 'up')).toBeNull();
  expect(homeMovePlan(input, 'one:a', 'down')?.[0]?.orderKey).not.toBe('t');
  rows[1]!.activeOrderKey = null;
  expect(homeMovePlan(input, 'one:a', 'down')).toBeNull();
  input.reorderableEnvironmentIds.add('two');
  expect(homeMovePlan(input, 'one:a', 'down')?.length).toBeGreaterThan(1);
});

test('batched availability matches the actual planner for all rows without per-row planning in production', () => {
  const rows = Array.from({ length: 24 }, (_, i) => thread(String(i), { environmentId: i % 3 ? 'one' : 'two', activeOrderKey: i % 4 ? String.fromCharCode(98 + i % 20) : null }));
  for (const supported of [new Set(['one']), new Set(['one', 'two'])]) {
    const input = { ordered: rows, allThreads: [...rows, thread('hidden', { environmentId: 'two', activeOrderKey: 't' })], section: 'active' as const, reorderableEnvironmentIds: supported };
    const availability = homeMoveAvailability(input);
    for (const row of rows) expect(availability.get(homeOrderKey(row))).toEqual({ canMoveUp: homeMovePlan(input, homeOrderKey(row), 'up') !== null,
      canMoveDown: homeMovePlan(input, homeOrderKey(row), 'down') !== null });
  }
});

test('pending order holds ACK-before-shell, applies after filtering and releases only after canonical confirmation', async () => {
  const f = fixture(); await f.run();
  expect(homeOrderState(f.client).pending?.commandsComplete).toBe(true);
  const first = f.snapshot(); expect(first.blocked).toBe(true);
  expect(homeApplyPending(first.sections.active.filter(row => row.environmentId === 'one'), 'active', first.pending).map(homeOrderKey)).toEqual(['one:a', 'one:c']);
  expect(mobileHome(now, { query: 'b', orderSnapshot: first }, f.client, f.background).items.filter(row => row.kind === 'thread').map(row => row.key)).toEqual(['two:b']);
  f.publish(f.writes()[0]!); expect(f.snapshot().pending).toBeNull(); expect(f.snapshot().blocked).toBe(false);
});

test('pending reconciliation cancels external changes, membership/anchor changes and reversed confirmations', () => {
  const rows = [thread('a', { environmentId: 'one', activeOrderKey: 'b' }), thread('b', { environmentId: 'two', activeOrderKey: 'm' })];
  const pending = homeCreatePending('active', rows, 'one:a', 'down', [{ id: 'one:a', orderKey: 't' }], 1);
  expect(homeReconcilePending(pending, rows.slice(1))).toBeNull();
  expect(homeReconcilePending(pending, [{ ...rows[0], unsettledAt: 'new' }, rows[1]!])).toBeNull();
  expect(homeReconcilePending(pending, [rows[0]!, { ...rows[1], activeOrderKey: 'z' }])).toBeNull();
  const confirmed = homeReconcilePending(pending, [{ ...rows[0], activeOrderKey: 't' }, rows[1]!])!;
  expect(confirmed.confirmed).toEqual(['one:a']); expect(homeReconcilePending(confirmed, rows)).toBeNull();
  expect(homeReconcilePending({ ...confirmed, commandsComplete: true }, [{ ...rows[0], activeOrderKey: 't' }, rows[1]!])).toBeNull();
});

test('source menu gates Move by section, Working beta and pending state', async () => {
  const f = fixture(); const menu = () => mobileHomeMenu('one', 'a', now, f.client, f.background);
  expect(menu().filter(row => row.id.startsWith('move')).map(row => [row.id, row.disabled])).toEqual([['move-up', true], ['move-down', false]]);
  mobileHomeOrder(f.client, mobileHomeSources(f.client, f.background), now, { workingEnabled: true });
  expect(menu().some(row => row.id.startsWith('move'))).toBe(false); await f.run(); expect(f.writes()).toHaveLength(0);
  f.client.shell.threads[0]!.pinnedAt = 'date'; expect(menu().filter(row => row.id.startsWith('move'))).toHaveLength(2);
  f.client.shell.threads[0]!.settledOverride = 'settled'; expect(menu().some(row => row.id.startsWith('move'))).toBe(false);
  f.client.shell.threads[0]!.snoozedUntil = new Date(now + 60000).toISOString(); expect(menu().some(row => row.id.startsWith('move'))).toBe(false);
});

test('Move uses full unfiltered section, correct source payload and keeps focused drafts unchanged', async () => {
  const f = fixture(); expect(await f.run()).toMatchObject({ message: '', nextLocation: '', archiveChanged: false });
  expect(f.writes()).toHaveLength(1); expect(f.writes()[0]).toMatchObject({ generation: 3, payload: { type: 'thread.active.reorder', threadId: 'a', commandId: 'one-command-1' } });
  expect(homeOrderState(f.client).pending?.orderedIds).toEqual(['two:b', 'one:a', 'one:c']);
  expect(f.client.environmentId).toBe('one'); expect(f.client.threadId).toBe('a'); expect(f.client.draft).toBe('Retain draft');
});

test('keyless rewrite preflights every environment and allocates IDs through each target endpoint', async () => {
  const f = fixture(undefined, true); await f.run();
  expect(f.writes()).toHaveLength(3);
  expect(f.calls.filter(row => row.op === 'http').map(row => row.fleet ?? '')).toEqual(['', 'two-route']);
  const firstWrite = f.calls.findIndex(row => row.method === 'orchestration.dispatchCommand');
  expect(f.calls.slice(0, firstWrite).filter(row => row.op === 'http')).toHaveLength(2);
  for (const request of f.writes()) expect(str(obj(request.payload).commandId)).toStartWith(request.fleet ? 'two-route-command-' : 'one-command-');
});

test('denied assignment permission prevents every rewrite write', async () => {
  const f = fixture(request => request.op === 'http' && request.fleet ? { ok: true, generation: 8, value: { authenticated: true, permissions: [] } } : undefined, true);
  expect(await f.run()).toMatchObject({ message: 'This connection cannot change threads.' }); expect(f.writes()).toHaveLength(0); expect(homeOrderState(f.client).pending).toBeNull();
});

test('a later refusal preserves earlier canonical assignments and cancels only the optimistic hold', async () => {
  let count = 0;
  const f = fixture(request => {
    if (request.method !== 'orchestration.dispatchCommand') return;
    if (++count === 1) { f.publish(request); return; }
    return { ok: false, generation: request.fleet ? 8 : 3, error: { kind: 'server', message: 'Refused' } };
  }, true);
  expect(await f.run()).toMatchObject({ message: 'Refused', uncertain: false }); expect(f.writes()).toHaveLength(2);
  const first = f.writes()[0]!, shell = first.fleet ? f.remote.shell : f.client.shell;
  expect(shell.threads.find(row => row.id === obj(first.payload).threadId)?.activeOrderKey).toBe(obj(first.payload).orderKey);
  expect(homeOrderState(f.client).pending).toBeNull(); expect(homeOrderState(f.client).busy).toBe(false);
});

test('unknown background write cannot be repeated from either surface until its exact canonical key arrives', async () => {
  const f = fixture(request => request.method === 'orchestration.dispatchCommand' ? { ok: false, generation: request.fleet ? 8 : 3, error: { kind: 'transport', message: 'Unknown', uncertain: true } } : undefined);
  expect(await f.run('move-up', 'b', 'two')).toMatchObject({ uncertain: true }); expect(f.writes()[0]?.fleet).toBe('two-route');
  mobileHomeActionsObserve('sidebar', false, true, f.client);
  await mobileHomeAction('sidebar', 'one', 'a', 'move-down', '', now, f.native, f.client, f.background);
  expect(f.writes()).toHaveLength(1); expect(f.snapshot().blocked).toBe(true);
  f.publish(f.writes()[0]!); expect(f.snapshot().blocked).toBe(false);
});

test('generic write rejection locks the order while definitive refusal allows a retry', async () => {
  const f = fixture(request => request.method === 'orchestration.dispatchCommand' ? Promise.reject(new Error('Transport ended')) : undefined);
  expect(await f.run()).toMatchObject({ uncertain: true }); await f.run(); expect(f.writes()).toHaveLength(1);
  const g = fixture(request => request.method === 'orchestration.dispatchCommand' ? { ok: false, generation: 3, error: { kind: 'server', message: 'Denied' } } : undefined);
  expect(await g.run()).toMatchObject({ uncertain: false }); await g.run(); expect(g.writes()).toHaveLength(2);
});

test('route departure while awaiting grant and generation/capability changes during IDs send no write', async () => {
  const gate = deferred(), started = deferred();
  const f = fixture(request => { if (request.op === 'http') { started.resolve(null); return gate.promise; } });
  const pending = f.run(); await started.promise;
  mobileHomeActionsObserve('left', false, false, f.client); gate.resolve({ ok: true, generation: 3, value: { authenticated: true, permissions: ['orchestration:operate'] } });
  await expect(pending).rejects.toMatchObject({ kind: 'superseded' }); expect(f.writes()).toHaveLength(0);
  const g = fixture(request => { if (request.op === 'ids') g.client.generation++; });
  expect(await g.run()).toMatchObject({ message: 'The connection changed. Refresh the thread order.' }); expect(g.writes()).toHaveLength(0);
  const h = fixture(request => { if (request.op === 'ids') obj(obj(h.client.config.environment).capabilities).threadActiveReorder = false; });
  expect((await h.run()).message).toContain('does not support reordering'); expect(h.writes()).toHaveLength(0);
});

test('double taps share one move owner; shell-before-ACK confirmation releases after the receipt', async () => {
  const gate = deferred(), started = deferred();
  const f = fixture(request => { if (request.method === 'orchestration.dispatchCommand') { f.publish(request); started.resolve(null); return gate.promise; } });
  const pending = f.run(); await started.promise; expect(f.snapshot().pending?.confirmed).toEqual(['one:a']);
  await f.run(); expect(f.writes()).toHaveLength(1);
  gate.resolve({ ok: true, generation: 3, value: {} }); expect(await pending).toMatchObject({ message: '' }); expect(f.snapshot().pending).toBeNull();
});

test('external order change during grant cancels the captured plan before any command', async () => {
  const f = fixture(request => { if (request.op === 'http') f.client.shell.threads[1]!.activeOrderKey = 'y'; });
  expect(await f.run()).toMatchObject({ message: 'The thread order changed. Try the move again.' }); expect(f.writes()).toHaveLength(0);
});

test('queued settled reentry uses the same full section and card menu authority', () => {
  const f = fixture(); f.client.shell.threads[0]!.settledOverride = 'settled';
  mobileHomeOrder(f.client, mobileHomeSources(f.client, f.background), now, { queuedThreadKeys: new Set(['one:a']) });
  const menu = mobileHomeMenu('one', 'a', now, f.client, f.background);
  expect(menu.some(item => item.id === 'settle')).toBe(true);
  expect(menu.find(item => item.id === 'move-down')?.disabled).toBe(false);
  expect(mobileHome(now, {}, f.client, f.background).items.find(item => item.key === 'one:a')).toMatchObject({ section: 'active', card: true, queued: true });
});

test('an uncertain earlier row action blocks Move until canonical evidence confirms it', async () => {
  let fail = true;
  const f = fixture(request => request.method === 'orchestration.dispatchCommand' && fail
    ? { ok: false, generation: 3, error: { kind: 'transport', message: 'Unknown', uncertain: true } } : undefined);
  expect(await mobileHomeAction('visit', 'one', 'a', 'settle', '', now, f.native, f.client, f.background)).toMatchObject({ uncertain: true });
  fail = false; expect(await f.run()).toMatchObject({ uncertain: true }); expect(f.writes()).toHaveLength(1);
  f.client.shell.threads[0]!.settledOverride = 'settled';
  await f.run(); expect(f.writes()).toHaveLength(1);
});

test('wrong-generation write replies remain uncertain; valid old-generation ACKs cancel without replay', async () => {
  const f = fixture(request => request.method === 'orchestration.dispatchCommand' ? { ok: true, generation: 999, value: {} } : undefined);
  expect(await f.run()).toMatchObject({ uncertain: true }); await f.run(); expect(f.writes()).toHaveLength(1);
  const g = fixture(request => { if (request.method === 'orchestration.dispatchCommand') g.client.generation++; });
  expect(await g.run()).toMatchObject({ uncertain: false, message: 'The connection changed. Refresh the thread order.' });
  expect(homeOrderState(g.client).pending).toBeNull(); expect(g.writes()).toHaveLength(1);
});

test('unknown writes remain scoped to their origin and survive route departure', async () => {
  const f = fixture(request => {
    if (request.method !== 'orchestration.dispatchCommand') return;
    mobileHomeActionsObserve('left', false, false, f.client);
    throw new Error('Transport ended');
  });
  expect(await f.run()).toMatchObject({ message: '' }); expect(homeOrderState(f.client).unknown?.origin).toBe('https://one.test');
  f.client.origin = 'https://different.test'; f.publish(f.writes()[0]!);
  expect(f.snapshot().blocked).toBe(true);
  f.client.origin = 'https://one.test'; expect(f.snapshot().blocked).toBe(false);
});

test('pending projection visibly moves the row while preserving filtered neighbors and owner state', async () => {
  const f = fixture(); await f.run('move-down', 'b', 'two');
  const shown = mobileHome(now, {}, f.client, f.background).items.filter(item => item.kind === 'thread');
  expect(shown.map(item => item.key)).toEqual(['one:a', 'one:c', 'two:b']);
  expect(mobileHome(now, { environmentId: 'one' }, f.client, f.background).items.filter(item => item.kind === 'thread').map(item => item.key)).toEqual(['one:a', 'one:c']);
});

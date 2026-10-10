import { expect, test } from 'bun:test';
import { T3Client } from './shared/client';
import { EnvironmentFleet, type FleetEntry } from './shared/settings-b-fleet';
import { initialShell, obj, type Obj } from './shared/domain';
import type { Native } from './shared/protocol';
import { mobileHomeActionsObserve as observe, mobileHomeMenu as menu, mobileHomeAction as action,
  mobileHomeSnoozePresets as presets, mobileHomeSnoozeSelection as snoozeSelection } from './home-actions';
const now = Date.parse('2026-10-07T12:00:00Z');
const capabilities = { serverResolvedCommandContext: true, threadSettlement: true, threadSnooze: true,
  threadPinning: true, threadPinReorder: true, threadAutoSettleOptOut: true, threadTitleRegeneration: true };
const thread = (patch: Obj = {}): Obj => ({ id: 't', projectId: 'p', title: 'Original', branch: 'feat/雪 ?#', worktreePath: '/work/path ?',
  archivedAt: null, deletedAt: null, pinnedAt: null, settledOverride: null, activeRunId: null, latestRunId: null,
  activeProviderThreadId: null, status: 'idle', ...patch });
function deferred<T>() { let resolve!: (value: T) => void; const promise = new Promise<T>(done => { resolve = done; }); return { promise, resolve }; }
function fixture(hook?: (request: Obj) => unknown | Promise<unknown>) {
  const client = new T3Client(), background = new EnvironmentFleet(), calls: Obj[] = []; let serial = 0;
  Object.assign(client, { environmentId: 'one', origin: 'https://one.test', generation: 3, connection: 'connected', configLive: true,
    shellLive: true, threadLive: true, shellLoaded: true, scopes: ['orchestration:operate'], projectId: 'p', threadId: 't' });
  client.config = { environment: { capabilities: { ...capabilities } } }; client.shell.threads = [thread()];
  client.local.drafts['one:t'] = 'Retain draft';
  const native: Native = { available: true, watch() {}, async later(input) {
    const request = obj(input); calls.push(request); const override = await hook?.(request); if (override !== undefined) return override;
    const generation = request.fleet ? 8 : 3;
    return { ok: true, generation, value: request.path === '/api/auth/session' ? { authenticated: true, permissions: ['orchestration:operate'] }
      : request.op === 'ids' ? [`command-${++serial}`] : request.op === 'mobilePrompt' ? { choice: 'submit', text: '  Renamed  ' }
        : request.op === 'mobileAlert' ? { choice: 'delete' } : request.op === 'copyText' ? { copied: true } : {} };
  } };
  observe('route', true, false, client);
  const run = (kind: string, value = '', env = 'one', id = 't') => action('route', env, id, kind, value, now, native, client, background);
  const items = () => menu('one', 't', now, client, background);
  const writes = () => calls.filter(row => row.method === 'orchestration.dispatchCommand');
  const remote = (patch: Obj = {}) => {
    const entry: FleetEntry = { key: 'remote', origin: 'https://two.test', environmentId: 'two', phase: 'connected', message: '', traceId: '', generation: 8,
      synchronized: 8, lastEvent: 0, subscriptions: {}, config: { environment: { capabilities: { ...capabilities } } }, shell: { ...initialShell(), threads: [thread(patch)] }, scopes: ['orchestration:operate'], error: '', requested: true };
    background.entries.set(entry.key, entry); return entry;
  };
  return { client, background, native, calls, run, items, writes, remote };
}

import { mobileHomeSwipe } from './home-actions';
import { homeSwipeGateExpiry, homeSwipeOperation } from './home-swipe';
import { homeOrderState } from './home-order';

test('swipe descriptors preserve source primary, secondary, queued variant, labels and reset identity', () => {
  const f = fixture(), swipe = () => mobileHomeSwipe('one', 't', now, f.client, f.background);
  expect(swipe()).toMatchObject({ primary: 'settle', primaryLabel: 'Settle', primarySymbol: 'checkmark', secondary: 'snooze', snoozable: true });
  expect(swipe().snoozeItems.at(-1)).toMatchObject({ id: 'snooze:custom', operation: 'swipe:snooze:custom' });
  expect(swipe().snoozeItems.some(item => item.operation === 'delete')).toBe(false);
  const key = swipe().resetKey; f.client.shell.threads[0]!.title = 'Renamed'; expect(swipe().resetKey).toBe(key);
  f.client.shell.threads[0]!.settledOverride = 'settled'; expect(swipe().primary).toBe('unsettle'); expect(swipe().secondary).toBe('snooze'); expect(swipe().resetKey).not.toBe(key);
  homeOrderState(f.client).queuedThreadKeys = ['one:t']; expect(swipe().primary).toBe('settle');
  f.client.shell.threads[0]!.snoozedUntil = new Date(now + 3600000).toISOString();
  expect(swipe()).toMatchObject({ primary: 'unsnooze', primaryAccessibilityLabel: 'Wake Renamed now', secondary: '', snoozable: false });
  expect(swipe().snoozeItems).toEqual([]);
  f.client.scopes = []; expect(swipe().primary).toBe(''); expect(swipe().resetKey).toBe('');
});

test('settled and legacy swipe Snooze succeeds without adding Snooze to their context menus', async () => {
  for (const legacy of [false, true]) {
    const f = fixture(); if (legacy) obj(obj(f.client.config.environment).capabilities).threadSettlement = false;
    else f.client.shell.threads[0]!.settledOverride = 'settled';
    expect(f.items().some(item => item.id === 'snooze')).toBe(false);
    expect((await f.run('snooze:hour')).message).toBe('This thread action is no longer available.'); expect(f.writes()).toHaveLength(0);
    expect((await f.run('swipe:snooze:hour')).message).toBe('');
    expect(f.writes()).toHaveLength(1); expect(f.writes()[0]!.payload).toMatchObject({ type: 'thread.snooze', threadId: 't', snoozedUntil: new Date(now + 3600000).toISOString() });
  }
});

test('swipe lifecycle sends only the current primary and preserves source reason and Archive guard', async () => {
  for (const kind of ['settle', 'unsettle', 'unsnooze', 'archive']) {
    const f = fixture(); if (kind === 'unsettle') f.client.shell.threads[0]!.settledOverride = 'settled';
    if (kind === 'unsnooze') f.client.shell.threads[0]!.snoozedUntil = new Date(now + 3600000).toISOString();
    if (kind === 'archive') obj(obj(f.client.config.environment).capabilities).threadSettlement = false;
    expect((await f.run(`swipe:${kind}`)).message).toBe('');
    expect(f.writes()[0]!.payload).toMatchObject({ type: `thread.${kind}`, threadId: 't', ...(kind === 'unsettle' || kind === 'unsnooze' ? { reason: 'user' } : {}) });
  }
  const f = fixture(); await f.run('swipe:unsettle'); expect(f.writes()).toHaveLength(0);
  obj(obj(f.client.config.environment).capabilities).threadSettlement = false;
  Object.assign(f.client.shell.threads[0]!, { status: 'running', latestRunId: 'r', activeRunId: 'r' });
  expect((await f.run('swipe:archive')).message).toContain('Interrupt it first'); expect(f.writes()).toHaveLength(0);
});

test('unknown swipe commands refuse before grants and Snooze raw guards exclude pending and queued work', async () => {
  const f = fixture(); for (const kind of ['swipe:delete', 'swipe:rename', 'swipe:snooze:unknown', 'swipe:arrange', 'swipe:']) {
    expect(homeSwipeOperation(kind)).toBeNull(); expect((await f.run(kind)).message).not.toBe('');
  }
  expect(f.calls).toHaveLength(0);
  for (const patch of [{ pendingRuntimeRequest: { kind: 'approval' } }, { pendingRuntimeRequest: { kind: 'user_input' } },
    { latestUserMessageAt: new Date(now).toISOString() }, { status: 'starting', latestRunId: 'r' }]) {
    const g = fixture(); Object.assign(g.client.shell.threads[0]!, patch);
    expect(mobileHomeSwipe('one', 't', now, g.client, g.background).snoozable).toBe(false);
    await g.run('swipe:snooze:hour'); expect(g.writes()).toHaveLength(0);
  }
  const raw = thread({ latestUserMessageAt: new Date(now).toISOString() });
  expect(homeSwipeGateExpiry(raw, now)).toBe(now + 120000);
  expect(homeSwipeGateExpiry(raw, now + 120001)).toBe(0);
});

test('swipe action rechecks grant, capability, runtime and current primary after IDs', async () => {
  for (const change of ['permission', 'capability', 'pending', 'deleted', 'primary', 'generation']) {
    const f = fixture(request => { if (request.op !== 'ids') return;
      if (change === 'permission') f.client.scopes = [];
      if (change === 'capability') obj(obj(f.client.config.environment).capabilities).threadSnooze = false;
      if (change === 'pending') f.client.shell.threads[0]!.pendingRuntimeRequest = { kind: 'approval' };
      if (change === 'deleted') f.client.shell.threads = [];
      if (change === 'primary') f.client.shell.threads[0]!.snoozedUntil = new Date(now + 3600000).toISOString();
      if (change === 'generation') f.client.generation++;
    });
    await f.run('swipe:snooze:hour').catch(() => undefined); expect(f.writes()).toHaveLength(0);
  }
  const f = fixture(request => request.path === '/api/auth/session' ? { ok: true, generation: 3, value: { authenticated: true, permissions: [] } } : undefined);
  await f.run('swipe:settle'); expect(f.writes()).toHaveLength(0);
});

test('swipe and ordinary actions share the same lock and uncertain-write hold', async () => {
  const gate = deferred<unknown>(), started = deferred<void>();
  const f = fixture(request => { if (request.path === '/api/auth/session') { started.resolve(); return gate.promise; } });
  const pending = f.run('swipe:settle'); await started.promise; await f.run('settle'); await f.run('swipe:settle');
  expect(f.calls.filter(request => request.path === '/api/auth/session')).toHaveLength(1);
  gate.resolve({ ok: true, generation: 3, value: { authenticated: true, permissions: ['orchestration:operate'] } });
  await pending; expect(f.writes()).toHaveLength(1);
  const g = fixture(request => request.method === 'orchestration.dispatchCommand' ? Promise.reject(new Error('lost')) : undefined);
  expect((await g.run('swipe:settle')).uncertain).toBe(true);
  expect((await g.run('settle')).uncertain).toBe(true); expect((await g.run('swipe:settle')).uncertain).toBe(true); expect(g.writes()).toHaveLength(1);
});

test('swipe Custom uses the same picker with explicit invocation and confirmation-time revalidation', async () => {
  for (const legacy of [false, true]) {
    const f = fixture(request => request.op === 'mobileCustomSnooze' ? { ok: true, generation: 3, value: {
      choice: 'snooze', confirmedAt: now + 45000, snoozedUntil: new Date(now + 3600000).toISOString() } } : undefined);
    if (legacy) obj(obj(f.client.config.environment).capabilities).threadSettlement = false;
    else f.client.shell.threads[0]!.settledOverride = 'settled';
    expect((await f.run('swipe:snooze:custom')).message).toBe('');
    expect(f.calls.find(row => row.op === 'mobileCustomSnooze')).toMatchObject({ invocation: 'swipe', origin: 'https://one.test', generation: 3, threadId: 't' });
    expect(f.calls.filter(row => row.path === '/api/auth/session')).toHaveLength(2); expect(f.writes()).toHaveLength(1);
  }
  const g = fixture(request => { if (request.op !== 'mobileCustomSnooze') return;
    g.client.shell.threads[0]!.latestUserMessageAt = new Date(now + 45000).toISOString();
    return { ok: true, generation: 3, value: { choice: 'snooze', confirmedAt: now + 45000, snoozedUntil: new Date(now + 3600000).toISOString() } };
  });
  await g.run('swipe:snooze:custom'); expect(g.writes()).toHaveLength(0);
  const cancelled = fixture(request => request.op === 'mobileCustomSnooze' ? { ok: true, generation: 3, value: { choice: 'cancel' } } : undefined);
  await cancelled.run('swipe:snooze:custom'); expect(cancelled.writes()).toHaveLength(0);
});

test('swipe picker route departure and expired concrete presets never dispatch', async () => {
  const f = fixture(request => { if (request.op !== 'mobileCustomSnooze') return;
    observe('other-route', true, false, f.client);
    return { ok: true, generation: 3, value: { choice: 'snooze', confirmedAt: now, snoozedUntil: new Date(now + 3600000).toISOString() } };
  });
  await expect(f.run('swipe:snooze:custom')).rejects.toMatchObject({ kind: 'superseded' }); expect(f.writes()).toHaveLength(0);
  const g = fixture(), late = new Date(2026, 9, 7, 22).getTime();
  await action('route', 'one', 't', 'swipe:snooze:evening', new Date(late - 3600000).toISOString(), late, g.native, g.client, g.background);
  expect(g.writes()).toHaveLength(0);
});

import { expect, test } from 'bun:test';
import { T3Client } from './shared/client';
import { EnvironmentFleet, type FleetEntry } from './shared/settings-b-fleet';
import { initialShell, obj, type Obj } from './shared/domain';
import type { Native } from './shared/protocol';
import { mobileHomeView } from './home-state';
import { contractComponent, type Env } from '../tests/support/contract-action-fixture';
import { contractFns } from '../tests/support/contract-fn-fixture';
import { mobileHomeActionsObserve as observe, mobileHomeMenu as menu, mobileHomeAction as action,
  mobileHomeSnoozePresets as presets, mobileHomeSnoozeSelection as snoozeSelection } from './home-actions';
import { appText } from '../tests/support/app-files';
const now = Date.parse('2026-10-07T12:00:00Z');

test('custom Home and thread feeds opt out of platform row separators', () => {
  const home = appText('mobile/home.contract'), thread = appText('mobile/thread.contract');
  expect(home).toContain('list id=listId appearance="none"');
  expect(thread).toContain('list id="thread-feed" appearance="none"');
  expect(thread).toContain('scrollFollowEnd=true scroll-behavior="smooth"');
});

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

// Native replies stand for captured row receipts, not elapsed animation time.
// These controller tests cannot qualify the native row registry or applied motion.
function dismissalFixture(writeOutcome: 'success' | 'refused' | 'uncertain' = 'success') {
  const home = deferred<void>(), sidebar = deferred<void>(), checkpoint = deferred<void>();
  let grants = ['orchestration:operate'];
  const f = fixture(request => {
    if (request.path === '/api/auth/session') return { ok: true, generation: 3, value: { authenticated: true, permissions: grants } };
    if (request.op === 'mobileHomeDismissal' && request.phase === 'prepare') return { ok: true, generation: 3, value: { token: 'captured-dismissal-1' } };
    if (request.op === 'mobileHomeDismissal' && request.phase === 'begin') {
      checkpoint.resolve();
      return Promise.all([home.promise, sidebar.promise]).then(() => ({ ok: true, generation: 3,
        value: { token: 'captured-dismissal-1', completed: true } }));
    }
    if (request.op === 'mobileHomeDismissal' && request.phase === 'finish') return { ok: true, generation: 3, value: {} };
    if (request.method === 'orchestration.dispatchCommand') {
      checkpoint.resolve();
      if (writeOutcome !== 'success') return { ok: false, generation: 3,
        error: { kind: 'transport', message: 'Synthetic command refusal', uncertain: writeOutcome === 'uncertain' } };
    }
  });
  const begins = () => f.calls.filter(request => request.op === 'mobileHomeDismissal' && request.phase === 'begin');
  const finishes = () => f.calls.filter(request => request.op === 'mobileHomeDismissal' && request.phase === 'finish');
  const revoke = () => { grants = []; f.client.scopes = []; };
  return { ...f, home, sidebar, checkpoint, begins, finishes, revoke };
}

for (const kind of ['settle', 'swipe:settle', 'swipe:unsettle', 'swipe:unsnooze', 'swipe:archive', 'swipe:snooze:hour']) {
  test(`${kind} waits for both captured Home and sidebar receipts before the actual write`, async () => {
    const f = dismissalFixture(); f.remote();
    if (kind === 'swipe:unsettle') f.client.shell.threads[0]!.settledOverride = 'settled';
    if (kind === 'swipe:unsnooze') f.client.shell.threads[0]!.snoozedUntil = new Date(now + 3600000).toISOString();
    if (kind === 'swipe:archive') obj(obj(f.client.config.environment).capabilities).threadSettlement = false;
    const pending = f.run(kind);
    try {
      await f.checkpoint.promise;
      expect(f.writes()).toHaveLength(0);
      expect(f.begins()).toHaveLength(1);
      expect(f.begins()[0]).toMatchObject({ requestRoute: 'route', environmentId: 'one', threadId: 't', origin: 'https://one.test', generation: 3 });
      f.home.resolve(); await f.home.promise;
      expect(f.writes()).toHaveLength(0);
      f.sidebar.resolve(); expect((await pending).message).toBe('');
      expect(f.writes()).toHaveLength(1);
      expect(f.finishes()).toHaveLength(1);
      expect(f.finishes()[0]).toMatchObject({ token: 'captured-dismissal-1', committed: true });
    } finally { f.home.resolve(); f.sidebar.resolve(); await pending.catch(() => undefined); }
  });
}

for (const change of ['permission', 'route', 'generation', 'deleted', 'primary']) {
  test(`dismissal completion rechecks ${change} and restores the captured ticket without a write`, async () => {
    const f = dismissalFixture(), pending = f.run('swipe:settle');
    try {
      await f.checkpoint.promise;
      expect(f.writes()).toHaveLength(0);
      f.home.resolve(); await f.home.promise;
      if (change === 'permission') f.revoke();
      if (change === 'route') observe('replacement-route', true, false, f.client);
      if (change === 'generation') f.client.generation++;
      if (change === 'deleted') f.client.shell.threads = [];
      if (change === 'primary') f.client.shell.threads[0]!.snoozedUntil = new Date(now + 3600000).toISOString();
      f.sidebar.resolve(); await pending.catch(() => undefined);
      expect(f.writes()).toHaveLength(0);
      expect(f.finishes()).toHaveLength(1);
      expect(f.finishes()[0]).toMatchObject({ token: 'captured-dismissal-1', committed: false });
    } finally { f.home.resolve(); f.sidebar.resolve(); await pending.catch(() => undefined); }
  });
}

for (const outcome of ['refused', 'uncertain'] as const) {
  test(`${outcome} write restores its captured dismissal while retaining the existing outcome policy`, async () => {
    const f = dismissalFixture(outcome), pending = f.run('swipe:settle');
    try {
      await f.checkpoint.promise;
      f.home.resolve(); f.sidebar.resolve();
      const result = await pending;
      expect(result.uncertain).toBe(outcome === 'uncertain'); expect(f.writes()).toHaveLength(1);
      expect(f.finishes()).toHaveLength(1);
      expect(f.finishes()[0]).toMatchObject({ token: 'captured-dismissal-1', committed: false });
    } finally { f.home.resolve(); f.sidebar.resolve(); await pending.catch(() => undefined); }
  });
}

test('controller retains its captured ticket after a sidebar receipt retires and another thread appears', async () => {
  const f = dismissalFixture('refused'), pending = f.run('swipe:settle');
  try {
    await f.checkpoint.promise;
    expect(f.writes()).toHaveLength(0);
    // The native boundary resolves the old sidebar registration on retirement.
    // A replacement registration belongs to a later snapshot, not this await.
    f.sidebar.resolve(); f.client.shell.threads.push(thread({ id: 'replacement' })); f.home.resolve();
    await pending;
    expect(f.begins()).toHaveLength(1); expect(f.finishes()).toHaveLength(1);
    expect(f.finishes()[0]).toMatchObject({ token: 'captured-dismissal-1', committed: false });
    expect(f.finishes()[0]!.row).toBeUndefined();
  } finally { f.home.resolve(); f.sidebar.resolve(); await pending.catch(() => undefined); }
});

// Reuse the app's existing authored-action executor. This runs the real Contract
// state/action/derive bodies, not Swift event delivery or applied-commit receipts.
const dismissalRow = contractComponent('mobile/home.contract', 'HomeListItem');
const swipeFns = contractFns('mobile/home-swipe.contract', [], { floor: Math.floor, parseNumber: (text: string) => {
  const number = Number(text); return text.trim() !== '' && Number.isFinite(number) ? number : undefined;
} });
function dismissalView(f: ReturnType<typeof fixture>, route = 'route', surface = 'compact') {
  return mobileHomeView([0, now, '', 10, true, false, false, true, true, '', '', '', 'repository', route, true, true, [], surface, true], f.client, f.background);
}
function authoredDismissal(reducedMotion = false, f = fixture(), surface = 'compact') {
  const item = () => dismissalView(f, 'route', surface).items.find(row => row.key === 'one:t')!;
  const scope: Env = { item: item(), surface: surface === 'sidebar' ? 'sidebar-list' : 'home-list', active: true, owner: 'home', openToken: '', dragToken: '', dragOffset: 0,
    reducedMotion, colors: {}, min: Math.min, max: Math.max, ...swipeFns };
  const state = dismissalRow.state();
  const derives = () => dismissalRow.derives(state, scope);
  const event = (message: string) => dismissalRow.call('dismissalEvent', state, { ...scope, ...derives() }, [message]);
  scope.resetDismissal = () => dismissalRow.call('resetDismissal', state, scope, []);
  const packet = (phase: string, at = 0, registration = 11, token = 'capture-one') =>
    ['2', (scope.item as { dismissalIdentity: string }).dismissalIdentity, registration, token, phase, at, 320, 72].join('|');
  const retired = (registration = 11, token = 'capture-one', identity = (scope.item as { dismissalIdentity: string }).dismissalIdentity) =>
    ['2', identity, registration, token, 'retired'].join('|');
  return { ...f, item, scope, state, derives, event, packet, retired };
}

test('thread card separators appear between screen rows and never inside sidebar cards', () => {
  for (const surface of ['compact', 'sidebar']) {
    const f = authoredDismissal(false, fixture(), surface);
    const item = f.scope.item as ReturnType<typeof mobileHomeView>['items'][number];
    item.trailingDivider = true;
    expect(dismissalRow.drawn('testId=`home-thread-divider-', f.state, f.scope)).toBe(surface === 'compact');
    item.trailingDivider = false;
    expect(dismissalRow.drawn('testId=`home-thread-divider-', f.state, f.scope)).toBe(false);
    item.trailingDivider = true;
    item.card = false;
    expect(dismissalRow.drawn('testId=`home-thread-divider-', f.state, f.scope)).toBe(false);
  }
});

test('authored dismissal reaches the exact 220/180ms phase boundaries with cubic-out geometry', () => {
  const f = authoredDismissal();
  f.scope.dragToken = (f.scope.item as { swipe: { token: string } }).swipe.token; f.scope.dragOffset = -58;
  f.event(f.packet('out', 1000));
  expect(f.state.exitFrom).toBe(-58); expect(f.derives()).toMatchObject({ exitOffset: -58, swipeTransition: 'none' });
  f.event(f.packet('tick', 1110));
  expect(f.state.exitProgress).toBe(0.5); expect(f.derives().exitEase).toBe(0.875);
  expect(f.derives().exitOffset).toBe(-58 + (-320 + 58) * 0.875);
  f.event(f.packet('tick', 1220));
  expect(f.state).toMatchObject({ exitPhase: 'out-done', exitProgress: 1 });
  // Only the native applied receipt requests collapse; time alone keeps out-done.
  f.event(f.packet('tick', 2000)); expect(f.state.exitPhase).toBe('out-done');
  f.event(f.packet('collapse', 2010));
  f.event(f.packet('tick', 2100)); expect(f.derives()).toMatchObject({ collapsing: true, exitEase: 0.875 });
  expect(72 * (1 - Number(f.derives().exitEase))).toBe(9);
  f.event(f.packet('tick', 2190)); expect(f.state).toMatchObject({ exitPhase: 'collapsed', exitProgress: 1 });
});

test('a retained settled row retires every authored exit slot and resumes ordinary reveal/close springs', () => {
  const home = authoredDismissal(), sidebar = authoredDismissal();
  for (const row of [home, sidebar]) {
    row.event(row.packet('out')); row.event(row.packet('tick', 220));
    row.event(row.packet('collapse', 230)); row.event(row.packet('tick', 410));
  }
  const before = home.item(); home.client.shell.threads[0]!.settledOverride = 'settled';
  const after = home.item(); expect(after.key).toBe(before.key); expect(after.dismissalIdentity).not.toBe(before.dismissalIdentity);
  home.scope.item = after;
  expect(home.derives()).toMatchObject({ exiting: false, exitOffset: 0, swipeTransition: 'none' });
  home.event(home.retired());
  expect(home.state).toEqual(dismissalRow.state());
  expect(sidebar.state).toMatchObject({ exitToken: 'capture-one', exitPhase: 'collapsed' });
  home.scope.openToken = after.swipe.token;
  expect(home.derives()).toMatchObject({ exitOffset: -116, swipeTransition: 'translate -exact-spring(330, 26, 0.7)' });
  home.scope.openToken = '';
  expect(home.derives()).toMatchObject({ exitOffset: 0, swipeTransition: 'translate -exact-spring(330, 26, 0.7)' });
});

test('authored retirement cannot clear another identity, registration or later captured operation', () => {
  const f = authoredDismissal(); f.event(f.packet('out'));
  const before = { ...f.state };
  for (const packet of [f.retired(12), f.retired(11, 'other'), f.retired(11, 'capture-one', 'other-identity')]) {
    f.event(packet); expect(f.state).toEqual(before);
  }
  f.event(f.packet('out', 1000, 12, 'capture-two'));
  const later = { ...f.state };
  f.event(f.retired()); expect(f.state).toEqual(later);
  f.event(f.retired(12, 'capture-two')); expect(f.state).toEqual(dismissalRow.state());
});

test('reduced-motion phases still await receipt events and restoration clears all authored dismissal state', () => {
  const f = authoredDismissal(true);
  f.event(f.packet('out', 1000)); expect(f.state).toMatchObject({ exitPhase: 'out-done', exitProgress: 1 });
  f.event(f.packet('tick', 2000)); expect(f.state.exitPhase).toBe('out-done');
  f.event(f.packet('collapse', 2010)); expect(f.state).toMatchObject({ exitPhase: 'collapsed', exitProgress: 1 });
  f.event(f.packet('restore', 2020)); expect(f.state).toEqual(dismissalRow.state());
  f.scope.reducedMotion = false;
  expect(f.derives()).toMatchObject({ exiting: false, collapsing: false, exitOffset: 0, swipeTransition: 'translate -exact-spring(330, 26, 0.7)' });
});

for (const outcome of ['refused', 'uncertain'] as const) {
  test(`${outcome} recovery changes logical row data after native retirement and clears retained exit state`, async () => {
    const f = dismissalFixture(outcome), home = authoredDismissal(false, f), sidebar = authoredDismissal(false, f, 'sidebar');
    const before = home.item(), token = 'captured-dismissal-1';
    for (const row of [home, sidebar]) {
      row.event(row.packet('out', 0, 11, token)); row.event(row.packet('tick', 220, 11, token));
      row.event(row.packet('collapse', 230, 11, token)); row.event(row.packet('tick', 410, 11, token));
      expect(row.derives()).toMatchObject({ collapsing: true, exitEase: 1 });
    }
    const pending = f.run('swipe:settle');
    try {
      await f.checkpoint.promise; f.home.resolve(); f.sidebar.resolve();
      expect((await pending).uncertain).toBe(outcome === 'uncertain');
      // The captured native views are gone: finish supplies no restore event.
      const recovered = home.item();
      expect(recovered.dismissalIdentity).not.toBe(before.dismissalIdentity);
      expect(recovered.key).toBe(before.key); expect(recovered.swipe.resetKey).toBe(before.swipe.resetKey);
      expect(recovered.nativeMenu).toBe(before.nativeMenu);
      expect(sidebar.item().dismissalIdentity).toBe(recovered.dismissalIdentity);
      expect(home.item()).toEqual(recovered);
      expect(dismissalRow.derives(dismissalRow.state(), { ...home.scope, item: recovered })).toMatchObject({ exiting: false, collapsing: false });
      for (const row of [home, sidebar]) {
        row.scope.item = recovered;
        expect(row.derives()).toMatchObject({ exiting: false, collapsing: false, exitOffset: 0 });
        row.event(row.retired(11, token));
        expect(row.state).toEqual(dismissalRow.state());
        expect(row.derives().swipeTransition).toBe('translate -exact-spring(330, 26, 0.7)');
      }
    } finally { f.home.resolve(); f.sidebar.resolve(); await pending.catch(() => undefined); }
  });
}

test('presentation recovery survives revoked operation permission without admitting a write', async () => {
  const f = dismissalFixture(), scopes = [...f.client.scopes];
  f.client.scopes = [];
  const readOnlyIdentity = dismissalView(f).items.find(row => row.key === 'one:t')!.dismissalIdentity;
  f.client.scopes = scopes;
  const before = dismissalView(f).items.find(row => row.key === 'one:t')!, pending = f.run('swipe:settle');
  try {
    await f.checkpoint.promise; f.revoke(); f.home.resolve(); f.sidebar.resolve(); await pending;
    const after = dismissalView(f).items.find(row => row.key === 'one:t')!;
    expect(after.dismissalIdentity).not.toBe(readOnlyIdentity);
    expect(after.menuItems).toEqual([]); expect(after.swipe.resetKey).toBe(''); expect(f.writes()).toHaveLength(0);
    f.client.scopes = scopes;
    const granted = dismissalView(f).items.find(row => row.key === 'one:t')!;
    expect(granted.dismissalIdentity).not.toBe(before.dismissalIdentity);
    expect(granted.swipe.resetKey).toBe(before.swipe.resetKey);
  } finally { f.home.resolve(); f.sidebar.resolve(); await pending.catch(() => undefined); }
});

test('acknowledged dismissal and actions with no captured dismissal retain the base projection identity', async () => {
  const f = dismissalFixture(), before = dismissalView(f).items.find(row => row.key === 'one:t')!, pending = f.run('swipe:settle');
  try {
    await f.checkpoint.promise; f.home.resolve(); f.sidebar.resolve(); await pending;
    expect(dismissalView(f).items.find(row => row.key === 'one:t')).toEqual(before);
  } finally { f.home.resolve(); f.sidebar.resolve(); await pending.catch(() => undefined); }
  const uncaptured = fixture(), original = dismissalView(uncaptured).items.find(row => row.key === 'one:t');
  await uncaptured.run('swipe:settle'); await uncaptured.run('swipe:unsettle');
  expect(dismissalView(uncaptured).items.find(row => row.key === 'one:t')).toEqual(original);
});

test('a later acknowledged attempt preserves recovery while another failure advances it exactly once', async () => {
  const f = dismissalFixture('refused'), later = f.native.later;
  let acknowledge = false;
  f.native.later = async input => {
    const request = obj(input);
    if (acknowledge && request.method === 'orchestration.dispatchCommand') {
      f.calls.push(request); return { ok: true, generation: 3, value: {} };
    }
    return later(input);
  };
  const identity = () => dismissalView(f).items.find(row => row.key === 'one:t')!.dismissalIdentity;
  const original = identity(); f.home.resolve(); f.sidebar.resolve();
  await f.run('swipe:settle'); const recovered = identity(); expect(recovered).not.toBe(original);
  acknowledge = true; await f.run('swipe:settle'); expect(identity()).toBe(recovered);
  acknowledge = false; await f.run('swipe:settle'); const repeated = identity(); expect(repeated).not.toBe(recovered);
  expect(identity()).toBe(repeated); expect(f.writes()).toHaveLength(3);
});

for (const change of ['route', 'origin', 'generation', 'reset', 'departure']) {
  test(`a retired ${change} attempt cannot recover its successor, while a fresh attempt still can`, async () => {
    const f = dismissalFixture('refused'), original = { ...f.client.shell.threads[0]! };
    const before = dismissalView(f).items.find(row => row.key === 'one:t')!, pending = f.run('swipe:settle');
    try {
      await f.checkpoint.promise;
      if (change === 'origin') f.client.origin = 'https://replacement.test';
      if (change === 'generation') f.client.generation++;
      if (change === 'reset') f.client.shell.threads[0]!.snoozedUntil = new Date(now + 3600000).toISOString();
      if (change === 'departure') f.client.shell.threads = [];
      const successor = dismissalView(f, change === 'route' ? 'other-route' : 'route').items.find(row => row.key === 'one:t');
      f.home.resolve(); f.sidebar.resolve(); await pending.catch(() => undefined);
      expect(dismissalView(f, change === 'route' ? 'other-route' : 'route').items.find(row => row.key === 'one:t')).toEqual(successor);
      f.client.origin = 'https://one.test'; f.client.generation = 3; f.client.shell.threads = [{ ...original }];
      expect(dismissalView(f).items.find(row => row.key === 'one:t')!.dismissalIdentity).toBe(before.dismissalIdentity);
      await f.run('swipe:settle');
      expect(dismissalView(f).items.find(row => row.key === 'one:t')!.dismissalIdentity).not.toBe(before.dismissalIdentity);
      f.client.shell.threads = []; expect(dismissalView(f).items).toEqual([]);
      f.client.shell.threads = [{ ...original }];
      expect(dismissalView(f).items.find(row => row.key === 'one:t')!.dismissalIdentity).toBe(before.dismissalIdentity);
    } finally { f.home.resolve(); f.sidebar.resolve(); await pending.catch(() => undefined); }
  });
}

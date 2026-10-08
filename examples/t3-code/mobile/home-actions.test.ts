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

test('ordinary source menus preserve lifecycle order, title actions, submenus and custom picker', () => {
  const f = fixture(); const top = () => f.items().filter(item => !item.parentId).map(item => item.id);
  expect(top()).toEqual(['new-thread-on-branch', 'copy-thread-id', 'settle', 'snooze', 'pin', 'rename', 'regenerate-title', 'auto-settle', 'delete']);
  expect(f.items().find(item => item.id === 'delete')?.destructive).toBe(true);
  expect(f.items().find(item => item.id === 'snooze:custom')?.disabled).toBe(false);
  expect(f.items().filter(item => item.parentId === 'auto-settle').map(item => item.checked)).toEqual([true, false]);
  Object.assign(f.client.shell.threads[0]!, { settledOverride: 'settled' });
  expect(top()).toEqual(['new-thread-on-branch', 'copy-thread-id', 'unsettle', 'pin', 'rename', 'regenerate-title', 'auto-settle', 'delete']);
  Object.assign(f.client.shell.threads[0]!, { snoozedUntil: new Date(now + 3600000).toISOString() });
  expect(top()).toEqual(['new-thread-on-branch', 'copy-thread-id', 'unsnooze', 'rename', 'regenerate-title', 'auto-settle', 'delete']);
  f.client.shell.threads[0] = thread({ pinnedAt: '2026-01-01' }); expect(top()).toContain('unpin'); expect(top()).not.toContain('pin');
  f.client.config = { environment: { capabilities: { ...capabilities, threadSettlement: false } } };
  expect(top()).toEqual(['new-thread-on-branch', 'copy-thread-id', 'archive', 'arrange-open', 'move-up', 'move-down', 'unpin', 'rename', 'regenerate-title', 'delete']);
  expect(f.calls).toHaveLength(0);
});

test('readonly, disconnected, archived, deleted and subagent rows have no menu or write', async () => {
  for (const patch of [{ archivedAt: 'date' }, { deletedAt: 'date' }, { lineage: { relationshipToParent: 'subagent' } }]) {
    const f = fixture(); Object.assign(f.client.shell.threads[0]!, patch); expect(f.items()).toEqual([]); await f.run('delete'); expect(f.writes()).toHaveLength(0);
  }
  const f = fixture(); f.client.scopes = []; expect(f.items()).toEqual([]); await f.run('pin'); expect(f.calls).toHaveLength(0);
  f.client.scopes = ['orchestration:operate']; f.client.connection = 'connecting'; expect(f.items()).toEqual([]);
});

test('native Rename trims, preserves draft and selection, while empty/noop/cancel never write', async () => {
  const f = fixture(); expect(await f.run('rename')).toMatchObject({ message: '', nextLocation: '', archiveChanged: false });
  expect(f.writes()[0]?.payload).toEqual({ type: 'thread.metadata.update', threadId: 't', title: 'Renamed', commandId: 'command-1' });
  expect(f.calls.filter(item => item.path === '/api/auth/session')).toHaveLength(2);
  expect(f.client.threadId).toBe('t'); expect(f.client.local.drafts['one:t']).toBe('Retain draft');
  for (const answer of [{ choice: 'cancel' }, { choice: 'submit', text: ' Original ' }, { choice: 'submit', text: '   ' }]) {
    const g = fixture(request => request.op === 'mobilePrompt' ? { ok: true, generation: 3, value: answer } : undefined);
    const result = await g.run('rename'); expect(g.writes()).toHaveLength(0);
    expect(result.message).toBe(answer.text === '   ' ? 'Thread title cannot be empty.' : '');
  }
});

test('Delete confirms the exact title and dispatches only non-worktree deletion; cancel does nothing', async () => {
  const f = fixture(); expect(await f.run('delete')).toMatchObject({ archiveChanged: true, message: '', nextLocation: '' });
  expect(f.calls.find(item => item.op === 'mobileAlert')).toMatchObject({ title: 'Delete thread?', message: '“Original” will be permanently deleted, including its terminal history.' });
  expect(f.writes()[0]?.payload).toEqual({ type: 'thread.delete', threadId: 't', commandId: 'command-1' });
  expect(f.calls.some(item => item.op === 'connect' || item.method === 'terminal.close' || item.method === 'vcs.removeWorktree')).toBe(false);
  const g = fixture(request => request.op === 'mobileAlert' ? { ok: true, generation: 3, value: { choice: 'cancel' } } : undefined);
  await g.run('delete'); expect(g.writes()).toHaveLength(0);
});

test('background endpoint keeps colliding thread identity and focused draft unchanged', async () => {
  const f = fixture(); f.remote({ title: 'Remote' }); const result = await f.run('rename', '', 'two');
  expect(result.message).toBe(''); expect(f.writes()[0]).toMatchObject({ fleet: 'remote', generation: 8, payload: { threadId: 't', type: 'thread.metadata.update' } });
  expect(f.calls.filter(item => item.path === '/api/auth/session').every(item => item.fleet === 'remote')).toBe(true);
  expect(f.client.environmentId).toBe('one'); expect(f.client.threadId).toBe('t'); expect(f.client.draft).toBe('Retain draft');
});

test('fresh grant denial and revocation after Delete confirmation dispatch nothing', async () => {
  let grants = 0;
  const f = fixture(request => request.path === '/api/auth/session' ? { ok: true, generation: 3, value: { authenticated: true, permissions: ++grants === 1 ? ['orchestration:operate'] : [] } } : undefined);
  expect(await f.run('delete')).toMatchObject({ message: 'This connection cannot change threads.', alertTitle: 'Could not delete thread' }); expect(f.writes()).toHaveLength(0);
  const g = fixture(request => request.path === '/api/auth/session' ? { ok: true, generation: 3, value: { authenticated: true, scopes: ['orchestration:operate'], permissions: [] } } : undefined);
  expect(await g.run('pin')).toMatchObject({ message: 'This connection cannot change threads.' }); expect(g.writes()).toHaveLength(0);
});

test('source lifecycle payloads, archive runtime gate and global pin ordering', async () => {
  const f = fixture(); f.remote({ id: 'other', pinnedAt: 'date', pinOrderKey: 'b' }); await f.run('pin');
  expect(obj(f.writes()[0]?.payload).orderKey < 'b').toBe(true);
  await f.run('settle'); expect(f.writes().at(-1)?.payload).toMatchObject({ type: 'thread.settle' });
  f.client.shell.threads[0] = thread({ settledOverride: 'settled' }); await f.run('unsettle'); expect(f.writes().at(-1)?.payload).toMatchObject({ type: 'thread.unsettle', reason: 'user' });
  f.client.shell.threads[0] = thread({ snoozedUntil: new Date(now + 3600000).toISOString() }); await f.run('unsnooze'); expect(f.writes().at(-1)?.payload).toMatchObject({ type: 'thread.unsnooze', reason: 'user' });
  f.client.shell.threads[0] = thread(); await f.run('auto-settle:disabled'); expect(f.writes().at(-1)?.payload).toMatchObject({ type: 'thread.auto-settle.set', enabled: false });
  await f.run('regenerate-title'); expect(f.writes().at(-1)?.payload).toMatchObject({ type: 'thread.metadata.update', regenerateTitle: true });
  f.client.config = { environment: { capabilities: { ...capabilities, threadSettlement: false } } };
  f.client.shell.threads[0] = thread({ latestRunId: 'run', activityRunStatus: 'running' }); const count = f.writes().length;
  expect(await f.run('archive')).toMatchObject({ message: 'This thread is working. Interrupt it first, then try again.' }); expect(f.writes()).toHaveLength(count);
  f.client.shell.threads[0] = thread(); expect(await f.run('archive')).toMatchObject({ archiveChanged: true });
});

test('branch navigation keeps literal query data; Copy acknowledges actual clipboard capability', async () => {
  const f = fixture(); const result = await f.run('new-thread-on-branch'), url = new URL(result.nextLocation, 'https://app.test');
  expect(url.pathname).toBe('/new/draft'); expect(Object.fromEntries(url.searchParams)).toEqual({ environmentId: 'one', projectId: 'p', branch: 'feat/雪 ?#', worktreePath: '/work/path ?' }); expect(f.writes()).toHaveLength(0);
  await f.run('copy-thread-id'); expect(f.calls.find(item => item.op === 'copyText')).toMatchObject({ text: 't' });
  const g = fixture(request => request.op === 'copyText' ? { ok: true, generation: 3, value: { copied: false } } : undefined);
  expect(await g.run('copy-thread-id')).toMatchObject({ message: 'Clipboard access is unavailable in this session.' });
});

test('snooze resolves relative presets on tap, retains vanished future calendar time and leaves custom to its picker', async () => {
  const shown = presets(now).find(item => item.id === 'hour')!;
  expect(snoozeSelection('snooze:hour', shown.value, now + 60000)).toBe(new Date(now + 3660000).toISOString());
  const late = new Date(2026, 9, 7, 17, 30).getTime(), evening = new Date(2026, 9, 7, 18).toISOString();
  expect(snoozeSelection('snooze:evening', evening, late)).toBe(evening);
  expect(snoozeSelection('snooze:evening', evening, late + 3600000)).toBe(''); expect(snoozeSelection('snooze:custom', evening, late)).toBe('');
  const f = fixture(); await f.run('snooze:hour', shown.value); expect(f.writes()[0]?.payload).toMatchObject({ type: 'thread.snooze', snoozedUntil: shown.value });
  const before = f.writes().length; await f.run('snooze:custom'); expect(f.writes()).toHaveLength(before);
});

test('route departure/reentry invalidates a pending prompt and double tap acquires one operation', async () => {
  const started = deferred<void>(), gate = deferred<unknown>();
  const f = fixture(request => { if (request.op === 'mobilePrompt') { started.resolve(); return gate.promise; } });
  const pending = f.run('rename'); await started.promise; await f.run('rename'); expect(f.calls.filter(item => item.op === 'mobilePrompt')).toHaveLength(1);
  observe('elsewhere', false, false, f.client); observe('route', true, false, f.client);
  gate.resolve({ ok: true, generation: 3, value: { choice: 'submit', text: 'Wrong route' } });
  await expect(pending).rejects.toMatchObject({ kind: 'superseded' }); expect(f.writes()).toHaveLength(0);
});

test('identical observe calls retain ownership, while row deletion or generation change prevents dispatch', async () => {
  const f = fixture(request => { if (request.op === 'mobilePrompt') observe('route', true, false, f.client); });
  expect(await f.run('rename')).toMatchObject({ message: '' }); expect(f.writes()).toHaveLength(1);
  const g = fixture(request => { if (request.op === 'mobileAlert') g.client.shell.threads = []; });
  expect(await g.run('delete')).toMatchObject({ message: 'That thread is no longer available.' }); expect(g.writes()).toHaveLength(0);
  const h = fixture(request => { if (request.path === '/api/auth/session') h.client.generation++; });
  expect(await h.run('pin')).toMatchObject({ message: 'The connection changed. Refresh this thread.' }); expect(h.writes()).toHaveLength(0);
});

test('unknown write outcome suppresses repeat until canonical desired state arrives; explicit refusal may retry', async () => {
  const f = fixture(request => request.method === 'orchestration.dispatchCommand' ? Promise.reject(new Error('Connection lost')) : undefined);
  expect(await f.run('pin')).toMatchObject({ uncertain: true }); await f.run('pin'); expect(f.writes()).toHaveLength(1);
  const payload = obj(f.writes()[0]?.payload); Object.assign(f.client.shell.threads[0]!, { pinnedAt: 'date', pinOrderKey: payload.orderKey });
  await f.run('unpin'); expect(f.writes()).toHaveLength(2);
  const g = fixture(request => request.method === 'orchestration.dispatchCommand' ? { ok: false, generation: 3, error: { kind: 'server', message: 'Denied', uncertain: false } } : undefined);
  expect(await g.run('pin')).toMatchObject({ uncertain: false, message: 'Denied' }); await g.run('pin'); expect(g.writes()).toHaveLength(2);
});

test('explicit uncertain bridge refusal preserves duplicate protection on background writes', async () => {
  const f = fixture(request => request.method === 'orchestration.dispatchCommand' ? { ok: false, generation: 8, error: { kind: 'transport', message: 'Unknown outcome', uncertain: true } } : undefined);
  f.remote(); expect(await f.run('pin', '', 'two')).toMatchObject({ uncertain: true }); await f.run('pin', '', 'two'); expect(f.writes()).toHaveLength(1);
});

test('all results have exactly the closed Contract fields and never carry command/prompt helpers', async () => {
  const f = fixture(); const result = await f.run('pin');
  expect(Object.keys(result).sort()).toEqual(['alertTitle', 'archiveChanged', 'message', 'nextLocation', 'requestRoute', 'revision', 'uncertain']);
});


test('Archive becoming running while native identifiers are allocated is refused before dispatch', async () => {
  const f = fixture(request => { if (request.op === 'ids') f.client.shell.threads[0] = thread({ latestRunId: 'run', activityRunStatus: 'running' }); });
  f.client.config = { environment: { capabilities: { ...capabilities, threadSettlement: false } } };
  expect(await f.run('archive')).toMatchObject({ message: 'This thread is working. Interrupt it first, then try again.' }); expect(f.writes()).toHaveLength(0);
});

test('Archive uses source presented idle runtime when background work holds completion', async () => {
  const f = fixture(); f.client.config = { environment: { capabilities: {} } };
  f.client.shell.threads[0] = thread({ latestRunId: 'run', activeProviderThreadId: 'provider', status: 'running', pendingBackgroundTasks: [{ kind: 'monitor' }] });
  expect(await f.run('archive')).toMatchObject({ message: '', archiveChanged: true }); expect(f.writes()).toHaveLength(1);
});

test('changed capabilities and snooze guards report the source-specific errors; pending title regeneration is a no-op', async () => {
  const f = fixture(); f.client.shell.threads[0] = thread({ pendingRuntimeRequest: { kind: 'approval' } });
  expect(await f.run('snooze:hour')).toMatchObject({ message: 'This thread is waiting on you. Respond to the pending request before snoozing it.' });
  f.client.shell.threads[0] = thread({ latestRunId: 'run', status: 'starting' });
  expect(await f.run('snooze:hour')).toMatchObject({ message: "This thread is still starting a turn. Try again once it's running." });
  f.client.shell.threads[0] = thread({ titleRegeneration: { requestId: 'running' } });
  expect(await f.run('regenerate-title')).toMatchObject({ message: '', alertTitle: '' });
  f.client.config = { environment: { capabilities: {} } };
  expect(await f.run('pin')).toMatchObject({ message: "This environment's server does not support pinning yet. Update the server to use Pin." });
  expect(f.writes()).toHaveLength(0);
});

const customAnswer = (patch: Obj = {}, generation = 3) => ({ ok: true, generation,
  value: { choice: 'snooze', snoozedUntil: '2026-10-07T13:37:42.123Z', confirmedAt: now + 600000, ...patch } });

test('Custom Snooze preserves the native confirmation ISO and uses a fresh grant', async () => {
  const f = fixture(request => request.op === 'mobileCustomSnooze' ? customAnswer() : undefined);
  expect(await f.run('snooze:custom', 'ignored displayed preset')).toMatchObject({ message: '', nextLocation: '', uncertain: false });
  expect(f.calls.find(row => row.op === 'mobileCustomSnooze')).toEqual({ op: 'mobileCustomSnooze', requestRoute: 'route', environmentId: 'one', threadId: 't', origin: 'https://one.test', generation: 3 });
  expect(f.calls.filter(row => row.path === '/api/auth/session')).toHaveLength(2);
  expect(f.writes()[0]?.payload).toEqual({ type: 'thread.snooze', threadId: 't', snoozedUntil: '2026-10-07T13:37:42.123Z', commandId: 'command-1' });
  expect(f.client.threadId).toBe('t'); expect(f.client.draft).toBe('Retain draft');
});

test('Custom Snooze cancellation releases the row lock and never allocates a command', async () => {
  const f = fixture(request => request.op === 'mobileCustomSnooze' ? customAnswer({ choice: 'cancel' }) : undefined);
  await f.run('snooze:custom'); await f.run('snooze:custom');
  expect(f.calls.filter(row => row.op === 'mobileCustomSnooze')).toHaveLength(2);
  expect(f.calls.some(row => row.op === 'ids')).toBe(false); expect(f.writes()).toHaveLength(0);
});

test('Custom Snooze rejects malformed dates and confirmation clocks before a second grant or write', async () => {
  for (const patch of [{ snoozedUntil: '' }, { snoozedUntil: 'tomorrow' }, { snoozedUntil: '2026-02-30T13:00:00.000Z' },
    { snoozedUntil: '2026-10-07T13:37:42+00:00' }, { confirmedAt: undefined }, { confirmedAt: '1791374400000' },
    { confirmedAt: Date.parse('2026-10-07T13:37:42.123Z') }, { confirmedAt: Date.parse('2026-10-07T13:37:42.124Z') },
    { confirmedAt: NaN }, { confirmedAt: Infinity }, { confirmedAt: 1.5 }, { confirmedAt: 8.64e15 + 1 }]) {
    const f = fixture(request => request.op === 'mobileCustomSnooze' ? customAnswer(patch) : undefined);
    expect(await f.run('snooze:custom')).toMatchObject({ message: 'The snooze picker returned an invalid date and time.', uncertain: false });
    expect(f.calls.filter(row => row.path === '/api/auth/session')).toHaveLength(1); expect(f.writes()).toHaveLength(0);
  }
});

test('Custom Snooze background identity and generation stay scoped while the picker is local', async () => {
  const f = fixture(request => request.op === 'mobileCustomSnooze' ? customAnswer({}, 8) : undefined); f.remote();
  expect(await f.run('snooze:custom', '', 'two')).toMatchObject({ message: '' });
  expect(f.calls.find(row => row.op === 'mobileCustomSnooze')).toMatchObject({ environmentId: 'two', threadId: 't', generation: 8, origin: 'https://two.test' });
  expect(f.calls.find(row => row.op === 'mobileCustomSnooze')?.fleet).toBeUndefined();
  expect(f.calls.filter(row => row.path === '/api/auth/session').every(row => row.fleet === 'remote')).toBe(true);
  expect(f.writes()[0]).toMatchObject({ fleet: 'remote', generation: 8, payload: { threadId: 't', type: 'thread.snooze' } });
  expect(f.client.environmentId).toBe('one'); expect(f.client.draft).toBe('Retain draft');
});

test('Custom Snooze uses confirmation time to reject newly queued work after a long picker wait', async () => {
  const started = deferred<void>(), gate = deferred<unknown>();
  const f = fixture(request => { if (request.op === 'mobileCustomSnooze') { started.resolve(); return gate.promise; } });
  const pending = f.run('snooze:custom'); await started.promise;
  f.client.shell.threads[0]!.latestUserMessageAt = new Date(now + 599000).toISOString();
  gate.resolve(customAnswer());
  expect(await pending).toMatchObject({ message: "This thread is still starting a turn. Try again once it's running." });
  expect(f.writes()).toHaveLength(0);
  const g = fixture(request => { if (request.op === 'mobileCustomSnooze') { g.client.shell.threads[0]!.latestUserMessageAt = new Date(now + 60000).toISOString(); return customAnswer(); } });
  expect(await g.run('snooze:custom')).toMatchObject({ message: '' }); expect(g.writes()).toHaveLength(1);
});

test('Custom Snooze double taps share one picker and a stale route or endpoint never dispatches', async () => {
  for (const change of ['route', 'generation', 'origin', 'disconnect']) {
    const started = deferred<void>(), gate = deferred<unknown>();
    const f = fixture(request => { if (request.op === 'mobileCustomSnooze') { started.resolve(); return gate.promise; } });
    const pending = f.run('snooze:custom'); await started.promise; await f.run('snooze:custom');
    expect(f.calls.filter(row => row.op === 'mobileCustomSnooze')).toHaveLength(1);
    if (change === 'route') observe('other', false, false, f.client);
    if (change === 'generation') f.client.generation++;
    if (change === 'origin') f.client.origin = 'https://replacement.test';
    if (change === 'disconnect') f.client.connection = 'disconnected';
    gate.resolve(customAnswer());
    if (change === 'route') await expect(pending).rejects.toMatchObject({ kind: 'superseded' });
    else expect(await pending).toMatchObject({ message: 'The connection changed. Refresh this thread.', uncertain: false });
    expect(f.writes()).toHaveLength(0);
  }
});

test('Custom Snooze refuses changed grants, capabilities, membership and late pending input', async () => {
  let grants = 0;
  const denied = fixture(request => request.op === 'mobileCustomSnooze' ? customAnswer()
    : request.path === '/api/auth/session' ? { ok: true, generation: 3, value: { authenticated: true, permissions: ++grants === 1 ? ['orchestration:operate'] : [] } } : undefined);
  expect(await denied.run('snooze:custom')).toMatchObject({ message: 'This connection cannot change threads.' }); expect(denied.writes()).toHaveLength(0);
  for (const change of ['capability', 'deleted', 'settled', 'pending-input']) {
    const f = fixture(request => {
      if (request.op === 'mobileCustomSnooze') {
        if (change === 'capability') obj(obj(f.client.config.environment).capabilities).threadSnooze = false;
        if (change === 'deleted') f.client.shell.threads = [];
        if (change === 'settled') f.client.shell.threads[0]!.settledOverride = 'settled';
        return customAnswer();
      }
      if (request.op === 'ids' && change === 'pending-input') f.client.shell.threads[0]!.pendingRuntimeRequest = { kind: 'user_input' };
    });
    expect((await f.run('snooze:custom')).message).not.toBe(''); expect(f.writes()).toHaveLength(0);
  }
});

test('Custom Snooze keeps the confirmed ISO fixed through network delay and protects uncertain writes', async () => {
  let prompts = 0;
  const f = fixture(request => {
    if (request.op === 'mobileCustomSnooze') { prompts++; return customAnswer(); }
    if (request.method === 'orchestration.dispatchCommand') return { ok: false, generation: 3, error: { kind: 'transport', message: 'Unknown', uncertain: true } };
  });
  expect(await f.run('snooze:custom')).toMatchObject({ uncertain: true }); await f.run('snooze:custom');
  expect(prompts).toBe(1); expect(f.writes()).toHaveLength(1);
  f.client.shell.threads[0]!.snoozedUntil = '2026-10-07T13:37:42.123Z';
  await f.run('unsnooze'); expect(f.writes()).toHaveLength(2);
  const g = fixture(request => request.op === 'mobileCustomSnooze' ? customAnswer()
    : request.method === 'orchestration.dispatchCommand' ? { ok: true, generation: 999, value: {} } : undefined);
  expect(await g.run('snooze:custom')).toMatchObject({ uncertain: true }); await g.run('snooze:custom'); expect(g.writes()).toHaveLength(1);
});

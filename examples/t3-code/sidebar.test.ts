import { describe, expect, test } from 'bun:test';
import type { T3Client } from './client';
import type { Obj } from './domain';
import type { Files, Native } from './protocol';
import { toasts } from './toast';
import { ageLabel, effectiveSnoozed, isWoke, orderKeyBetween, planReorder, recedes, sectionOf, sidebarStatus, snoozeWakeLabel,
  sortActive, sortPinned, topStatus, unseenCompletion, wokeAt, workingDuration, capabilities, sidebarVisible } from './sidebar-model';
import { bulkMenuItems, nativeTemplate, threadMenuItems } from './sidebar-menu';
import { sidebarSnapshot, terminalProcessCount } from './sidebar-view';
import { legacySidebarSnapshot } from './legacy-sidebar-view';
import { threadItems } from './palette';
import { terminalMetadataEvent } from './terminal-drawer-view';
import { recordTerminalFocus } from './terminal-focus';
import { sidebarCommand, sidebarLocal, sidebarSelecting, undoLatest, visitOpenThread } from './sidebar-commands';
import { resolveCustomSnooze, sidebarPrefs, sidebarSession } from './sidebar-state';

const NOW = Date.parse('2026-10-04T12:00:00.000Z');
const iso = (offset: number) => new Date(NOW + offset).toISOString();
const ALL_CAPS = { threadSettlement: true, threadSnooze: true, threadPinning: true, threadPinReorder: true, threadActiveReorder: true,
  threadAutoSettleOptOut: true, threadTitleRegeneration: true, threadVisitedTracking: true };
const shell = (id: string, extra: Obj = {}): Obj => ({ id, projectId: 'p1', title: `Thread ${id}`, status: 'idle', latestRunId: null,
  activeProviderThreadId: null, pendingRuntimeRequest: null, createdAt: iso(-3_600_000), updatedAt: iso(-3_600_000), archivedAt: null,
  settledOverride: null, settledAt: null, modelSelection: { instanceId: 'codex', model: 'm' }, lineage: { relationshipToParent: null }, ...extra });

interface Fake { client: T3Client; dispatched: Obj[]; opened: string[]; drafts: string[]; calls: Obj[]; picks: string[] }
function fake(threads: Obj[], options: { caps?: Obj; settings?: Obj; threadId?: string; picks?: string[]; modifiers?: Obj } = {}): Fake {
  const dispatched: Obj[] = [], opened: string[] = [], drafts: string[] = [], calls: Obj[] = [], picks = [...(options.picks ?? [])];
  let ids = 0;
  const client = {
    shell: { projects: [{ id: 'p1', title: 'Parity fixture', workspaceRoot: '/fixture' }, { id: 'p2', title: 'Other', workspaceRoot: '/other' }], threads, sequence: 1 },
    config: { environment: { capabilities: { ...ALL_CAPS, ...(options.caps ?? {}) } }, providers: [], keybindings: [] },
    environmentId: 'env', threadId: options.threadId ?? '', projectId: 'p1', query: '', connection: 'connected', writable: true, ready: true,
    presentation: {}, local: { drafts: {}, snapshotDrafts: {}, snapshotReleases: [], deviceSettings: { timestampFormat: '24-hour' }, clientSettings: { confirmThreadArchive: false, confirmThreadDelete: true, confirmThreadUnpin: false, sidebarWorkingShelfEnabled: false, ...(options.settings ?? {}) }, sidebarWidth: 256 },
    projectGroups() { return [{ key: 'g1', name: 'Parity fixture', members: [{ id: 'p1' }] }, { key: 'g2', name: 'Other', members: [{ id: 'p2' }] }]; },
    restAccess: () => ({
      ids: async (count: number) => Array.from({ length: count }, () => `c${ids++}`),
      request: async (method: string, payload: Obj) => { if (method === 'orchestration.dispatchCommand') dispatched.push(payload); return {}; },
      call: async (request: Obj) => {
        calls.push(request);
        if (request.op === 'sidebarMenu') return { id: picks.shift() ?? null };
        if (request.op === 'sidebarModifiers') return options.modifiers ?? {};
        return {};
      },
    }),
    async openSelected(_native: Native, id: string) { opened.push(id); client.threadId = id; },
    async openDraft(_native: Native, projectId: string) { drafts.push(projectId); client.threadId = ''; },
  } as unknown as T3Client & { threadId: string };
  return { client, dispatched, opened, drafts, calls, picks };
}
const native = { available: true, watch() {}, later: async () => ({}) } as unknown as Native;
const files = {} as Files;
const helpers = { projectIdentity: (name: string) => ({ projectMark: name.slice(0, 2).toUpperCase(), projectInk: 'ink', projectSurface: 'surface' }),
  providerBadge: () => ({ providerBadge: '', providerBadgeColor: '' }) };
const strip = (payloads: Obj[]) => payloads.map(({ commandId: _id, ...rest }) => rest);

describe('sidebar model (Sidebar.logic.ts, client-runtime models/threadSettled/threadSort)', () => {
  test('status: approval and input outrank work; idle with background tasks waits; usage limits read Limited', () => {
    expect(sidebarStatus(shell('a', { pendingRuntimeRequest: { kind: 'command_approval' } }))).toBe('approval');
    expect(sidebarStatus(shell('a', { pendingRuntimeRequest: { kind: 'user_input' } }))).toBe('input');
    expect(sidebarStatus(shell('a', { latestRunId: 'r', status: 'running' }))).toBe('working');
    expect(sidebarStatus(shell('a', { latestRunId: 'r', status: 'completed', pendingBackgroundTasks: [{ id: 'x' }] }))).toBe('waiting');
    expect(sidebarStatus(shell('a', { latestRunId: 'r', status: 'failed', lastErrorClass: 'usage_limit' }))).toBe('limited');
    expect(sidebarStatus(shell('a', { latestRunId: 'r', status: 'failed' }))).toBe('failed');
    expect(sidebarStatus(shell('a', { latestRunId: 'r', status: 'completed' }))).toBe('ready');
  });

  test('pills carry the reference labels, icons and hues', () => {
    expect(topStatus('working', false, false)).toEqual({ label: 'Working', icon: 'circle-dashed', color: '#2b7fff' });
    expect(topStatus('waiting', false, false)?.label).toBe('Waiting');
    expect(topStatus('approval', false, false)).toMatchObject({ label: 'Approval', icon: 'shield-question' });
    expect(topStatus('input', false, false)).toMatchObject({ label: 'Input', icon: 'message-circle-question', color: 'light-dark(#4f39f6, #a3b3ff)' });
    expect(topStatus('limited', false, false)).toMatchObject({ label: 'Limited', color: '#fe9a00' });
    expect(topStatus('failed', false, false)).toMatchObject({ label: 'Failed', icon: 'circle-alert' });
    expect(topStatus('ready', true, true)).toMatchObject({ label: 'Woke', icon: 'alarm-clock' });
    expect(topStatus('ready', false, true)).toMatchObject({ label: 'Done', icon: 'circle-check', color: '#00bc7d' });
    expect(topStatus('ready', false, false)).toBeNull();
  });

  test('shelves: snooze wins until its wake, then settlement beats a stale pin; Working only with the beta', () => {
    const caps = capabilities({ environment: { capabilities: ALL_CAPS } });
    expect(sectionOf(shell('a', { snoozedUntil: iso(60_000), settledOverride: 'settled', pinnedAt: iso(-1) }), caps, NOW, false)).toBe('snoozed');
    expect(sectionOf(shell('a', { snoozedUntil: iso(-60_000), settledOverride: 'settled', pinnedAt: iso(-1) }), caps, NOW, false)).toBe('settled');
    expect(sectionOf(shell('a', { pinnedAt: iso(-1) }), caps, NOW, false)).toBe('pinned');
    const running = shell('a', { latestRunId: 'r', status: 'running' });
    expect(sectionOf(running, caps, NOW, false)).toBe('active');
    expect(sectionOf(running, caps, NOW, true)).toBe('working');
    expect(sectionOf(shell('a', { settledOverride: 'settled' }), capabilities({}), NOW, false)).toBe('active');
    expect(sidebarVisible(shell('a', { lineage: { relationshipToParent: 'subagent' } }))).toBe(false);
    expect(sidebarVisible(shell('a', { archivedAt: iso(-1) }))).toBe(false);
  });

  test('a snoozed thread raises its hand on approval or a newer completion, and wakes as Woke until visited', () => {
    const snoozed = shell('a', { snoozedUntil: iso(3_600_000), snoozedAt: iso(-600_000) });
    expect(effectiveSnoozed(snoozed, NOW)).toBe(true);
    expect(effectiveSnoozed({ ...snoozed, pendingRuntimeRequest: { kind: 'command_approval' } }, NOW)).toBe(false);
    const completed = { ...snoozed, latestRunId: 'r', status: 'completed', latestRunCompletedAt: iso(-60_000) };
    expect(wokeAt(completed, NOW)).toBe(iso(-60_000));
    const timer = shell('a', { snoozedUntil: iso(-120_000), snoozedAt: iso(-600_000) });
    expect(wokeAt(timer, NOW)).toBe(iso(-120_000));
    expect(isWoke(timer, wokeAt(timer, NOW), iso(-300_000))).toBe(true);
    expect(isWoke(timer, wokeAt(timer, NOW), iso(-1_000))).toBe(false);
  });

  test('unread counts a completion newer than the visit; never-visited reads as read; recede rules', () => {
    const done = shell('a', { latestRunId: 'r', status: 'completed', latestRunCompletedAt: iso(-1_000) });
    expect(unseenCompletion(done, iso(-60_000))).toBe(true);
    expect(unseenCompletion(done, undefined)).toBe(false);
    expect(unseenCompletion(done, iso(0))).toBe(false);
    expect(recedes('ready', false, false, false, false)).toBe(true);
    expect(recedes('ready', true, false, false, false)).toBe(false);
    expect(recedes('working', true, false, false, false)).toBe(true);
    expect(recedes('input', false, false, false, false)).toBe(false);
    expect(recedes('ready', false, false, true, false)).toBe(false);
  });

  test('order: keyless active threads lead newest-anchor first, keyed follow by key; activity never moves a row', () => {
    const a = shell('a', { createdAt: iso(-3000), latestUserMessageAt: iso(-1) });
    const b = shell('b', { createdAt: iso(-2000) });
    const c = shell('c', { createdAt: iso(-1000), activeOrderKey: 'm' });
    const d = shell('d', { createdAt: iso(-9000), unsettledAt: iso(-10) });
    expect(sortActive([a, b, c, d]).map(thread => thread.id)).toEqual(['d', 'b', 'a', 'c']);
    const p1 = shell('p1', { pinOrderKey: 'n', createdAt: iso(-1) }), p2 = shell('p2', { pinOrderKey: 'g' }), p3 = shell('p3', { createdAt: iso(-1) });
    expect(sortPinned([p1, p2, p3]).map(thread => thread.id)).toEqual(['p2', 'p1', 'p3']);
  });

  test('labels: compact ages, wake countdowns rounding up, working durations', () => {
    expect(ageLabel(iso(-30_000), NOW)).toBe('now');
    expect(ageLabel(iso(-36_000_000), NOW)).toBe('10h');
    expect(snoozeWakeLabel(iso(61_000), NOW)).toBe('2m');
    expect(snoozeWakeLabel(iso(3_700_000), NOW)).toBe('2h');
    expect(snoozeWakeLabel(iso(-1), NOW)).toBe('now');
    expect(workingDuration(65_000)).toBe('1m');
    expect(workingDuration(3_725_000)).toBe('1h 2m');
  });

  test('fractional order keys sort between neighbours and materialize keyless sections', () => {
    const key = orderKeyBetween('g', 'n')!;
    expect(key > 'g' && key < 'n').toBe(true);
    expect(orderKeyBetween(null, null)).toBe('n');
    expect(planReorder(['a', 'b', 'c'], new Map([['a', 'g'], ['b', 'n'], ['c', 't']]), 'c')).toHaveLength(1);
    const plan = planReorder(['a', 'b'], new Map([['a', null], ['b', null]]), 'b');
    expect(plan.map(entry => entry.id)).toEqual(['a', 'b']);
    expect(plan[0]!.orderKey < plan[1]!.orderKey).toBe(true);
  });

  test('custom snooze resolves local dates and positive durations, refusing the past', () => {
    expect(resolveCustomSnooze({ mode: 'duration', date: '', time: '', amount: '2', unit: 'hours', }, NOW)).toEqual({ until: iso(7_200_000), error: '' });
    expect(resolveCustomSnooze({ mode: 'duration', date: '', time: '', amount: '0', unit: 'hours' }, NOW).error).toBe('Enter a positive duration.');
    expect(resolveCustomSnooze({ mode: 'date', date: '2020-01-01', time: '09:00', amount: '', unit: '' }, NOW).error).toBe('Choose a valid date and time in the future.');
    expect(resolveCustomSnooze({ mode: 'date', date: '2030-02-30', time: '09:00', amount: '', unit: '' }, NOW).until).toBeNull();
  });
});

describe('thread action menus (threadActionMenu.logic.ts, Sidebar.logic.ts bulk items, ElectronMenu.ts)', () => {
  const caps = capabilities({ environment: { capabilities: ALL_CAPS } });
  const presets = [{ id: 'hour', label: 'In 1 hour', wakeLabel: '13:00' }];
  test('the row menu lists the reference items in order, gated by capability', () => {
    const items = threadMenuItems({ branch: 'main', projectFilter: { label: 'Parity fixture', isActive: false }, isPinned: false, isSettled: false,
      autoSettleEnabled: true, isSnoozed: false, canSnoozeNow: true, isRegeneratingTitle: false, isRunning: false, caps, presets });
    expect(items.map(item => item.label)).toEqual(['New thread on main', 'Pin thread', 'Settle thread', 'Snooze', 'Rename thread', 'Regenerate title',
      'Mark unread', 'Filter by Parity fixture', 'Auto-settle behavior', 'Copy', 'Project settings', 'Archive thread', 'Delete']);
    expect(items.find(item => item.id === 'snooze')!.children!.map(child => child.label)).toEqual(['In 1 hour (13:00)', 'Custom…']);
    expect(items.find(item => item.id === 'copy')!.children!.map(child => child.label)).toEqual(['Path', 'Branch', 'Thread ID']);
    const minimal = threadMenuItems({ branch: '', projectFilter: { label: 'P', isActive: true }, isPinned: true, isSettled: true, autoSettleEnabled: false,
      isSnoozed: true, canSnoozeNow: false, isRegeneratingTitle: true, isRunning: true, caps: capabilities({}), presets });
    expect(minimal.map(item => item.label)).toEqual(['Rename thread', 'Mark unread', 'Show all projects', 'Copy', 'Project settings', 'Archive thread', 'Delete']);
    expect(minimal.find(item => item.id === 'archive')!.disabled).toBe(true);
    const states = threadMenuItems({ branch: '', projectFilter: null, isPinned: true, isSettled: true, autoSettleEnabled: false, isSnoozed: true,
      canSnoozeNow: true, isRegeneratingTitle: true, isRunning: false, caps, presets });
    expect(states.map(item => item.label).slice(0, 4)).toEqual(['Unpin thread', 'Un-settle thread', 'Wake thread', 'Rename thread']);
    expect(states.find(item => item.id === 'regenerate-title')).toMatchObject({ label: 'Regenerating…', disabled: true });
    expect(states.find(item => item.id === 'auto-settle')!.children!.map(child => child.checked)).toEqual([false, true]);
  });

  test('the bulk menu counts only what each action touches', () => {
    expect(bulkMenuItems({ count: 3, pinnedCount: 1, canSnooze: true, regeneratable: 2, regenerationSupported: 3, presets }).map(item => item.label))
      .toEqual(['Unpin (1)', 'Settle (3)', 'Snooze (3)', 'Regenerate titles (2)', 'Mark unread (3)', 'Delete (3)']);
    expect(bulkMenuItems({ count: 2, pinnedCount: 0, canSnooze: false, regeneratable: 0, regenerationSupported: 2, presets }).map(item => item.label))
      .toEqual(['Settle (2)', 'Regenerating… (2)', 'Mark unread (2)', 'Delete (2)']);
  });

  test('the native template places explicit separators and one before the first destructive row', () => {
    const template = nativeTemplate([{ id: 'a', label: 'A' }, { id: 'd', label: 'Delete', destructive: true }]);
    expect(template.map(entry => entry.type)).toEqual(['item', 'separator', 'item']);
    const row = nativeTemplate([{ id: 'a', label: 'A' }, { id: 'b', label: 'B', separatorBefore: true }, { id: 'd', label: 'Delete', destructive: true }]);
    expect(row.map(entry => entry.type)).toEqual(['item', 'separator', 'item', 'item']);
    expect(row[3]).toMatchObject({ destructive: true, enabled: true });
  });
});

describe('sidebar projection (sidebar-view.ts)', () => {
  test('rows paint pinned, active, then shelves; collapsed shelves keep only the open thread; the tail pages', () => {
    const settled = Array.from({ length: 14 }, (_, index) => shell(`s${index}`, { settledOverride: 'settled', settledAt: iso(-index * 60_000) }));
    const threads = [shell('a'), shell('p', { pinnedAt: iso(-1) }), shell('z', { snoozedUntil: iso(7_200_000) }), ...settled];
    const { client } = fake(threads, { threadId: 's12' });
    let view = sidebarSnapshot(client, NOW, helpers);
    expect(view.threads.map(row => row.id)).toEqual(['p', 'a', 's12']);
    expect(view.sidebar).toMatchObject({ settledCount: 14, snoozedCount: 1, settledExpanded: false, showMore: 0 });
    sidebarPrefs(client).settledExpanded = true; sidebarPrefs(client).snoozedExpanded = true;
    view = sidebarSnapshot(client, NOW, helpers);
    expect(view.threads.map(row => row.id)).toEqual(['p', 'a', 'z', ...settled.slice(0, 10).map(thread => thread.id as string), 's12']);
    expect(view.sidebar.showMore).toBe(3);
    expect(view.threads.find(row => row.id === 'z')).toMatchObject({ card: false, wakeLabel: '2h', canWake: true });
    expect(view.threads.find(row => row.id === 'p')).toMatchObject({ card: true, pinned: true, canPin: true });
  });

  test('every project is listed until the filter scopes it; empty states name the scope', async () => {
    const { client } = fake([shell('a'), shell('b', { projectId: 'p2' })]);
    expect(sidebarSnapshot(client, NOW, helpers).threads.map(row => row.id)).toEqual(['a', 'b']);
    await sidebarLocal(client, native, 'scope', '', 'g2');
    const scoped = sidebarSnapshot(client, NOW, helpers);
    expect(scoped.threads.map(row => row.id)).toEqual(['b']);
    expect(scoped.sidebar).toMatchObject({ scopeKey: 'g2', scopeLabel: 'Other' });
    client.shell.threads = [];
    expect(sidebarSnapshot(client, NOW, helpers).sidebar.emptyText).toBe('No threads in Other yet');
    await sidebarLocal(client, native, 'scope', '', '');
    expect(sidebarSnapshot(client, NOW, helpers).sidebar.emptyText).toBe('No threads yet');
    client.shell.projects = [];
    expect(sidebarSnapshot(client, NOW, helpers).sidebar).toMatchObject({ emptyText: 'No projects yet', emptyAddProject: true });
  });

  test('search keys move the highlight with wrap-around, Escape clears; pending text while the server searches', async () => {
    const { client } = fake([shell('a', { title: 'alpha one' }), shell('b', { title: 'alpha two' }), shell('c', { title: 'beta' })]);
    (client as unknown as { query: string }).query = 'alpha';
    expect(sidebarSnapshot(client, NOW, helpers).sidebar).toMatchObject({ searching: true, searchSelectedId: 'a' });
    await sidebarLocal(client, native, 'search-key', '', 'ArrowUp');
    expect(sidebarSnapshot(client, NOW, helpers).sidebar.searchSelectedId).toBe('b');
    await sidebarLocal(client, native, 'search-key', '', 'ArrowDown');
    expect(sidebarSnapshot(client, NOW, helpers).sidebar.searchSelectedId).toBe('a');
    await sidebarLocal(client, native, 'search-key', '', 'Escape');
    expect(client.query).toBe('');
  });

  test('woke, done and the unsent-draft marker reach the row', () => {
    const { client } = fake([shell('w', { snoozedUntil: iso(-60_000), snoozedAt: iso(-600_000), lastVisitedAt: iso(-300_000) }),
      shell('d', { latestRunId: 'r', status: 'completed', latestRunCompletedAt: iso(-1_000), lastVisitedAt: iso(-60_000) })]);
    client.local.drafts['env:d'] = 'unsent';
    const rows = sidebarSnapshot(client, NOW, helpers).threads;
    expect(rows.find(row => row.id === 'w')).toMatchObject({ woke: true, status: 'Woke' });
    expect(rows.find(row => row.id === 'd')).toMatchObject({ status: 'Done', draft: true, titleColor: 'light-dark(#27272a, #f3f3f3)', titleWeight: 500 });
  });
});

describe('sidebar commands (sidebar-commands.ts)', () => {
  test('pin takes the top of the pinned run; unpin offers Undo, which re-pins at the old key', async () => {
    const { client, dispatched } = fake([shell('a'), shell('p', { pinnedAt: iso(-1), pinOrderKey: 'n' })]);
    await sidebarCommand(client, native, files, 'pin', 'a', '');
    expect(strip(dispatched)).toEqual([{ type: 'thread.pin', threadId: 'a', orderKey: 'h' }]);
    await sidebarCommand(client, native, files, 'unpin', 'p', '');
    expect(sidebarSnapshot(client, NOW, helpers).sidebar.undoText).toBe('Unpinned 1 thread,');
    await undoLatest(client, native);
    expect(strip(dispatched).slice(1)).toEqual([{ type: 'thread.unpin', threadId: 'p' }, { type: 'thread.pin', threadId: 'p', orderKey: 'n' }]);
  });

  test('settling the open thread moves forward to the next card; consecutive settles merge one Undo', async () => {
    const { client, dispatched, opened } = fake([shell('a', { createdAt: iso(-1) }), shell('b', { createdAt: iso(-2) }), shell('c', { createdAt: iso(-3) })], { threadId: 'a' });
    await sidebarCommand(client, native, files, 'settle', 'a', '');
    expect(opened).toEqual(['b']);
    await sidebarCommand(client, native, files, 'settle', 'c', '');
    expect(sidebarSnapshot(client, NOW, helpers).sidebar.undoText).toBe('Settled 2 threads,');
    await sidebarCommand(client, native, files, 'undo', '', '');
    expect(strip(dispatched).map(payload => `${payload.type}:${payload.threadId}`)).toEqual(['thread.settle:a', 'thread.settle:c', 'thread.unsettle:c', 'thread.unsettle:a']);
  });

  test('rename trims, warns on empty, skips unchanged; failures and successes surface as toasts', async () => {
    const { client, dispatched } = fake([shell('a', { title: 'Old' })]);
    const session = sidebarSession(client);
    await sidebarLocal(client, native, 'rename-start', 'a', '');
    expect(session.renameId).toBe('a');
    await sidebarCommand(client, native, files, 'rename', 'a', '  New title  ');
    expect(strip(dispatched)).toEqual([{ type: 'thread.metadata.update', threadId: 'a', title: 'New title' }]);
    await sidebarLocal(client, native, 'rename-start', 'a', '');
    await sidebarCommand(client, native, files, 'rename', 'a', '   ');
    expect(toasts(client).at(-1)).toMatchObject({ kind: 'warning', title: 'Thread title cannot be empty' });
    await sidebarLocal(client, native, 'rename-start', 'a', '');
    await sidebarLocal(client, native, 'rename-key', 'a', 'Escape');
    expect(session.renameId).toBe('');
    await sidebarCommand(client, native, files, 'rename', 'a', 'Ignored after cancel');
    expect(dispatched).toHaveLength(1);
  });

  test('the row menu runs the picked action: mark unread, auto-settle, regenerate, copy, archive, delete with confirmation', async () => {
    const { client, dispatched, calls } = fake([shell('a', { branch: 'feature/x' })], { picks: ['mark-unread', 'auto-settle:disabled', 'regenerate-title', 'copy-branch', 'archive', 'delete'] });
    for (let index = 0; index < 5; index++) await sidebarCommand(client, native, files, 'menu', 'a', 'row');
    expect(strip(dispatched)).toEqual([{ type: 'thread.mark-unread', threadId: 'a' }, { type: 'thread.auto-settle.set', threadId: 'a', enabled: false },
      { type: 'thread.metadata.update', threadId: 'a', regenerateTitle: true }, { type: 'thread.archive', threadId: 'a' }]);
    expect(calls.filter(call => call.op === 'copyText').map(call => call.text)).toEqual(['feature/x']);
    expect(toasts(client).some(toast => toast.title === 'Branch copied')).toBe(true);
    await sidebarCommand(client, native, files, 'menu', 'a', 'row');
    expect(sidebarSnapshot(client, NOW, helpers).sidebar).toMatchObject({ dialog: 'delete', dialogTitle: 'Delete thread "Thread a"?', dialogDescription: 'This permanently clears conversation history for this thread.' });
    await sidebarCommand(client, native, files, 'dialog-confirm', '', '');
    expect(strip(dispatched).at(-1)).toEqual({ type: 'thread.delete', threadId: 'a' });
  });

  test('archive confirms only when Archive confirmation is on; a running thread cannot archive', async () => {
    const { client, dispatched } = fake([shell('a'), shell('r', { latestRunId: 'x', status: 'running', activeRunId: 'x' })], { settings: { confirmThreadArchive: true }, picks: ['archive'] });
    await sidebarCommand(client, native, files, 'menu', 'a', 'row');
    expect(sidebarSnapshot(client, NOW, helpers).sidebar).toMatchObject({ dialog: 'archive', dialogTitle: 'Archive thread "Thread a"?' });
    await sidebarLocal(client, native, 'dialog-close', '', '');
    expect(dispatched).toHaveLength(0);
    await sidebarCommand(client, native, files, 'archive', 'r', '');
    expect(dispatched).toHaveLength(0);
  });

  test('⌘-click toggles and ⇧-click extends the selection in sidebar order; a plain click clears it', async () => {
    const threads = [shell('a', { createdAt: iso(-1) }), shell('b', { createdAt: iso(-2) }), shell('c', { createdAt: iso(-3) })];
    const command = fake(threads, { modifiers: { command: true } });
    expect(await sidebarSelecting(command.client, native, 'a', 'click')).toBe(true);
    expect(await sidebarSelecting(command.client, native, 'c', 'click')).toBe(true);
    expect(sidebarSession(command.client).selection).toEqual(['a', 'c']);
    const shift = fake(threads, { modifiers: { shift: true }, threadId: 'a' });
    expect(await sidebarSelecting(shift.client, native, 'c', 'click')).toBe(true);
    expect(sidebarSession(shift.client).selection).toEqual(['a', 'b', 'c']);
    const plain = fake(threads);
    sidebarSession(plain.client).selection = ['a'];
    expect(await sidebarSelecting(plain.client, native, 'b', 'click')).toBe(false);
    expect(sidebarSession(plain.client).selection).toEqual([]);
  });

  test('a selected row opens the bulk menu; Settle (N) parks the whole batch', async () => {
    const { client, dispatched } = fake([shell('a'), shell('b')], { picks: ['settle'] });
    sidebarSession(client).selection = ['a', 'b'];
    await sidebarCommand(client, native, files, 'menu', 'a', 'row');
    expect(strip(dispatched).map(payload => `${payload.type}:${payload.threadId}`)).toEqual(['thread.settle:a', 'thread.settle:b']);
    expect(sidebarSession(client).selection).toEqual([]);
  });

  test('opening a thread visits it once per watermark; servers without tracking keep a local visit', () => {
    const tracked = fake([shell('a', { updatedAt: iso(-10), lastVisitedAt: iso(-60_000) })], { threadId: 'a' });
    visitOpenThread(tracked.client, native); visitOpenThread(tracked.client, native);
    return new Promise<void>(resolve => setTimeout(() => {
      expect(strip(tracked.dispatched)).toEqual([{ type: 'thread.visit', threadId: 'a', visitedAt: iso(-10) }]);
      const local = fake([shell('b', { updatedAt: iso(-5) })], { threadId: 'b', caps: { threadVisitedTracking: false } });
      visitOpenThread(local.client, native);
      expect(sidebarPrefs(local.client).visited.b).toBe(iso(-5));
      resolve();
    }, 5));
  });
});

describe('sidebar navigation, drafts and the hover card', () => {
  test('New thread: one project creates at once; several open the picker unless ⇧ is held', async () => {
    const several = fake([shell('a')]);
    expect(await sidebarCommand(several.client, native, files, 'new-thread-click', '', '')).toBe('sidebar:palette-new-thread');
    const shifted = fake([shell('a')], { modifiers: { shift: true } });
    expect(await sidebarCommand(shifted.client, native, files, 'new-thread-click', '', '')).toBe('sidebar:new-thread');
    expect(shifted.drafts).toEqual(['p1']);
    const single = fake([shell('a')]);
    (single.client as unknown as { projectGroups: () => unknown[] }).projectGroups = () => [{ key: 'g1', name: 'Parity fixture', members: [{ id: 'p1' }] }];
    expect(await sidebarCommand(single.client, native, files, 'new-thread-click', '', '')).toBe('sidebar:new-thread');
  });

  test('the brand opens a draft in the most recently active project; a project gear opens its settings', async () => {
    const { client, drafts } = fake([shell('a', { projectId: 'p2', latestUserMessageAt: iso(-1) }), shell('b', { latestUserMessageAt: iso(-60_000) })]);
    expect(await sidebarCommand(client, native, files, 'home', '', '')).toBe('sidebar:new-thread');
    expect(drafts).toEqual(['p2']);
    expect(await sidebarCommand(client, native, files, 'project-settings-group', '', 'g1')).toBe('sidebar:navigate');
    expect(sidebarSnapshot(client, NOW, helpers).sidebar).toMatchObject({ navigateKind: 'project-settings', navigateProject: 'p1', scopeOpen: false });
  });

  test('unsent new-thread drafts become cards above the list, except the draft on screen; X discards text and images', async () => {
    const { client, drafts } = fake([shell('a')], { threadId: 'a' });
    client.local.drafts['env:new:p2'] = 'Sketch the parser\nsecond line';
    (client.local as unknown as { snapshotDrafts: Record<string, Obj[]>; snapshotReleases: string[] }).snapshotDrafts = { 'env:new:p1': [{ id: 'img-1' }] };
    (client.local as unknown as { snapshotReleases: string[] }).snapshotReleases = [];
    expect(sidebarSnapshot(client, NOW, helpers).sidebar.drafts.map(draft => [draft.id, draft.preview])).toEqual([['p1', '1 attachment'], ['p2', 'Sketch the parser']]);
    expect(await sidebarCommand(client, native, files, 'open-draft', 'p2', '')).toBe('sidebar:new-thread');
    expect(drafts).toEqual(['p2']);
    (client as unknown as { projectId: string }).projectId = 'p2';
    expect(sidebarSnapshot(client, NOW, helpers).sidebar.drafts.map(draft => draft.id)).toEqual(['p1']);
    await sidebarCommand(client, native, files, 'discard-project-draft', 'p1', '');
    expect(sidebarSnapshot(client, NOW, helpers).sidebar.drafts).toEqual([]);
    // r11-upstream (95edeb753b): the discard waits behind the undo notice; its uploads go when Undo can no longer bring them back.
    expect(sidebarSnapshot(client, NOW, helpers).sidebar.undoText).toBe('Discarded 1 draft,');
    expect((client.local as unknown as { snapshotReleases: string[] }).snapshotReleases).toEqual([]);
  });

  test('the hover card names the machine, model · instance, hand-offs and the last error', () => {
    const { client } = fake([shell('a', { providerInstanceHistory: ['claude', 'codex'], lastError: 'quota', lastErrorClass: 'usage_limit' })]);
    Object.assign(client.config, { environment: { label: 'Studio Mac', platform: { machine: 'laptop' }, capabilities: ALL_CAPS },
      providers: [{ instanceId: 'codex', driver: 'codex', displayName: 'Codex', models: [{ slug: 'm', name: 'GPT-6-Astra' }] }, { instanceId: 'claude', driver: 'claudeAgent', displayName: 'Claude' }] });
    const row = sidebarSnapshot(client, NOW, { ...helpers, providerBadge: () => ({ providerBadge: 'CO', providerBadgeColor: '' }) }).threads[0]!;
    expect(row).toMatchObject({ hoverEnvironment: 'Studio Mac', hoverMachine: 'laptop', hoverModel: 'GPT-6-Astra · Codex',
      hoverHandoff: 'Handed off from Claude', hoverError: 'Usage limit reached', hoverErrorWarning: true });
  });

  test('custom snooze keeps the dialog open with its error, then snoozes every selected thread', async () => {
    const { client, dispatched } = fake([shell('a'), shell('b')], { picks: ['snooze:custom'] });
    sidebarSession(client).selection = ['a', 'b'];
    await sidebarCommand(client, native, files, 'menu', 'a', 'row');
    expect(sidebarSnapshot(client, NOW, helpers).sidebar).toMatchObject({ dialog: 'snooze', dialogMode: 'date', dialogUnit: 'hours', dialogAmount: '2' });
    await sidebarLocal(client, native, 'dialog-field', 'mode', 'duration');
    await sidebarLocal(client, native, 'dialog-field', 'amount', '-1');
    await sidebarCommand(client, native, files, 'dialog-confirm', '', '');
    expect(sidebarSnapshot(client, NOW, helpers).sidebar).toMatchObject({ dialog: 'snooze', dialogError: 'Enter a positive duration.' });
    await sidebarLocal(client, native, 'dialog-field', 'amount', '3');
    await sidebarLocal(client, native, 'dialog-field', 'unit', 'days');
    await sidebarCommand(client, native, files, 'dialog-confirm', '', '');
    expect(strip(dispatched).map(payload => `${payload.type}:${payload.threadId}`)).toEqual(['thread.snooze:a', 'thread.snooze:b']);
    expect(sidebarSnapshot(client, NOW, helpers).sidebar.dialog).toBe('');
  });
});


describe('terminal activity in thread lists (B10)', () => {
  test('only subprocesses count, updates clear the indicator across sidebar, legacy and palette', () => {
    const { client } = fake([shell('a'), shell('b')], { settings: { legacySidebarEnabled: true } });
    const summary = (threadId: string, terminalId: string, busy: boolean) => ({ threadId, terminalId, cwd: '/fixture', worktreePath: null,
      status: 'running', pid: 1, exitCode: null, exitSignal: null, hasRunningSubprocess: busy, label: 'sh', updatedAt: iso(0) });
    const update = (terminals: Obj[]) => terminalMetadataEvent(client, { subscriptionId: '1-1', value: { type: 'snapshot', terminals } });
    update([summary('a', 'term-1', true), summary('a', 'term-2', true), summary('a', 'term-3', false), summary('b', 'term-1', false)]);
    expect(sidebarSnapshot(client, NOW, helpers).threads.find(row => row.id === 'a')).toMatchObject({ terminalCount: 2, terminalLabel: '2 terminal processes running' });
    expect(sidebarSnapshot(client, NOW, helpers).threads.find(row => row.id === 'b')).toMatchObject({ terminalCount: 0, terminalLabel: '' });
    expect(legacySidebarSnapshot(client, NOW, helpers).projects.flatMap(project => project.threads).find(row => row.id === 'a')?.terminalCount).toBe(2);
    expect(threadItems(client, NOW, new Map(), '').find(item => item.row.key === 'a')?.row.terminalCount).toBe(2);
    expect(terminalProcessCount(client, 'fleet:other:a')).toBe(0);
    update([summary('a', 'term-1', true)]);
    expect(sidebarSnapshot(client, NOW, helpers).threads.find(row => row.id === 'a')?.terminalLabel).toBe('1 terminal process running');
    update([]);
    expect(sidebarSnapshot(client, NOW, helpers).threads.every(row => row.terminalCount === 0)).toBe(true);
    expect(legacySidebarSnapshot(client, NOW, helpers).projects.flatMap(project => project.threads).every(row => row.terminalCount === 0)).toBe(true);
    expect(threadItems(client, NOW, new Map(), '').every(item => item.row.terminalCount === 0)).toBe(true);
  });
  test('Command-hold jump hints disappear while the terminal owns focus and return on blur', () => {
    const { client } = fake([shell('a')], { threadId: 'a', settings: { legacySidebarEnabled: true } });
    client.presentation.sidebarJumpHints = true;
    expect(sidebarSnapshot(client, NOW, helpers).sidebar.jumpHints).toBe(true);
    expect(legacySidebarSnapshot(client, NOW, helpers).jumpHints).toBe(true);
    recordTerminalFocus(client, { environmentId: 'env', threadId: 'a', terminalId: 'term-1', focused: true });
    expect(sidebarSnapshot(client, NOW, helpers).sidebar.jumpHints).toBe(false);
    expect(legacySidebarSnapshot(client, NOW, helpers).jumpHints).toBe(false);
    recordTerminalFocus(client, { environmentId: 'env', threadId: 'a', terminalId: 'term-1', focused: false });
    expect(sidebarSnapshot(client, NOW, helpers).sidebar.jumpHints).toBe(true);
    expect(legacySidebarSnapshot(client, NOW, helpers).jumpHints).toBe(true);
  });
});

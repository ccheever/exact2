import { describe, expect, test } from 'bun:test';
import { pushToast, toasts } from './toast';
import { applyDismissals, advanceToasts, toastViews, commandShortcut, surfaces, titleMenu, withOffsets, TOAST_LIMIT } from './shell';
import { threadTransitions, transitionToast, threadNotifications, nativeNotifyStatus, reportWindowFacts, notifyEnvironments } from './shell-notify';
import { EnvironmentFleet, type FleetEntry } from './settings-b-fleet';
import { threadOps } from './client-ops-threads';
import { shellCommand, shellLocal, shellFailure, shellSuccess, resolveRenameCommit, settingsFailure } from './shell-commands';
import type { T3Client } from './client';
import type { Obj } from './domain';
import type { Files, Native } from './protocol';

const prefs = (extra: Obj = {}) => ({ notificationMode: 'off', inAppNotificationsEnabled: false, confirmThreadArchive: false, confirmThreadDelete: true, confirmThreadUnpin: false, ...extra });
function fakeClient(extra: Obj = {}): T3Client {
  return { threadId: '', projectId: '', ready: true, environmentId: 'env', connection: 'connected', shell: { threads: [], projects: [] },
    config: { environment: { capabilities: {} } }, local: { clientSettings: prefs(), deviceSettings: { timestampFormat: '24-hour' } }, ...extra } as unknown as T3Client;
}

describe('toast queue', () => {
  test('newest is in front, timers run only while visible and unhovered', () => {
    const client = fakeClient();
    pushToast(client, { kind: 'success', title: 'one' });
    pushToast(client, { kind: 'error', title: 'two', description: 'boom' });
    const views = toastViews(toasts(client));
    expect(views.map(view => [view.title, view.index])).toEqual([['two', 0], ['one', 1]]);
    expect(views[0]).toMatchObject({ icon: 'circle-alert', copyText: 'boom', stacked: false });
    expect(views[1]).toMatchObject({ icon: 'circle-check', iconColor: '#00bc7d', copyText: '' });
    advanceToasts(client, 1000, false);
    advanceToasts(client, 4000, true);
    expect(toasts(client)).toHaveLength(2);
    advanceToasts(client, 4900, false);
    advanceToasts(client, 5800, false);
    advanceToasts(client, 6700, false);
    advanceToasts(client, 7600, false);
    expect(toasts(client)).toHaveLength(2);
    for (let at = 8500; at <= 12000; at += 500) advanceToasts(client, at, false);
    expect(toasts(client)).toHaveLength(0);
  });

  test('a stale clock never expires a fresh toast at once, and loading toasts wait', () => {
    const client = fakeClient();
    pushToast(client, { kind: 'info', title: 'fresh' });
    pushToast(client, { kind: 'loading', title: 'spinner' });
    advanceToasts(client, 1000, false);
    advanceToasts(client, 600_000, false);
    expect(toasts(client).map(toast => toast.title)).toEqual(['fresh', 'spinner']);
    for (let at = 600_500; at < 620_000; at += 500) advanceToasts(client, at, false);
    expect(toasts(client).map(toast => toast.title)).toEqual(['spinner']);
  });

  test('only the front three count down', () => {
    const client = fakeClient();
    for (const title of ['a', 'b', 'c', 'd']) pushToast(client, { kind: 'info', title, timeoutMs: 1000 });
    advanceToasts(client, 1000, false);
    for (let at = 1500; at <= 2500; at += 500) advanceToasts(client, at, false);
    expect(toasts(client).map(toast => toast.title)).toEqual(['a']);
    expect(TOAST_LIMIT).toBe(3);
  });

  test('dismissals: the orb runs onClose once, an action only closes', () => {
    const client = fakeClient();
    let closed = 0;
    const first = pushToast(client, { kind: 'warning', title: 'update', onClose: () => closed++ });
    const second = pushToast(client, { kind: 'warning', title: 'other', onClose: () => closed++ });
    applyDismissals(client, ` x${first} a${second}`);
    applyDismissals(client, ` x${first} a${second}`);
    expect(closed).toBe(1);
    expect(toasts(client)).toHaveLength(0);
  });

  test('stacked layout needs an action; leading glyphs and providers replace the kind icon', () => {
    const client = fakeClient();
    pushToast(client, { kind: 'warning', title: 'stacked', stacked: true, action: { label: 'Settings', op: 'ui:settings', id: 'providers' }, actionVariant: 'outline', leading: 'provider:codex' });
    pushToast(client, { kind: 'warning', title: 'no action', stacked: true, leading: 'shield-question:warning-foreground' });
    const [plain, stacked] = toastViews(toasts(client));
    expect(stacked).toMatchObject({ stacked: true, provider: 'codex', icon: '', actionLabel: 'Settings', actionOp: 'ui:settings', actionId: 'providers', actionOutline: true });
    expect(plain).toMatchObject({ stacked: false, icon: 'shield-question', iconColor: 'light-dark(#bb4d00, #ffb900)' });
  });

  test('a keyed toast replaces its predecessor', () => {
    const client = fakeClient();
    pushToast(client, { kind: 'warning', title: 'v1', key: 'provider-update' });
    pushToast(client, { kind: 'warning', title: 'v2', key: 'provider-update' });
    expect(toasts(client).map(toast => toast.title)).toEqual(['v2']);
  });
});

describe('header and panels', () => {
  test('shortcut labels follow the last binding for the command', () => {
    const config = { keybindings: [{ command: 'rightPanel.toggle', shortcut: { key: 'b', modKey: true, altKey: true } }, { command: 'terminal.toggle', shortcut: { key: 'j', modKey: true } }] };
    expect(commandShortcut(config, 'rightPanel.toggle')).toBe('⌥⌘B');
    expect(commandShortcut(config, 'threadPanel.toggle')).toBe('');
  });

  test('the launcher lists every surface; diff needs a connected thread', () => {
    const client = fakeClient({ threadId: 't1', projectId: 'p1', shell: { threads: [{ id: 't1', projectId: 'p1' }], projects: [{ id: 'p1', workspaceRoot: '/p' }] } });
    const rows = surfaces(client);
    expect(rows.map(row => `${row.label} ${row.shortcut}`)).toEqual(['Browser B', 'Terminal T', 'Files F', 'Diff D', 'Pull request P', 'Linked pull requests L', 'Device M']);
    // realinput-1010-fixes RI-1: the launcher hears a row's letter in either case (surfaceShortcutActionForKey).
    expect(rows.map(row => row.letter).join('')).toBe('btfdplm');
    expect(rows.find(row => row.id === 'diff')).toMatchObject({ available: true, reason: '' });
    // browser-surface: the Browser row needs the desktop module (its WKWebView); without one it says why.
    expect(rows.find(row => row.id === 'browser')).toMatchObject({ available: false, reason: 'Only available in the desktop app.' });
    expect(surfaces(Object.assign(client, { available: true })).find(row => row.id === 'browser')).toMatchObject({ available: true, reason: '' });
    expect(rows.find(row => row.id === 'pull-request')!.reason).toBe('No pull request on this branch yet.');
    expect(surfaces(fakeClient()).find(row => row.id === 'diff')!.available).toBe(false);
  });

  test('the title menu follows buildThreadActionMenuItems with capability gating', () => {
    const thread = { id: 't1', projectId: 'p1', title: 'Fixture', status: 'idle', branch: null, pinnedAt: null, settledAt: null, settledOverride: null, snoozedUntil: null, autoSettleDisabledAt: null };
    const caps = { threadPinning: true, threadSettlement: true, threadSnooze: true, threadTitleRegeneration: true, threadAutoSettleOptOut: true };
    const client = fakeClient({ threadId: 't1', projectId: 'p1', shell: { threads: [thread], projects: [] }, config: { environment: { capabilities: caps } } });
    const menu = titleMenu(client, Date.parse('2026-10-04T10:00:00'));
    const top = menu.filter(item => item.submenu === '');
    expect(top.map(item => item.label)).toEqual(['Pin thread', 'Settle thread', 'Snooze', 'Rename thread', 'Regenerate title', 'Mark unread', 'Auto-settle behavior', 'Copy', 'Project settings', 'Archive thread', 'Delete']);
    expect(top.filter(item => item.separated).map(item => item.id)).toEqual(['rename', 'copy', 'archive']);
    expect(top.map(item => item.offset)).toEqual([5, 33, 61, 98, 126, 154, 182, 219, 247, 284, 312]);
    expect(menu.filter(item => item.submenu === 'copy').map(item => item.label)).toEqual(['Path', 'Thread ID']);
    expect(menu.filter(item => item.submenu === 'snooze').at(-1)).toMatchObject({ label: 'Custom…', separated: true });
    expect(menu.find(item => item.id === 'auto-settle:enabled')).toMatchObject({ checked: true, op: 'shell:auto-settle', value: 'true' });
    const del = menu.find(item => item.id === 'delete')!;
    expect(del).toMatchObject({ destructive: true, op: 'ui:confirm', value: 'shell:delete', confirmTitle: 'Delete thread "Fixture"?', confirmBody: 'This permanently clears conversation history for this thread.' });
    expect(menu.find(item => item.id === 'archive')).toMatchObject({ op: 'shell:archive', disabled: false });
    const plain = titleMenu(fakeClient({ threadId: 't1', shell: { threads: [{ ...thread, status: 'running', branch: 'feature' }], projects: [] } }), 0);
    expect(plain.filter(item => item.submenu === '').map(item => item.id)).toEqual(['new-thread-on-branch', 'rename', 'mark-unread', 'copy', 'project-settings', 'archive', 'delete']);
    expect(plain.find(item => item.id === 'archive')!.disabled).toBe(true);
    expect(plain.filter(item => item.submenu === 'copy').map(item => item.label)).toEqual(['Path', 'Branch', 'Thread ID']);
    expect(titleMenu(fakeClient({ projectId: 'p1' }), 0).map(item => item.label)).toEqual(['Project settings']);
  });

  test('the title menu reads settle and snooze as useThreadActionMenu does', () => {
    const caps = { threadSettlement: true, threadSnooze: true };
    const menu = (fields: Obj, now = Date.parse('2026-10-04T10:00:00Z')) => titleMenu(fakeClient({ threadId: 't1', shell: { threads: [{ id: 't1', projectId: 'p1', title: 'T', status: 'idle', ...fields }], projects: [] },
      config: { environment: { capabilities: caps } } }), now).filter(item => item.submenu === '').map(item => item.id);
    // An auto-settled thread (settledAt, no override) still offers Settle; only an explicit settle offers Un-settle.
    expect(menu({ settledAt: '2026-10-01T00:00:00Z', settledOverride: null })).toContain('settle');
    expect(menu({ settledOverride: 'settled' })).toContain('unsettle');
    // A snooze whose time has passed is awake: Snooze, not Wake thread.
    expect(menu({ snoozedUntil: '2026-10-03T00:00:00Z' })).toContain('snooze');
    expect(menu({ snoozedUntil: '2026-10-05T00:00:00Z' })).toContain('unsnooze');
  });

  test('offsets skip submenu children', () => {
    expect(withOffsets([{ id: 'a', submenu: '', separated: false }, { id: 'b', submenu: 'a', separated: false, offset: 0 }, { id: 'c', submenu: '', separated: true }] as never).map(item => (item as { offset: number }).offset)).toEqual([5, 0, 42]);
  });
});

describe('thread notifications', () => {
  const thread = (id: string, extra: Obj = {}) => ({ id, title: `T ${id}`, status: 'running', latestRunId: 'r1', activeRunId: 'r1', activityRunStatus: 'running', pendingRuntimeRequest: null, updatedAt: '2026-10-04T10:00:00.000Z', archivedAt: null, lineage: {}, ...extra });
  test('completion and attention transitions; the first shell only records', () => {
    const client = fakeClient({ shell: { threads: [thread('a'), thread('b')], projects: [] } });
    const first = threadTransitions(client, null);
    expect(first.transitions).toEqual([]);
    client.shell.threads = [thread('a', { status: 'idle', activeRunId: null, activityRunStatus: null, latestRunCompletedAt: '2026-10-04T10:01:00.000Z' }),
      thread('b', { pendingRuntimeRequest: { kind: 'approval' } })];
    const second = threadTransitions(client, first.next);
    expect(second.transitions.map(entry => [entry.threadId, entry.kind, entry.title])).toEqual([['a', 'completion', 'Thread completed'], ['b', 'input', 'Approval needed']]);
    expect(threadTransitions(client, second.next).transitions).toEqual([]);
    client.shell.threads = [thread('a', { status: 'failed', activeRunId: null, activityRunStatus: null, latestRunId: 'r2' }), thread('s', { lineage: { relationshipToParent: 'subagent' } })];
    expect(threadTransitions(client, second.next).transitions.map(entry => entry.title)).toEqual(['Thread failed']);
    expect(transitionToast({ threadId: 'a', kind: 'input', status: 'failed', title: '' })).toEqual({ kind: 'error', leading: 'circle-alert:destructive-foreground' });
    expect(transitionToast({ threadId: 'a', kind: 'input', status: 'input', title: '' }).leading).toBe('message-circle-question:info-foreground');
  });

  test('focused: an in-app toast for another thread only; unfocused: a system notification; sound either way', async () => {
    const requests: Obj[] = [];
    const native = { available: true, watch() {}, later: async (request: unknown) => { requests.push(request as Obj); return { ok: true, generation: 0, value: {} }; } } as Native;
    const client = fakeClient({ threadId: 'open', shell: { threads: [thread('a'), thread('open')], projects: [] } });
    client.local.clientSettings = prefs({ notificationMode: 'notifications-and-sound', inAppNotificationsEnabled: true }) as never;
    const none = new EnvironmentFleet();
    await threadNotifications(client, native, { active: true, authorization: 'authorized', agent: false, opened: '', openedThread: '' }, none);
    client.shell.threads = [thread('a', { pendingRuntimeRequest: { kind: 'user_input' } }), thread('open', { pendingRuntimeRequest: { kind: 'user_input' } })];
    await threadNotifications(client, native, { active: true, authorization: 'authorized', agent: false, opened: '', openedThread: '' }, none);
    // "Open thread" names the thread with its environment (the reference's /$environmentId/$threadId).
    expect(toasts(client).map(toast => [toast.title, toast.description, toast.action?.label, toast.action?.id])).toEqual([['Input needed', 'T a', 'Open thread', 'fleet:env:a']]);
    expect(requests.filter(request => request.op === 'notifySound').map(request => request.kind)).toEqual(['input', 'input']);
    // The open thread gets neither a toast nor, while focused, a system notification.
    expect(requests.filter(request => request.op === 'notifyPost')).toEqual([]);
    client.shell.threads = [thread('a', { status: 'idle', activeRunId: null, activityRunStatus: null, latestRunCompletedAt: '2026-10-04T10:05:00.000Z' }), thread('open')];
    await threadNotifications(client, native, { active: false, authorization: 'authorized', agent: false, opened: '', openedThread: '' }, none);
    expect(requests.filter(request => request.op === 'notifyPost').map(request => [request.title, request.body, request.tag, request.threadId])).toEqual([['Thread completed', 'T a', 'env:a', 'fleet:env:a']]);
  });

  // PG-10 (desktop audit 2026-10-09): ThreadNotificationCoordinator mounts EnvironmentNotifications for every
  // environment, so a turn that ends in a background environment notifies while another one is focused.
  describe('every connected environment (ThreadNotificationCoordinator)', () => {
    const active = { active: true, authorization: 'authorized', agent: false, opened: '', openedThread: '' };
    const done = (id: string, at: string, extra: Obj = {}) => thread(id, { status: 'idle', activeRunId: null, activityRunStatus: null, latestRunCompletedAt: at, ...extra });
    function setup(clientExtra: Obj = {}) {
      const requests: Obj[] = [];
      const native = { available: true, watch() {}, later: async (request: unknown) => { requests.push(request as Obj); return { ok: true, generation: 0, value: {} }; } } as Native;
      const client = fakeClient({ environmentId: 'env-a', threadId: 'a1', shell: { threads: [thread('a1')], projects: [], sequence: 1 }, ...clientExtra });
      client.local.clientSettings = prefs({ notificationMode: 'notifications-and-sound', inAppNotificationsEnabled: true }) as never;
      const source = new EnvironmentFleet();
      const entry = { key: 'http://b\nenv-b', origin: 'http://b', environmentId: 'env-b', phase: 'connected', generation: 3, synchronized: 3,
        shell: { threads: [thread('b1', { title: 'Build B' }), thread('a1', { title: 'Same id in B' })], projects: [], sequence: 1 } } as unknown as FleetEntry;
      source.entries.set(entry.key, entry);
      return { requests, native, client, source, entry };
    }

    test('the focused environment and each background one are watched; a background shell is live once synchronized', () => {
      const { client, source, entry } = setup();
      expect(notifyEnvironments(client, source).map(environment => [environment.environmentId, environment.live, environment.focused])).toEqual([['env-a', true, true], ['env-b', true, false]]);
      entry.synchronized = 2;
      expect(notifyEnvironments(client, source)[1]!.live).toBe(false);
      // The fleet never lists the focus; if it briefly does, the focused client's shell is the one watched.
      entry.environmentId = 'env-a';
      expect(notifyEnvironments(client, source).map(environment => environment.environmentId)).toEqual(['env-a']);
    });

    test('a completion in a background environment toasts "Thread completed" with Open thread on that environment\'s thread', async () => {
      const { requests, native, client, source, entry } = setup();
      await threadNotifications(client, native, active, source);
      expect(toasts(client)).toEqual([]);
      // B's thread b1 completes, and B's own a1 (the id the focused thread has in A) asks for approval.
      entry.shell = { ...entry.shell, threads: [done('b1', '2026-10-09T10:01:00.000Z', { title: 'Build B' }), thread('a1', { title: 'Same id in B', pendingRuntimeRequest: { kind: 'approval' } })] };
      await threadNotifications(client, native, active, source);
      expect(toasts(client).map(toast => [toast.kind, toast.title, toast.description, toast.action?.label, toast.action?.op, toast.action?.id])).toEqual([
        ['success', 'Thread completed', 'Build B', 'Open thread', 'select-thread', 'fleet:env-b:b1'],
        // Only the open thread of the focused environment is quiet: B's a1 is another thread.
        ['warning', 'Approval needed', 'Same id in B', 'Open thread', 'select-thread', 'fleet:env-b:a1'],
      ]);
      expect(requests.filter(request => request.op === 'notifySound').map(request => request.kind)).toEqual(['completion', 'input']);
      expect(requests.filter(request => request.op === 'notifyPost')).toEqual([]);
      // Nothing changed since: nothing more.
      await threadNotifications(client, native, active, source);
      expect(toasts(client)).toHaveLength(2);
    });

    test('unfocused: the system notification is tagged with B and opens B\'s thread', async () => {
      const { requests, native, client, source, entry } = setup();
      const inactive = { ...active, active: false };
      await threadNotifications(client, native, inactive, source);
      entry.shell = { ...entry.shell, threads: [done('b1', '2026-10-09T10:01:00.000Z', { title: 'Build B' })] };
      await threadNotifications(client, native, inactive, source);
      expect(toasts(client)).toEqual([]);
      expect(requests.filter(request => request.op === 'notifyPost').map(request => [request.title, request.body, request.tag, request.threadId])).toEqual([['Thread completed', 'Build B', 'env-b:b1', 'fleet:env-b:b1']]);
    });

    test('a background shell that is not live forgets, so its first live shell after a reconnect only records', async () => {
      const { native, client, source, entry } = setup();
      await threadNotifications(client, native, active, source);
      entry.synchronized = -1; entry.generation = 4;
      entry.shell = { ...entry.shell, threads: [done('b1', '2026-10-09T10:01:00.000Z')] };
      await threadNotifications(client, native, active, source);
      entry.synchronized = 4;
      await threadNotifications(client, native, active, source);
      expect(toasts(client)).toEqual([]);
      entry.shell = { ...entry.shell, threads: [done('b1', '2026-10-09T10:02:00.000Z')] };
      await threadNotifications(client, native, active, source);
      expect(toasts(client).map(toast => toast.title)).toEqual(['Thread completed']);
    });

    test('an environment that leaves is forgotten; one the focus moved to keeps notifying its own threads', async () => {
      const { native, client, source, entry } = setup();
      await threadNotifications(client, native, active, source);
      source.entries.delete(entry.key);
      await threadNotifications(client, native, active, source);
      source.entries.set(entry.key, entry);
      entry.shell = { ...entry.shell, threads: [done('b1', '2026-10-09T10:01:00.000Z')] };
      await threadNotifications(client, native, active, source);
      expect(toasts(client)).toEqual([]);
      // A not-ready focused client does not hold back a background environment's notifications.
      (client as unknown as { ready: boolean }).ready = false;
      entry.shell = { ...entry.shell, threads: [done('b1', '2026-10-09T10:03:00.000Z')] };
      await threadNotifications(client, native, active, source);
      expect(toasts(client).map(toast => toast.action?.id)).toEqual(['fleet:env-b:b1']);
    });

    test('a call that runs while an earlier one awaits its notifications never rewinds an environment\'s memory', async () => {
      const { requests, native, client, source, entry } = setup();
      client.shell = { threads: [thread('a1'), thread('a2', { title: 'Build A' })], projects: [], sequence: 1 } as never;
      await threadNotifications(client, native, active, source);
      // A's a2 and B's b1 complete; the first call is held at A's sound, before it reaches B.
      let holding = true, release = () => {};
      const held = new Promise<void>(resolve => { release = resolve; });
      const reply = native.later.bind(native);
      native.later = async (request: unknown) => { if (holding && (request as Obj).op === 'notifySound') await held; return reply(request); };
      client.shell = { ...client.shell, threads: [thread('a1'), done('a2', '2026-10-09T10:01:00.000Z', { title: 'Build A' })] };
      entry.shell = { ...entry.shell, threads: [done('b1', '2026-10-09T10:01:00.000Z', { title: 'Build B' })] };
      const first = threadNotifications(client, native, active, source);
      // Meanwhile B's shell is fetched again (a newer object: b1's next turn completed) and a second call runs to the end.
      holding = false;
      entry.shell = { ...entry.shell, threads: [done('b1', '2026-10-09T10:02:00.000Z', { title: 'Build B' })] };
      await threadNotifications(client, native, active, source);
      release();
      await first;
      const settled = toasts(client).map(toast => [toast.title, toast.description, toast.action?.id]);
      expect(settled).toEqual([['Thread completed', 'Build B', 'fleet:env-b:b1'], ['Thread completed', 'Build A', 'fleet:env-a:a2'], ['Thread completed', 'Build B', 'fleet:env-b:b1']]);
      // Each completion notified once: a pass over the same shells adds nothing (no older shell's memory left behind).
      await threadNotifications(client, native, active, source);
      expect(toasts(client)).toHaveLength(settled.length);
      expect(requests.filter(request => request.op === 'notifySound')).toHaveLength(settled.length);
    });

    test('Open thread: a thread of the focused environment opens in place; another environment\'s is focused there', async () => {
      const opened: string[] = [];
      const self = fakeClient({ environmentId: 'env-a', openSelected: async (_native: Native, id: string) => { opened.push(id); } });
      const native = { available: true, watch() {}, later: async () => ({ ok: true, generation: 0, value: {} }) } as unknown as Native;
      const out = { message: '', id: '', value: '' };
      expect(await threadOps.call(self, 'select-thread', 'fleet:env-a:a2', '', 0, native, {} as Files, out as never)).toBe(true);
      expect(opened).toEqual(['a2']);
      // B is not in the (empty) fleet: the focus change is refused with the fleet's own message.
      await expect(threadOps.call(self, 'select-thread', 'fleet:env-b:b1', '', 0, native, {} as Files, out as never)).rejects.toThrow('That environment is no longer connected.');
    });
  });

  test('the window facts are the page\'s (exactPage, exact2 #219): focus for notifications, each change once to the activity reporter', async () => {
    const requests: Obj[] = [];
    const native = { available: true, watch() {}, later: async (request: unknown) => { requests.push(request as Obj);
      return { ok: true, generation: 0, value: { active: true, authorization: 'authorized', agent: false, opened: '1:a', openedThread: 'a' } }; } } as Native;
    const previous = { active: true, authorization: 'unknown', agent: false, opened: '', openedThread: '' };
    // A module's own focus reading is not the window's: the page's hasFocus is.
    expect(await nativeNotifyStatus(native, previous, false)).toEqual({ active: false, authorization: 'authorized', agent: false, opened: '1:a', openedThread: 'a' });
    const failing = { available: true, watch() {}, later: async () => { throw new Error('gone'); } } as unknown as Native;
    expect((await nativeNotifyStatus(failing, previous, false)).active).toBe(false);
    const owner = {};
    await reportWindowFacts(owner, native, true, true);
    await reportWindowFacts(owner, native, true, true);
    await reportWindowFacts(owner, native, true, false);
    await reportWindowFacts(owner, native, false, false);
    expect(requests.filter(request => request.op === 'activityFacts').map(request => [request.visible, request.focused])).toEqual([[true, true], [true, false], [false, false]]);
  });

  test('off and in-app off: nothing tracked or raised', async () => {
    const requests: Obj[] = [];
    const native = { available: true, watch() {}, later: async (request: unknown) => { requests.push(request as Obj); return { ok: true, generation: 0, value: {} }; } } as Native;
    const client = fakeClient({ shell: { threads: [thread('a')], projects: [] } });
    const none = new EnvironmentFleet();
    await threadNotifications(client, native, { active: true, authorization: 'unknown', agent: false, opened: '', openedThread: '' }, none);
    client.shell.threads = [thread('a', { pendingRuntimeRequest: { kind: 'user_input' } })];
    await threadNotifications(client, native, { active: true, authorization: 'unknown', agent: false, opened: '', openedThread: '' }, none);
    expect(toasts(client)).toHaveLength(0);
    expect(requests.filter(request => request.op !== 'notifyClear')).toEqual([]);
  });
});


describe('shell commands and routed failures', () => {
  function client(extra: Obj = {}) {
    const dispatched: Obj[] = [], requested: Obj[] = [];
    const value = fakeClient({
      shell: { threads: [{ id: 't1', title: 'Fixture', projectId: 'p1', worktreePath: null, branch: 'main', status: 'idle' }], projects: [{ id: 'p1', workspaceRoot: '/work/p1' }] },
      restAccess: () => ({ ids: async (count: number) => Array.from({ length: count }, (_, index) => `id-${index}`),
        request: async (method: string, payload: Obj) => { requested.push({ method, ...payload }); return {}; },
        dispatch: async (_storage: Files, payload: Obj, description: string) => { dispatched.push({ ...payload, description }); return {}; } }),
      ...extra,
    });
    return { value, dispatched, requested };
  }
  const storage = {} as Files;
  test('thread actions dispatch the V2 commands', async () => {
    const { value, dispatched } = client();
    for (const op of ['pin', 'unpin', 'mark-unread', 'regenerate-title', 'archive', 'delete']) await shellCommand(value, {} as Native, storage, op, 't1', '');
    await shellCommand(value, {} as Native, storage, 'auto-settle', 't1', 'false');
    await shellCommand(value, {} as Native, storage, 'rename', 't1', '  Renamed  ');
    await shellCommand(value, {} as Native, storage, 'rename', 't1', 'Fixture');
    expect(dispatched.map(entry => [entry.type, entry.regenerateTitle ?? entry.enabled ?? entry.title ?? ''])).toEqual([
      ['thread.pin', ''], ['thread.unpin', ''], ['thread.mark-unread', ''], ['thread.metadata.update', true], ['thread.archive', ''], ['thread.delete', ''],
      ['thread.auto-settle.set', false], ['thread.metadata.update', 'Renamed']]);
    await shellCommand(value, {} as Native, storage, 'rename', 't1', '   ');
    expect(toasts(value).map(toast => [toast.kind, toast.title])).toEqual([['warning', 'Thread title cannot be empty']]);
    await expect(shellCommand(value, {} as Native, storage, 'pin', 'gone', '')).rejects.toThrow('That thread is no longer available.');
  });

  test('archive refuses a running thread; the editor uses its RPC', async () => {
    const running = client({ shell: { threads: [{ id: 't1', title: 'Busy', status: 'running', activeRunId: 'r' }], projects: [] }, config: { providers: [{ driver: 'codex', instanceId: 'codex' }] } });
    await expect(shellCommand(running.value, {} as Native, storage, 'archive', 't1', '')).rejects.toThrow('Stop the running turn');
    await shellCommand(running.value, {} as Native, storage, 'open-editor', '/work/p1', 'cursor');
    expect(running.requested).toEqual([{ method: 'shell.openInEditor', cwd: '/work/p1', editor: 'cursor' }]);
  });

  test('copies toast with the copied value', async () => {
    const copied: string[] = [];
    const native = { available: true, watch() {}, later: async (request: unknown) => { copied.push(String((request as Obj).text)); return { ok: true, generation: 0, value: { copied: true } }; } } as Native;
    const { value } = client();
    await shellLocal(value, native, 'copy-path', 't1', '');
    await shellLocal(value, native, 'copy-branch', 't1', '');
    await shellLocal(value, native, 'copy-thread-id', 't1', '');
    expect(copied).toEqual(['/work/p1', 'main', 't1']);
    expect(toasts(value).map(toast => [toast.title, toast.description])).toEqual([['Path copied', '/work/p1'], ['Branch copied', 'main'], ['Thread ID copied', 't1']]);
    const empty = client({ shell: { threads: [{ id: 't1' }], projects: [] } });
    await shellLocal(empty.value, native, 'copy-path', 't1', '');
    expect(toasts(empty.value).map(toast => [toast.kind, toast.title])).toEqual([['error', 'Path unavailable']]);
  });

  test('failures route to the reference toast titles instead of the banner', () => {
    const { value } = client();
    expect(shellFailure(value, 'chat:settle', 'denied')).toBe(true);
    expect(shellFailure(value, 'shell:pin', 'denied')).toBe(true);
    expect(shellFailure(value, 'send', 'Choose or add a project first.')).toBe(true);
    expect(shellFailure(value, 'send', 'other')).toBe(false);
    expect(shellFailure(value, 'model', 'denied')).toBe(false);
    const offline = client({ connection: 'reconnecting' });
    expect(shellFailure(offline.value, 'send', 'Reconnect before making changes.')).toBe(true);
    expect(toasts(value).map(toast => [toast.kind, toast.title, toast.description])).toEqual([
      ['error', 'Failed to settle thread', 'denied'], ['error', 'Failed to pin thread', 'denied'], ['warning', 'Choose a project first', 'This draft no longer points to an available project.']]);
    expect(toasts(offline.value)[0]).toMatchObject({ kind: 'warning', title: 'Not connected: message not sent' });
    const copy = client({ threadId: 't1' });
    shellSuccess(copy.value, 'restlocal:copy-thread', '', 'Copied thread ID');
    expect(toasts(copy.value)[0]).toMatchObject({ kind: 'success', title: 'Thread ID copied', description: 't1' });
  });

  test('settings failures toast with the reference titles', () => {
    const { value } = client();
    expect(settingsFailure(value, 'rest:task', 'scope', 'action=save&id=t', 'Scheduled task is incomplete: Add a title, prompt, project, and model.')).toBe('Scheduled task is incomplete');
    expect(settingsFailure(value, 'rest:task', 'scope', 'action=save', 'Use an interval of at least one minute.')).toBe('Invalid interval');
    expect(settingsFailure(value, 'rest:task', 'scope', 'action=save', 'Enter an existing checkout path.')).toBe('Checkout path is required');
    expect(settingsFailure(value, 'rest:task', 'scope', 'action=save', 'Could not save scheduled task: Choose an available provider and model.')).toBe('Could not save scheduled task');
    expect(settingsFailure(value, 'rest:task', 'scope', 'action=toggle&id=t', 'denied')).toBe('Could not update scheduled task');
    expect(settingsFailure(value, 'rest:keybinding', 'scope', 'action=remove&previous=x', 'denied')).toBe('Unable to remove keybinding');
    expect(settingsFailure(value, 'rest:keybinding', 'scope', 'action=save&command=a', 'denied')).toBe('Unable to save keybinding');
    expect(settingsFailure(value, 'rest:keybinding-open', 'scope', '', 'denied')).toBe('Unable to open keybindings file');
    expect(settingsFailure(value, 'settings-core', 'textGenerationModelSelection:model|scope', 'x', 'denied')).toBe('Text generation model not saved');
    expect(settingsFailure(value, 'rest:storage', 'scope', '', 'denied')).toBe('');
    // The queue keeps the newest five (toast.ts).
    expect(toasts(value).map(toast => toast.title)).toEqual(['Could not update scheduled task', 'Unable to remove keybinding', 'Unable to save keybinding', 'Unable to open keybindings file', 'Text generation model not saved']);
    const fresh = client().value;
    settingsFailure(fresh, 'rest:task', 'scope', 'action=save', 'Use an interval of at least one minute.');
    expect(toasts(fresh).map(toast => [toast.title, toast.description])).toEqual([['Invalid interval', 'Enter an interval of at least one minute.']]);
    shellSuccess(value, 'copy-diagnostic', 'trace-1', 'Copied trace ID');
    expect(toasts(value).at(-1)).toMatchObject({ kind: 'success', title: 'Trace ID copied', description: 'trace-1' });
  });

  test('resolveRenameCommit', () => {
    expect(resolveRenameCommit(' a ', 'b')).toEqual({ action: 'commit', title: 'a' });
    expect(resolveRenameCommit('  ', 'b')).toEqual({ action: 'reject-empty' });
    expect(resolveRenameCommit('b ', 'b')).toEqual({ action: 'noop' });
  });
});

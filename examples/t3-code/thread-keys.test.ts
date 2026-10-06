// Task thread-commands-and-keys: "Delete the worktree too?" (useThreadActions deleteThread)
// and the key commands (ChatView.tsx window keydown, Sidebar.tsx traversal), T3 Code 1e2ecbd975.
import { describe, expect, test } from 'bun:test';
import type { T3Client } from './client';
import type { Obj } from './domain';
import type { Files, Native } from './protocol';
import { toasts } from './toast';
import { sidebarSnapshot } from './sidebar-view';
import { sidebarCommand } from './sidebar-commands';
import { sidebarSession } from './sidebar-state';
import { keyboardDispatch, keyboardDispatchSource, favoriteEditor } from './keyboard-dispatch';
import { setCloseThreadTerminals } from './worktree-cleanup';
import { adoptShellPrefs, shellPrefs } from './shell-prefs';
import { hostKeyRows } from './thread-keys';
import { EnvironmentFleet } from './settings-b-fleet';
import { rememberEditor, lastEditor } from './shell-details';

const NOW = Date.parse('2026-10-06T12:00:00.000Z');
const iso = (offset: number) => new Date(NOW + offset).toISOString();
const shell = (id: string, extra: Obj = {}): Obj => ({ id, projectId: 'p1', title: `Thread ${id}`, status: 'idle', latestRunId: null, activeProviderThreadId: null,
  pendingRuntimeRequest: null, createdAt: iso(-3_600_000), updatedAt: iso(-3_600_000), archivedAt: null, settledOverride: null, settledAt: null,
  modelSelection: { instanceId: 'codex', model: 'm' }, lineage: { relationshipToParent: null }, ...extra });

type Call = { method: string; payload: Obj };
function fake(threads: Obj[], options: { settings?: Obj; serverSettings?: Obj; threadId?: string; failRemove?: string; failRefresh?: string; scratch?: string } = {}) {
  const requests: Call[] = [], opened: string[] = [], drafts: string[] = [];
  let ids = 0;
  const client = {
    shell: { projects: [{ id: 'p1', title: 'Fixture', workspaceRoot: '/repo' }, { id: 'p2', title: 'Scratch', workspaceRoot: '/scratch' }], threads, sequence: 1 },
    config: { environment: { capabilities: {} }, providers: [], keybindings: [], ...(options.serverSettings ? { settings: options.serverSettings } : {}), ...(options.scratch ? { scratchWorkspaceRoot: options.scratch } : {}) },
    environmentId: 'env', threadId: options.threadId ?? '', projectId: 'p1', query: '', connection: 'connected', writable: true, ready: true, projection: {},
    presentation: {}, local: { drafts: {}, deviceSettings: { timestampFormat: '24-hour' }, clientSettings: { confirmThreadDelete: false, sidebarThreadSortOrder: 'created_at', ...(options.settings ?? {}) } },
    projectGroups() { return [{ key: 'g1', name: 'Fixture', members: [{ id: 'p1' }] }]; },
    restAccess: () => ({
      ids: async (count: number) => Array.from({ length: count }, () => `c${ids++}`),
      request: async (method: string, payload: Obj) => {
        requests.push({ method, payload });
        if (method === 'orchestration.dispatchCommand' && payload.type === 'thread.delete') client.shell.threads = client.shell.threads.filter(thread => thread.id !== payload.threadId);
        if (method === 'orchestration.getThreadProjection') return { providerSessions: [{ id: 's1' }] };
        if (method === 'vcs.removeWorktree' && options.failRemove) throw new Error(options.failRemove);
        if (method === 'vcs.refreshStatus' && options.failRefresh) throw new Error(options.failRefresh);
        return {};
      },
      call: async (request: Obj) => request.op === 'sidebarMenu' ? { id: 'delete' } : {},
    }),
    async openSelected(_native: Native, id: string) { opened.push(id); client.threadId = id; },
    async openDraft(_native: Native, projectId: string) { drafts.push(projectId); client.threadId = ''; },
  } as unknown as T3Client & { threadId: string; shell: { threads: Obj[] } };
  const trace = () => requests.map(({ method, payload }) => method === 'orchestration.dispatchCommand' ? str(payload.type) : method);
  return { client, requests, opened, drafts, trace };
}
const str = (value: unknown) => typeof value === 'string' ? value : '';
const native = { available: true, watch() {}, later: async () => ({}) } as unknown as Native;
const files = {} as Files;
const helpers = { projectIdentity: (name: string) => ({ projectMark: name.slice(0, 2).toUpperCase(), projectInk: 'ink', projectSurface: 'surface' }),
  providerBadge: () => ({ providerBadge: '', providerBadgeColor: '' }) };
const del = (client: T3Client, id: string) => sidebarCommand(client, native, files, 'menu', id, 'row');

describe('G5: Delete the worktree too?', () => {
  test('an orphaned worktree asks with the reference copy; Confirm deletes the thread, then removes the worktree and refreshes status', async () => {
    const { client, requests, trace } = fake([shell('a', { worktreePath: '/repo/.t3/worktrees/feature-a', latestRunId: 'r1', status: 'completed' }), shell('b')]);
    await del(client, 'a');
    expect(trace()).toEqual([]);
    expect(sidebarSnapshot(client, NOW, helpers).sidebar).toMatchObject({ dialog: 'delete-worktree', dialogTitle: 'Delete the worktree too?',
      dialogDescription: 'This thread is the only one linked to this worktree:\nfeature-a' });
    await sidebarCommand(client, native, files, 'dialog-confirm', '', '');
    // stopThreadSession (a runtime), thread.delete, vcs.removeWorktree (forced, project root), vcs.refreshStatus.
    expect(trace()).toEqual(['orchestration.getThreadProjection', 'provider-session.detach', 'thread.delete', 'vcs.removeWorktree', 'vcs.refreshStatus']);
    expect(requests[3]!.payload).toEqual({ cwd: '/repo', path: '/repo/.t3/worktrees/feature-a', force: true });
    expect(requests[4]!.payload).toEqual({ cwd: '/repo' });
    expect(sidebarSession(client).dialog.kind).toBe('');
  });
  test('Cancel keeps the worktree but still deletes the thread; the terminal hook runs before the delete', async () => {
    const closed: string[] = [];
    setCloseThreadTerminals(async (_client, _native, threadId) => { closed.push(threadId); });
    const { client, trace } = fake([shell('a', { worktreePath: '/repo/wt-a' })]);
    await del(client, 'a');
    await sidebarCommand(client, native, files, 'dialog-cancel', '', '');
    expect(trace()).toEqual(['thread.delete']);
    expect(closed).toEqual(['a']);
    setCloseThreadTerminals(async () => undefined);
  });
  test('a shared worktree does not ask; its last thread does; a bulk delete asks once, after the first deletion', async () => {
    const single = fake([shell('a', { worktreePath: '/repo/wt' }), shell('b', { worktreePath: '/repo/wt' })]);
    await del(single.client, 'a');
    expect(single.trace()).toEqual(['thread.delete']);
    await del(single.client, 'b');
    expect(sidebarSession(single.client).dialog.kind).toBe('delete-worktree');
    await sidebarCommand(single.client, native, files, 'dialog-confirm', '', '');
    expect(single.trace()).toEqual(['thread.delete', 'thread.delete', 'vcs.removeWorktree', 'vcs.refreshStatus']);

    const bulk = fake([shell('a', { worktreePath: '/repo/wt' }), shell('b', { worktreePath: '/repo/wt' }), shell('c')]);
    sidebarSession(bulk.client).selection = ['a', 'b'];
    await del(bulk.client, 'a');
    expect(bulk.trace()).toEqual(['thread.delete']);
    expect(sidebarSession(bulk.client).dialog).toMatchObject({ kind: 'delete-worktree', threadIds: ['b'], title: 'wt' });
    await sidebarCommand(bulk.client, native, files, 'dialog-confirm', '', '');
    expect(bulk.trace()).toEqual(['thread.delete', 'thread.delete', 'vcs.removeWorktree', 'vcs.refreshStatus']);
    expect(sidebarSession(bulk.client).selection).toEqual([]);
  });
  test('no prompt and no removal for a Scratch thread, with the cleanup rule on, or without a worktree', async () => {
    const scratch = fake([shell('s', { projectId: 'p2', worktreePath: '/scratch/x' })], { scratch: '/scratch' });
    await del(scratch.client, 's');
    expect(scratch.trace()).toEqual(['thread.delete']);
    const rule = fake([shell('a', { worktreePath: '/repo/wt' })], { serverSettings: { storageCleanup: { worktreeOnDelete: true } } });
    await del(rule.client, 'a');
    expect(rule.trace()).toEqual(['thread.delete']);
    const projectRule = fake([shell('a', { worktreePath: '/repo/wt' })], { serverSettings: { storageCleanup: { worktreeOnDelete: false }, projectSettingsOverrides: { p1: { worktreeCleanup: { mode: 'custom', rules: { worktreeOnDelete: true } } } } } });
    await del(projectRule.client, 'a');
    expect(projectRule.trace()).toEqual(['thread.delete']);
    const plain = fake([shell('a')]);
    await del(plain.client, 'a');
    expect(plain.trace()).toEqual(['thread.delete']);
  });
  test('a cleanup failure is its own stacked toast and the thread stays deleted', async () => {
    const removal = fake([shell('a', { worktreePath: '/repo/wt-a' })], { failRemove: 'worktree is locked' });
    await del(removal.client, 'a');
    await sidebarCommand(removal.client, native, files, 'dialog-confirm', '', '');
    expect(removal.trace()).toEqual(['thread.delete', 'vcs.removeWorktree']);
    expect(toasts(removal.client).map(toast => [toast.kind, toast.title, toast.description, toast.stacked])).toEqual([['error', 'Failed to delete worktree', 'Could not remove wt-a. worktree is locked', true]]);
    const refresh = fake([shell('a', { worktreePath: '/repo/wt-a' })], { failRefresh: 'status failed' });
    await del(refresh.client, 'a');
    await sidebarCommand(refresh.client, native, files, 'dialog-confirm', '', '');
    expect(toasts(refresh.client).map(toast => [toast.title, toast.description])).toEqual([['Worktree deleted, but Git status refresh failed', 'status failed']]);
  });
  test('the open thread falls back to the top remaining thread of its project in the thread sort', async () => {
    const { client, opened } = fake([shell('old', { createdAt: iso(-9_000_000) }), shell('a', { createdAt: iso(-5_000_000) }), shell('new', { createdAt: iso(-1_000_000) })], { threadId: 'a' });
    await del(client, 'a');
    expect(opened).toEqual(['new']);
  });
});

const binding = (command: string, key: string, mods: Obj = {}, whenAst?: Obj) => ({ command, shortcut: { key, ...mods }, ...(whenAst ? { whenAst } : {}) });
const keys = [binding('thread.previous', '[', { modKey: true, shiftKey: true }), binding('thread.next', ']', { modKey: true, shiftKey: true }),
  binding('thread.steerQueuedMessage', 'enter', { modKey: true, shiftKey: true }), binding('thread.editQueuedMessage', 'arrowup', { altKey: true }, { type: 'identifier', name: 'composerFocus' }),
  binding('composer.host', 'h', { modKey: true, shiftKey: true }), binding('composer.cycleHost', 'y', { modKey: true, altKey: true }), binding('rightPanel.toggle', 'b', { modKey: true, altKey: true }),
  binding('thread.copyReference', 'c', { modKey: true, shiftKey: true }), binding('commandPalette.toggle', 'k', { modKey: true }), binding('usage.open', 'u', { modKey: true }),
  binding('theme.select', 'a', { modKey: true, altKey: true }), binding('editor.openFavorite', 'o', { modKey: true }), binding('navigation.back', '[', { modKey: true }), binding('navigation.forward', ']', { modKey: true })];
const context = { composerFocus: false, editableFocus: false, turnRunning: false, modelPickerOpen: false, draftThreadRoute: false, modalOpen: false, settingsOpen: false, diffOpen: false };
function keyClient(extra: Obj = {}): T3Client {
  return { config: { keybindings: keys, providers: [], environment: { capabilities: {} } }, local: { deviceSettings: { appearanceMode: 'system' }, favoriteModels: [], drafts: {} }, providerId: '', modelId: '',
    shell: { projects: [{ id: 'p1', workspaceRoot: '/repo' }], threads: [shell('a'), shell('b'), shell('c')], sequence: 1 }, threadId: 'a', projectId: 'p1', environmentId: 'env', connection: 'connected',
    projection: {}, presentation: {}, ...extra } as unknown as T3Client;
}
const rows = (ids: string[], selected: string) => ids.map(id => ({ id, section: 'active', selected: id === selected }));
const find = (items: { command: string }[], command: string) => items.find(item => item.command === command) as Obj | undefined;

describe('A14: keys', () => {
  test('thread.previous / thread.next stop at the ends; no current thread starts at the last / first; an unlisted one goes nowhere', () => {
    const at = (selected: string, threadId: string) => keyboardDispatch(keyClient({ threadId }), rows(['a', 'b', 'c'], selected), '', '', context);
    expect(find(at('a', 'a'), 'thread.previous')).toBeUndefined();
    expect(find(at('a', 'a'), 'thread.next')!.target).toBe('b');
    expect(find(at('c', 'c'), 'thread.next')).toBeUndefined();
    expect(find(at('c', 'c'), 'thread.previous')!.target).toBe('b');
    expect(find(at('', ''), 'thread.previous')!.target).toBe('c');
    expect(find(at('', ''), 'thread.next')!.target).toBe('a');
    expect(find(at('', 'hidden'), 'thread.next')).toBeUndefined();
    expect(find(keyboardDispatch(keyClient(), rows(['a', 'b'], 'a'), '', '', { ...context, modelPickerOpen: true }), 'thread.next')).toBeUndefined();
  });
  test('⇧⌘↩ steers the first queued message only when the provider steers; ⌥↑ is a composer key for the last one, not while editing or on a draft', () => {
    const projection = { thread: { id: 'a', activeProviderThreadId: 'pt' }, messages: [{ id: 'm1', text: 'one' }, { id: 'm2', text: 'two' }],
      runs: [{ id: 'run', status: 'running', activeAttemptId: 'at', providerThreadId: 'pt', ordinal: 1 }, { id: 'q1', status: 'queued', userMessageId: 'm1', ordinal: 2 }, { id: 'q2', status: 'queued', userMessageId: 'm2', ordinal: 3 }],
      providerThreads: [{ id: 'pt', providerSessionId: 's' }], providerSessions: [{ id: 's', capabilities: { turns: { supportsActiveSteering: true } } }], providerTurns: [{ runAttemptId: 'at', status: 'running' }] };
    const client = keyClient({ projection });
    const items = keyboardDispatch(client, rows(['a'], 'a'), '', '', { ...context, composerFocus: true, editableFocus: true });
    expect(find(items, 'thread.steerQueuedMessage')).toMatchObject({ kind: 'command', target: 'cc:queued-steer', extra: 'q1', chord: 'Meta+Shift+Enter' });
    expect(find(items, 'thread.editQueuedMessage')).toMatchObject({ kind: 'composer-key', target: 'cclocal:queued-edit', extra: 'q2', chord: 'Alt+ArrowUp' });
    // composerFocus is the binding's `when`: without it the key is the text system's.
    expect(find(keyboardDispatch(client, rows(['a'], 'a'), '', '', context), 'thread.editQueuedMessage')).toBeUndefined();
    expect(find(keyboardDispatch(client, rows(['a'], 'a'), '', '', { ...context, composerFocus: true, draftThreadRoute: true }), 'thread.editQueuedMessage')).toBeUndefined();
    const quiet = keyClient({ projection: { ...projection, providerSessions: [{ id: 's', capabilities: { turns: {} } }] } });
    expect(find(keyboardDispatch(quiet, rows(['a'], 'a'), '', '', context), 'thread.steerQueuedMessage')).toBeUndefined();
  });
  test('⌥⌘B toggles a draft\'s right panel too; ⇧⌘C stays off a draft', () => {
    const items = keyboardDispatch(keyClient({ threadId: '' }), rows(['a'], ''), '', '', { ...context, draftThreadRoute: true });
    expect(find(items, 'rightPanel.toggle')).toMatchObject({ kind: 'diff' });
    expect(find(items, 'thread.copyReference')).toBeUndefined();
  });
  test('Settings keeps the palette provider\'s chords: ⌘K, ⌥⌘A, ⌘U', () => {
    const items = keyboardDispatch(keyClient(), rows(['a'], 'a'), '', '', { ...context, modalOpen: true, settingsOpen: true });
    expect(items.map(item => item.command)).toEqual(['settings.open', 'navigation.back', 'commandPalette.toggle', 'theme.select', 'usage.open']);
  });
  test('Back and Forward cover the pages over a thread (Usage)', () => {
    const client = keyClient();
    const source = (page: string) => keyboardDispatchSource(client, [rows(['a'], 'a'), '', '', false, false, false, false, false, false, false, 0, false, '', '', false, '', page]);
    source('');
    const onUsage = source('usage');
    expect(find(onUsage, 'navigation.back')).toMatchObject({ kind: 'thread', target: 'a' });
    const back = source('');
    expect(find(back, 'navigation.forward')).toMatchObject({ kind: 'palette-run', target: 'usage' });
  });
  test('⌘O opens the last-used editor while it is available; the choice persists in the preference file', () => {
    const config = { availableEditors: ['cursor', 'zed'] };
    expect(favoriteEditor(config)).toBe('cursor');
    expect(favoriteEditor(config, 'zed')).toBe('zed');
    expect(favoriteEditor(config, 'vscode')).toBe('cursor');
    const owner = { local: {} } as unknown as T3Client;
    rememberEditor(owner, 'zed');
    expect(lastEditor(owner)).toBe('zed');
    const next: Obj = {};
    adoptShellPrefs(next, JSON.parse(JSON.stringify({ shell: shellPrefs(owner) })));
    expect((next.shell as Obj).lastEditor).toBe('zed');
  });
  test('⇧⌘H opens the Run on control and Cycle Host steps to the next machine, wrapping, only on a draft with a choice', () => {
    const identity = { canonicalKey: 'git.example.invalid/acme/shared', rootPath: '/repos/shared' };
    const source = new EnvironmentFleet();
    source.entries.set('https://box.example.invalid\nb', { key: 'https://box.example.invalid\nb', origin: 'https://box.example.invalid', environmentId: 'b', phase: 'connected', message: '', traceId: '',
      generation: 1, synchronized: 1, lastEvent: 0, subscriptions: {}, config: { environment: { label: 'Build box', platform: { machine: 'cloud' } } },
      shell: { projects: [{ id: 'pb', workspaceRoot: '/repos/shared', repositoryIdentity: identity }], threads: [], sequence: 0 }, scopes: [], error: '', requested: true } as never);
    const client = { environmentId: 'a', origin: 'http://127.0.0.1:1', projectId: 'pa', threadId: '', draftKey: 'a:new:pa', config: { environment: { label: 'Local' } },
      shell: { projects: [{ id: 'pa', workspaceRoot: '/repos/shared', repositoryIdentity: identity }], threads: [] }, local: { groupingMode: 'repository', groupingOverrides: {} } } as unknown as T3Client;
    const added: string[][] = [];
    hostKeyRows((command, kind, target, _label, extra = '') => { added.push([command, kind, target, extra]); }, client, source);
    expect(added).toEqual([['composer.host', 'options', 'workspace', ''], ['composer.cycleHost', 'command-value', 'environment-run-on', 'b']]);
    added.length = 0;
    hostKeyRows((command, kind, target, _label, extra = '') => { added.push([command, kind, target, extra]); }, { ...client, threadId: 't1' } as unknown as T3Client, source);
    expect(added).toEqual([]);
    hostKeyRows((command, kind, target, _label, extra = '') => { added.push([command, kind, target, extra]); }, client, new EnvironmentFleet());
    expect(added).toEqual([]);
  });
});

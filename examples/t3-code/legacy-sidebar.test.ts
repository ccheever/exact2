// legacy-sidebar: ports of T3 Code 1e2ecbd975's threadSort.test.ts, the Sidebar.logic.test.ts
// cases for the functions the legacy sidebar uses, environmentGrouping.test.ts's grouping
// cases, and the legacy sidebar's own projection and gestures (MIT, see LICENSE-T3).
import { describe, expect, test } from 'bun:test';
import type { T3Client } from './client';
import type { Obj } from './domain';
import type { Files, Native } from './protocol';
import { toasts } from './toast';
import { EnvironmentFleet } from './settings-b-fleet';
import { sidebarPrefs, sidebarSession } from './sidebar-state';
import { sidebarCommand, sidebarLocal, sidebarSelecting } from './sidebar-commands';
import { keyboardDispatch } from './keyboard-dispatch';
import { projectScopes } from './sidebar-view';
import { buildMultiSelectThreadContextMenuItems, getLatestThreadForProject, isTrailingDoubleClick, legacyProjectMenuItems, legacyThreadMenuItems,
  orderItemsByPreferredIds, relativeTimeLabel, reorderProjects, resolveAdjacentThreadId, resolveProjectStatusIndicator, resolveThreadStatusPill,
  sortProjectsForSidebar, sortThreads, type StatusPill } from './legacy-sidebar-model';
import { legacyGroups, legacySession, legacySidebarSnapshot, legacyTraversal, legacyVisibleThreadIds } from './legacy-sidebar-view';

const at = (minute: number) => `2026-03-09T10:${String(minute).padStart(2, '0')}:00.000Z`;
const thread = (id: string, extra: Obj = {}): Obj => ({ id, projectId: 'p1', title: `Thread ${id}`, status: 'idle', latestRunId: null, activeProviderThreadId: null,
  pendingRuntimeRequest: null, createdAt: at(0), updatedAt: at(0), archivedAt: null, settledOverride: null, lineage: { relationshipToParent: null }, ...extra });

describe('sortThreads (threadSort.test.ts)', () => {
  test('sorts threads by the latest user message in recency mode', () => {
    const sorted = sortThreads([
      thread('thread-1', { updatedAt: at(10), messages: [{ role: 'user', createdAt: at(1) }] }),
      thread('thread-2', { createdAt: at(5), updatedAt: at(5), messages: [{ role: 'user', createdAt: at(6) }] }),
    ], 'updated_at');
    expect(sorted.map(entry => entry.id)).toEqual(['thread-2', 'thread-1']);
  });
  test('falls back to thread timestamps when there is no user message', () => {
    const sorted = sortThreads([
      thread('thread-1', { updatedAt: at(1), messages: [{ role: 'assistant', createdAt: at(2) }] }),
      thread('thread-2', { createdAt: at(5), updatedAt: at(5), messages: [] }),
    ], 'updated_at');
    expect(sorted.map(entry => entry.id)).toEqual(['thread-2', 'thread-1']);
  });
  test('can sort threads by createdAt when configured', () => {
    const sorted = sortThreads([thread('thread-1', { createdAt: at(5), updatedAt: at(5) }), thread('thread-2', { createdAt: at(0), updatedAt: at(10) })], 'created_at');
    expect(sorted.map(entry => entry.id)).toEqual(['thread-1', 'thread-2']);
  });
  test('uses updatedAt as a fallback for created_at sorting when createdAt is invalid', () => {
    const sorted = sortThreads([thread('thread-1', { createdAt: 'invalid-date', updatedAt: at(5) }), thread('thread-2', { createdAt: at(0), updatedAt: at(10) })], 'created_at');
    expect(sorted.map(entry => entry.id)).toEqual(['thread-1', 'thread-2']);
  });
  test('the shell stamp latestUserMessageAt wins over messages; ties order by id descending', () => {
    expect(sortThreads([thread('a', { latestUserMessageAt: at(9) }), thread('b', { latestUserMessageAt: at(3), messages: [{ role: 'user', createdAt: at(30) }] })], 'updated_at').map(entry => entry.id)).toEqual(['a', 'b']);
    expect(sortThreads([thread('a'), thread('c'), thread('b')], 'updated_at').map(entry => entry.id)).toEqual(['c', 'b', 'a']);
  });
  test('returns the latest active thread for a project', () => {
    const latest = getLatestThreadForProject([
      thread('thread-1', { createdAt: at(0), updatedAt: at(1) }),
      thread('thread-2', { createdAt: at(5), updatedAt: at(10), archivedAt: '2026-03-10T00:00:00.000Z' }),
      thread('thread-3', { createdAt: at(6), updatedAt: at(6) }),
    ], 'p1', 'updated_at');
    expect(latest?.id).toBe('thread-3');
  });
});

describe('Sidebar.logic.test.ts cases', () => {
  test('buildMultiSelectThreadContextMenuItems offers bulk archive with the count, disabled while a thread runs', () => {
    expect(buildMultiSelectThreadContextMenuItems({ count: 3, hasRunningThread: false })).toContainEqual({ id: 'archive', label: 'Archive (3)', disabled: false });
    expect(buildMultiSelectThreadContextMenuItems({ count: 2, hasRunningThread: true })).toContainEqual({ id: 'archive', label: 'Archive (2)', disabled: true });
    expect(buildMultiSelectThreadContextMenuItems({ count: 2, hasRunningThread: false }).map(item => item.label)).toEqual(['Mark unread (2)', 'Archive (2)', 'Delete (2)']);
  });
  test('isTrailingDoubleClick: single and synthetic clicks activate; the second and later clicks do not', () => {
    expect(isTrailingDoubleClick(1)).toBe(false);
    expect(isTrailingDoubleClick(0)).toBe(false);
    expect(isTrailingDoubleClick(2)).toBe(true);
    expect(isTrailingDoubleClick(3)).toBe(true);
  });
  test('orderItemsByPreferredIds keeps preferred ids first, skips stale ids and keeps the rest in order', () => {
    const items = [{ id: 'project-1' }, { id: 'project-2' }, { id: 'project-3' }];
    expect(orderItemsByPreferredIds({ items, preferredIds: ['project-3', 'project-missing', 'project-1'], getId: item => item.id }).map(item => item.id)).toEqual(['project-3', 'project-1', 'project-2']);
  });
  test('orderItemsByPreferredIds does not duplicate items when preferred ids repeat', () => {
    expect(orderItemsByPreferredIds({ items: [{ id: 'project-1' }, { id: 'project-2' }], preferredIds: ['project-2', 'project-1', 'project-2'], getId: item => item.id }).map(item => item.id)).toEqual(['project-2', 'project-1']);
  });
  test('orderItemsByPreferredIds resolves preference aliases', () => {
    const items = [{ id: 'physical-a', cwd: '/work/a' }, { id: 'physical-b', cwd: '/work/b' }, { id: 'physical-c', cwd: '/work/c' }];
    expect(orderItemsByPreferredIds({ items, preferredIds: ['legacy:/work/c', 'legacy:/work/a'], getId: item => item.id, getPreferenceIds: item => [item.id, `legacy:${item.cwd}`] }).map(item => item.id))
      .toEqual(['physical-c', 'physical-a', 'physical-b']);
  });
  test('resolveAdjacentThreadId resolves adjacent ids without wrapping', () => {
    const ids = ['thread-1', 'thread-2', 'thread-3'];
    expect(resolveAdjacentThreadId({ threadIds: ids, currentThreadId: 'thread-2', direction: 'previous' })).toBe('thread-1');
    expect(resolveAdjacentThreadId({ threadIds: ids, currentThreadId: 'thread-2', direction: 'next' })).toBe('thread-3');
    expect(resolveAdjacentThreadId({ threadIds: ids, currentThreadId: null, direction: 'next' })).toBe('thread-1');
    expect(resolveAdjacentThreadId({ threadIds: ids, currentThreadId: null, direction: 'previous' })).toBe('thread-3');
    expect(resolveAdjacentThreadId({ threadIds: ids, currentThreadId: 'thread-1', direction: 'previous' })).toBeNull();
  });

  // The reference's ThreadSummary fields as the V2 shell carries them (sidebar-model.ts shellRuntime/latestRun).
  const running: Obj = { latestRunId: 'turn-1', activeRunId: 'turn-1', status: 'running', interactionMode: 'plan', hasActionableProposedPlan: false };
  const completed: Obj = { latestRunId: 'turn-1', activeRunId: null, status: 'completed', latestRunCompletedAt: at(3), interactionMode: 'plan' };
  test('resolveThreadStatusPill: approval before everything, then input, then work', () => {
    expect(resolveThreadStatusPill(thread('t', { ...running, pendingRuntimeRequest: { kind: 'command_approval' } }), undefined)).toMatchObject({ label: 'Pending Approval', pulse: false });
    expect(resolveThreadStatusPill(thread('t', { ...running, pendingRuntimeRequest: { kind: 'user_input' } }), undefined)).toMatchObject({ label: 'Awaiting Input', pulse: false });
    expect(resolveThreadStatusPill(thread('t', running), undefined)).toMatchObject({ label: 'Working', pulse: true });
    expect(resolveThreadStatusPill(thread('t', { ...running, status: 'starting' }), undefined)).toMatchObject({ label: 'Connecting', pulse: true });
  });
  test('resolveThreadStatusPill: waiting while background work holds the completion, not for left-running commands', () => {
    expect(resolveThreadStatusPill(thread('t', { ...running, activeRunId: null, status: 'idle', pendingBackgroundTasks: [{ taskId: 'bg-1', kind: 'monitor' }] }), undefined))
      .toMatchObject({ label: 'Waiting', color: 'light-dark(#71717b, #818181)', pulse: false });
    expect(resolveThreadStatusPill(thread('t', { ...running, pendingBackgroundTasks: [{ taskId: 'bg-1', kind: 'command' }] }), undefined)).toMatchObject({ label: 'Working', pulse: true });
    expect(resolveThreadStatusPill(thread('t', { ...running, activeRunId: null, status: 'idle', pendingBackgroundTasks: [] }), undefined)).toBeNull();
  });
  test('resolveThreadStatusPill: plan ready after a settled plan turn; completed only against a visit marker', () => {
    expect(resolveThreadStatusPill(thread('t', { ...completed, hasActionableProposedPlan: true }), undefined)).toMatchObject({ label: 'Plan Ready', pulse: false });
    expect(resolveThreadStatusPill(thread('t', completed), undefined)).toBeNull();
    expect(resolveThreadStatusPill(thread('t', { ...completed, interactionMode: 'default' }), at(2))).toMatchObject({ label: 'Completed', pulse: false });
    expect(resolveThreadStatusPill(thread('t', { ...completed, interactionMode: 'default' }), at(4))).toBeNull();
  });
  const pill = (label: StatusPill['label']): StatusPill => ({ label, color: label, dot: label, pulse: label === 'Working' });
  test('resolveProjectStatusIndicator: null without a status; the highest priority wins', () => {
    expect(resolveProjectStatusIndicator([null, null])).toBeNull();
    expect(resolveProjectStatusIndicator([pill('Completed'), pill('Pending Approval'), pill('Working')])).toMatchObject({ label: 'Pending Approval' });
    expect(resolveProjectStatusIndicator([pill('Completed'), pill('Plan Ready')])).toMatchObject({ label: 'Plan Ready' });
    expect(resolveProjectStatusIndicator([pill('Waiting'), pill('Working')])).toMatchObject({ label: 'Working' });
    expect(resolveProjectStatusIndicator([pill('Plan Ready'), pill('Waiting')])).toMatchObject({ label: 'Waiting' });
  });

  const project = (id: string, title: string, extra: Obj = {}) => ({ id, title, createdAt: at(0), updatedAt: at(0), ...extra });
  test('sortProjectsForSidebar: the newest user message across threads, then project stamps, then name and id', () => {
    expect(sortProjectsForSidebar([project('project-1', 'Older project'), project('project-2', 'Newer project')], [
      thread('a', { projectId: 'project-1', updatedAt: at(20), messages: [{ role: 'user', createdAt: at(1) }] }),
      thread('b', { projectId: 'project-2', updatedAt: at(5), messages: [{ role: 'user', createdAt: at(5) }] }),
    ], 'updated_at').map(entry => entry.id)).toEqual(['project-2', 'project-1']);
    expect(sortProjectsForSidebar([project('project-1', 'Older', { updatedAt: at(1) }), project('project-2', 'Newer', { updatedAt: at(5) })], [], 'updated_at').map(entry => entry.id)).toEqual(['project-2', 'project-1']);
    expect(sortProjectsForSidebar([project('project-2', 'Beta', { createdAt: 'x', updatedAt: 'y' }), project('project-1', 'Alpha', { createdAt: 'x', updatedAt: 'y' })], [], 'updated_at').map(entry => entry.id)).toEqual(['project-1', 'project-2']);
  });
  test('sortProjectsForSidebar preserves manual order and ignores archived threads the caller filtered', () => {
    expect(sortProjectsForSidebar([project('project-2', 'Second'), project('project-1', 'First')], [], 'manual').map(entry => entry.id)).toEqual(['project-2', 'project-1']);
    const threads = [thread('v', { projectId: 'project-1', updatedAt: at(2) }), thread('x', { projectId: 'project-2', updatedAt: at(10), archivedAt: at(11) })].filter(entry => entry.archivedAt == null);
    expect(sortProjectsForSidebar([project('project-1', 'Visible', { updatedAt: at(1) }), project('project-2', 'Archived-only', { updatedAt: at(0) })], threads, 'updated_at').map(entry => entry.id)).toEqual(['project-1', 'project-2']);
  });
  test('reorderProjects (uiStateStore) moves the dragged keys to the target', () => {
    expect(reorderProjects(['a', 'b', 'c', 'd'], ['a'], ['c'])).toEqual(['b', 'c', 'a', 'd']);
    expect(reorderProjects(['a', 'b', 'c', 'd'], ['d'], ['b'])).toEqual(['a', 'd', 'b', 'c']);
    expect(reorderProjects(['a', 'b'], ['a'], ['a'])).toEqual(['a', 'b']);
  });
  test('formatRelativeTimeLabel: just now, then minutes, hours and days ago', () => {
    const now = Date.parse(at(30));
    expect(relativeTimeLabel(at(30), now)).toBe('just now');
    expect(relativeTimeLabel(at(25), now)).toBe('5m ago');
    expect(relativeTimeLabel('2026-03-09T07:00:00.000Z', now)).toBe('3h ago');
    expect(relativeTimeLabel('2026-03-07T10:00:00.000Z', now)).toBe('2d ago');
  });
});

describe('legacy menus (LegacySidebar.tsx)', () => {
  test('the thread menu has no pin, settle or snooze rows', () => {
    expect(legacyThreadMenuItems('feature/x').map(item => item.label)).toEqual(['New thread on feature/x', 'Rename thread', 'Mark unread', 'Copy Path', 'Copy Thread ID', 'Project settings', 'Delete']);
    expect(legacyThreadMenuItems('').map(item => item.id)).not.toContain('new-thread-on-branch');
  });
  test('the project menu targets one project, or a submenu of members for a group', () => {
    const one = legacyProjectMenuItems([{ id: 'p1', physicalKey: 'env:/a', title: 'A', workspaceRoot: '/a', environmentLabel: '', remote: false }]);
    expect(one.map(item => item.label)).toEqual(['Rename', 'Group into...', 'Copy Path', 'Project settings', 'Remove']);
    expect(one.find(item => item.label === 'Remove')).toMatchObject({ id: 'delete:env:/a', destructive: true });
    const two = legacyProjectMenuItems([{ id: 'p1', physicalKey: 'env:/a', title: 'A', workspaceRoot: '/a', environmentLabel: '', remote: false },
      { id: 'fleet:r:p9', physicalKey: 'r:/b', title: 'A', workspaceRoot: '/b', environmentLabel: 'Studio', remote: true }]);
    expect(two[0]).toMatchObject({ id: 'rename:submenu', label: 'Rename' });
    expect(two[0]!.children!.map(item => item.label)).toEqual(['/a', 'Studio — /b']);
    expect(two[2]!.children![1]).not.toHaveProperty('disabled');
    expect(two[4]!.children![1]).toMatchObject({ disabled: true, destructive: true });
  });
});

interface Fake { client: T3Client; calls: Obj[]; writes: Obj[]; dispatched: Obj[]; drafts: string[]; opened: string[] }
const identity = (name: string) => ({ projectMark: name.slice(0, 2), projectInk: 'ink', projectSurface: 'surface' });
function fake(threads: Obj[], options: { settings?: Obj; threadId?: string; picks?: string[]; modifiers?: Obj; projects?: Obj[]; hints?: boolean } = {}): Fake {
  const calls: Obj[] = [], writes: Obj[] = [], dispatched: Obj[] = [], drafts: string[] = [], opened: string[] = [], picks = [...(options.picks ?? [])];
  let ids = 0;
  const client = {
    shell: { projects: options.projects ?? [{ id: 'p1', title: 'Alpha', workspaceRoot: '/alpha', createdAt: at(0), updatedAt: at(0) }, { id: 'p2', title: 'Beta', workspaceRoot: '/beta', createdAt: at(1), updatedAt: at(1) }], threads },
    config: { environment: { capabilities: { threadVisitedTracking: true } }, providers: [], keybindings: [
      { command: 'thread.jump.1', shortcut: { key: '1', modKey: true } }, { command: 'thread.jump.2', shortcut: { key: '2', modKey: true } },
      { command: 'chat.new', shortcut: { key: 'n', modKey: true } }, { command: 'commandPalette.toggle', shortcut: { key: 'k', modKey: true } },
      { command: 'thread.previous', shortcut: { key: '[', modKey: true, shiftKey: true } }, { command: 'thread.next', shortcut: { key: ']', modKey: true, shiftKey: true } }] },
    environmentId: 'env', threadId: options.threadId ?? '', projectId: 'p1', query: '', connection: 'connected', writable: true, ready: true, revision: 1,
    presentation: { sidebarJumpHints: options.hints === true },
    local: { drafts: {}, snapshotDrafts: {}, deviceSettings: { timestampFormat: '24-hour' }, groupingMode: 'repository', groupingOverrides: {}, selections: {},
      clientSettings: { legacySidebarEnabled: true, confirmThreadArchive: false, confirmThreadDelete: true, sidebarProjectSortOrder: 'updated_at', sidebarThreadSortOrder: 'updated_at', sidebarThreadPreviewCount: 6, ...(options.settings ?? {}) } },
    projectGroups() { return client.shell.projects.map(project => ({ key: `env:${project.workspaceRoot}`, name: project.title, members: [project] })); },
    restAccess: () => ({
      ids: async (count: number) => Array.from({ length: count }, () => `c${ids++}`),
      request: async (method: string, payload: Obj) => { if (method === 'orchestration.dispatchCommand') dispatched.push(payload); return {}; },
      write: async (_storage: Files, pending: Obj) => { writes.push(pending); return {}; },
      call: async (request: Obj) => {
        calls.push(request);
        if (request.op === 'sidebarMenu') return { id: picks.shift() ?? null };
        if (request.op === 'sidebarModifiers') return options.modifiers ?? {};
        return {};
      },
    }),
    async openSelected(_native: Native, id: string) { opened.push(id); client.threadId = id; },
    async openDraft(_native: Native, projectId: string) { drafts.push(projectId); client.threadId = ''; },
  } as unknown as T3Client & { threadId: string; shell: { projects: Obj[]; threads: Obj[] } };
  return { client, calls, writes, dispatched, drafts, opened };
}
const native = { available: true, watch() {}, later: async () => ({}) } as unknown as Native;
const files = {} as Files;
const NOW = Date.parse(at(59));
const many = (count: number, projectId = 'p1') => Array.from({ length: count }, (_, index) => thread(`${projectId}-t${index}`, { projectId, latestUserMessageAt: at(50 - index) }));

describe('legacy projection (LegacySidebar.tsx SidebarProjectItem)', () => {
  test('off: the snapshot carries no projects', () => {
    expect(legacySidebarSnapshot(fake(many(2), { settings: { legacySidebarEnabled: false } }).client, NOW, { projectIdentity: identity })).toMatchObject({ enabled: false, projects: [] });
  });
  test('projects sort by their newest thread; each shows the preview count, Show more / Show less and the hidden status', async () => {
    const threads = [...many(8), thread('busy', { projectId: 'p1', latestUserMessageAt: at(1), latestRunId: 'r', activeRunId: 'r', status: 'running' }), thread('b1', { projectId: 'p2', latestUserMessageAt: at(55) })];
    const { client } = fake(threads);
    let view = legacySidebarSnapshot(client, NOW, { projectIdentity: identity });
    expect(view.projects.map(project => project.name)).toEqual(['Beta', 'Alpha']);
    const alpha = view.projects[1]!;
    expect(alpha.threads.map(row => row.id)).toEqual(['p1-t0', 'p1-t1', 'p1-t2', 'p1-t3', 'p1-t4', 'p1-t5']);
    expect(alpha).toMatchObject({ showMore: true, showLess: false, hiddenLabel: 'Working', hiddenPulse: true, expanded: true });
    expect(alpha.threads[0]).toMatchObject({ age: '9m ago', title: 'Thread p1-t0', active: false });
    await sidebarLocal(client, native, 'legacy-more', alpha.key, '');
    view = legacySidebarSnapshot(client, NOW, { projectIdentity: identity });
    expect(view.projects[1]!.threads).toHaveLength(9);
    expect(view.projects[1]).toMatchObject({ showMore: false, showLess: true });
    await sidebarLocal(client, native, 'legacy-less', alpha.key, '');
    await sidebarLocal(client, native, 'legacy-preview', '', '3');
    expect(legacySidebarSnapshot(client, NOW, { projectIdentity: identity }).projects[1]!.threads).toHaveLength(3);
    await sidebarLocal(client, native, 'legacy-preview', '', '99');
    expect(client.local.clientSettings!.sidebarThreadPreviewCount).toBe(15);
    await sidebarLocal(client, native, 'legacy-preview', '', '0');
    expect(client.local.clientSettings!.sidebarThreadPreviewCount).toBe(1);
    await sidebarLocal(client, native, 'legacy-preview', '', '');
    expect(client.local.clientSettings!.sidebarThreadPreviewCount).toBe(1);
    await sidebarLocal(client, native, 'legacy-preview-step', '', '-1');
    expect(client.local.clientSettings!.sidebarThreadPreviewCount).toBe(1);
    await sidebarLocal(client, native, 'legacy-preview-step', '', '1');
    expect(client.local.clientSettings!.sidebarThreadPreviewCount).toBe(2);
  });
  test('a collapsed project shows its status and keeps only the open thread; an empty expanded project says No threads yet', async () => {
    const { client } = fake([...many(3), thread('done', { projectId: 'p1', latestRunId: 'r', status: 'completed', latestRunCompletedAt: at(40), lastVisitedAt: at(30) })], { threadId: 'p1-t1' });
    const key = legacyGroups(client)[0]!.key;
    await sidebarLocal(client, native, 'legacy-toggle', key, '');
    expect(sidebarPrefs(client).projectExpanded[key]).toBe(false);
    const view = legacySidebarSnapshot(client, NOW, { projectIdentity: identity });
    const alpha = view.projects.find(project => project.key === key)!;
    expect(alpha).toMatchObject({ expanded: false, statusLabel: 'Completed', showMore: false, showEmpty: false });
    expect(alpha.threads.map(row => [row.id, row.active])).toEqual([['p1-t1', true]]);
    expect(view.projects.find(project => project.name === 'Beta')).toMatchObject({ showEmpty: true, threads: [] });
    expect(legacyVisibleThreadIds(client)).toEqual(['p1-t1']);
  });
  test('Sort threads and Sort projects follow the options; Manual follows the dragged order', async () => {
    const { client } = fake([thread('old', { projectId: 'p1', createdAt: at(1), latestUserMessageAt: at(50) }), thread('new', { projectId: 'p1', createdAt: at(9), latestUserMessageAt: at(2) }), thread('b', { projectId: 'p2', createdAt: at(0), latestUserMessageAt: at(40) })]);
    expect(legacyVisibleThreadIds(client)).toEqual(['old', 'new', 'b']);
    await sidebarLocal(client, native, 'legacy-sort-threads', '', 'created_at');
    expect(legacyVisibleThreadIds(client)).toEqual(['new', 'old', 'b']);
    await sidebarLocal(client, native, 'legacy-sort-projects', '', 'created_at');
    expect(legacyVisibleThreadIds(client)).toEqual(['new', 'old', 'b']);
    await sidebarLocal(client, native, 'legacy-sort-projects', '', 'manual');
    const keys = legacyGroups(client).map(group => group.key);
    await sidebarLocal(client, native, 'legacy-reorder', keys[1]!, keys[0]!);
    expect(sidebarPrefs(client).projectOrder).toEqual(['env:/beta', 'env:/alpha']);
    expect(legacyVisibleThreadIds(client)).toEqual(['b', 'new', 'old']);
    expect(projectScopes(client).map(scope => scope.key)).toEqual(['env:/beta', 'env:/alpha']); // the palette and picker read the same order
    await sidebarLocal(client, native, 'legacy-reorder', keys[1]!, '');
    expect(sidebarPrefs(client).projectOrder).toEqual(['env:/alpha', 'env:/beta']);
    expect(() => sidebarLocal(client, native, 'legacy-sort-threads', '', 'manual')).toThrow();
  });
  test('jump labels and traversal follow the visible order; ⌘ shows labels for the first nine', () => {
    const { client } = fake([...many(2), thread('b', { projectId: 'p2', latestUserMessageAt: at(55) })], { threadId: 'p1-t0', hints: true });
    const view = legacySidebarSnapshot(client, NOW, { projectIdentity: identity });
    expect(view.projects.flatMap(project => project.threads.map(row => [row.id, row.jump]))).toEqual([['b', '⌘1'], ['p1-t0', '⌘2'], ['p1-t1', '']]);
    expect(legacyTraversal(client)).toMatchObject({ previous: 'b', next: 'p1-t1' });
    const dispatch = keyboardDispatch(client, [], '', '', { composerFocus: false, editableFocus: false, turnRunning: false, modelPickerOpen: false, draftThreadRoute: false, modalOpen: false, settingsOpen: false, diffOpen: false, paletteOpen: false, paletteMode: '', prNumber: '' } as never);
    expect(dispatch.filter(item => item.command.startsWith('thread.')).map(item => [item.command, item.target])).toEqual([['thread.previous', 'b'], ['thread.next', 'p1-t1'], ['thread.jump.1', 'b'], ['thread.jump.2', 'p1-t0']]);
    expect(view).toMatchObject({ newThreadTip: 'New thread (⌘N)', searchShortcut: '⌘K' });
  });
  test('row details: PR state and tooltip, worktree, running blocks archive, selection', () => {
    const { client } = fake([thread('pr', { projectId: 'p1', worktreePath: '/wt/pr', branch: 'feat', pullRequests: [{ number: 7, url: 'https://github.com/a/b/pull/7', state: 'open', snapshot: { state: 'open', isDraft: false, title: 'Ship it' } }] }),
      thread('run', { projectId: 'p1', latestRunId: 'r', activeRunId: 'r', status: 'running' })]);
    sidebarSession(client).selection = ['run'];
    const rows = legacySidebarSnapshot(client, NOW, { projectIdentity: identity }).projects.find(project => project.name === 'Alpha')!.threads;
    expect(rows.find(row => row.id === 'pr')).toMatchObject({ prIcon: 'git-pull-request-arrow', prState: 'open', prTooltip: 'PR #7 - Open: Ship it', worktree: true, worktreeLabel: 'Worktree: /wt/pr (feat)', running: false });
    expect(rows.find(row => row.id === 'run')).toMatchObject({ running: true, selected: true, statusLabel: 'Working', label: 'Thread run, Working' });
  });
});

describe('environment grouping (environmentGrouping.test.ts) in the legacy sidebar', () => {
  const identityKey = { canonicalKey: 'github.com/example/shared-repo' };
  const fleetWith = (projects: Obj[]) => {
    const source = new EnvironmentFleet();
    source.entries.set('remote', { key: 'remote', origin: 'http://remote', environmentId: 'env-remote', phase: 'connected', message: '', traceId: '', generation: 1, synchronized: 1, lastEvent: 0,
      subscriptions: {}, config: { environment: { label: 'Studio' } }, shell: { projects, threads: [] } as never, scopes: [], error: '', requested: false });
    return source;
  };
  test('groups matching repository identities across environments; a remote-only group is marked', () => {
    const { client } = fake([], { projects: [{ id: 'p1', title: 'shared-repo', workspaceRoot: '/tmp/shared-repo', repositoryIdentity: identityKey }] });
    const groups = legacyGroups(client, fleetWith([{ id: 'p9', title: 'shared-repo', workspaceRoot: '/srv/shared-repo', repositoryIdentity: identityKey }, { id: 'p8', title: 'solo', workspaceRoot: '/srv/solo' }]));
    expect(groups.map(group => [group.key, group.members.map(member => member.id), group.remoteOnly])).toEqual([
      ['github.com/example/shared-repo', ['p1', 'fleet:env-remote:p9'], false], ['env-remote:/srv/solo', ['fleet:env-remote:p8'], true]]);
    expect(groups[1]!.remoteLabels).toEqual(['Studio']);
  });
  test('projects without a repository identity stay physically scoped', () => {
    const { client } = fake([], { projects: [{ id: 'p1', title: 'shared-repo', workspaceRoot: '/tmp/shared-repo' }] });
    expect(legacyGroups(client, fleetWith([{ id: 'p9', title: 'shared-repo', workspaceRoot: '/tmp/shared-repo' }])).map(group => group.key)).toEqual(['env:/tmp/shared-repo', 'env-remote:/tmp/shared-repo']);
  });
  test('separate mode and per-project overrides decide the key', () => {
    const { client } = fake([], { projects: [{ id: 'p1', title: 'shared-repo', workspaceRoot: '/tmp/shared-repo', repositoryIdentity: identityKey }] });
    client.local.groupingMode = 'separate';
    expect(legacyGroups(client, fleetWith([]))[0]!.key).toBe('env:/tmp/shared-repo');
    client.local.groupingOverrides['env:/tmp/shared-repo'] = 'repository';
    expect(legacyGroups(client, fleetWith([]))[0]!.key).toBe('github.com/example/shared-repo');
    client.local.groupingMode = 'repository'; client.local.groupingOverrides['env:/tmp/shared-repo'] = 'separate';
    expect(legacyGroups(client, fleetWith([]))[0]!.key).toBe('env:/tmp/shared-repo');
  });
  test('keeps manual project order when building grouped entries', () => {
    const { client } = fake([], { projects: [{ id: 'p1', title: 'shared-repo', workspaceRoot: '/tmp/shared-repo', repositoryIdentity: identityKey }, { id: 'p3', title: 'separate', workspaceRoot: '/tmp/separate' }] });
    sidebarPrefs(client).projectOrder = ['env:/tmp/separate', 'env:/tmp/shared-repo'];
    expect(legacyGroups(client, fleetWith([{ id: 'p9', title: 'shared-repo', workspaceRoot: '/srv/shared-repo', repositoryIdentity: identityKey }])).map(group => group.name)).toEqual(['separate', 'shared-repo']);
  });
});

describe('legacy gestures (LegacySidebar.tsx handlers)', () => {
  test('⌘N creates in the current project at once in legacy mode; the default sidebar opens New thread in…', async () => {
    const legacy = fake([]);
    expect(await sidebarCommand(legacy.client, native, files, 'new-thread-click', '', '')).toBe('sidebar:new-thread');
    expect(legacy.drafts).toEqual(['p1']);
    const flat = fake([], { settings: { legacySidebarEnabled: false } });
    expect(await sidebarCommand(flat.client, native, files, 'new-thread-click', '', '')).toBe('sidebar:palette-new-thread');
  });
  test('the project New thread button creates in its project', async () => {
    const { client, drafts } = fake([]);
    expect(await sidebarCommand(client, native, files, 'legacy-new-thread', 'env:/beta', '')).toBe('sidebar:new-thread');
    expect(drafts).toEqual(['p2']);
  });
  test('⇧-click ranges over the project list', async () => {
    const { client } = fake([...many(4), thread('b', { projectId: 'p2', latestUserMessageAt: at(55) })], { modifiers: { shift: true } });
    sidebarSession(client).anchor = 'p1-t0';
    expect(await sidebarSelecting(client, native, 'p1-t2', 'click')).toBe(true);
    expect(sidebarSession(client).selection).toEqual(['p1-t0', 'p1-t1', 'p1-t2']);
  });
  test('thread menu: rename starts the inline edit; copy path toasts; delete asks first', async () => {
    const { client, calls } = fake([thread('a', { projectId: 'p1', branch: 'feat' })], { picks: ['rename', 'copy-path', 'delete', 'copy-thread-id'] });
    await sidebarCommand(client, native, files, 'legacy-thread-menu', 'a', '');
    expect(calls[0]!.items).toEqual(expect.arrayContaining([expect.objectContaining({ id: 'new-thread-on-branch', label: 'New thread on feat' })]));
    expect(sidebarSession(client)).toMatchObject({ renameId: 'a', renameTitle: 'Thread a' });
    await sidebarCommand(client, native, files, 'legacy-thread-menu', 'a', '');
    expect(calls.find(call => call.op === 'copyText')).toMatchObject({ text: '/alpha' });
    expect(toasts(client).at(-1)).toMatchObject({ kind: 'success', title: 'Path copied', description: '/alpha' });
    await sidebarCommand(client, native, files, 'legacy-thread-menu', 'a', '');
    expect(sidebarSession(client).dialog).toEqual({ kind: 'delete', threadIds: ['a'], title: 'Thread a' });
    await sidebarCommand(client, native, files, 'legacy-thread-menu', 'a', '');
    expect(toasts(client).at(-1)).toMatchObject({ title: 'Thread ID copied', description: 'a' });
  });
  test('a selected row opens the multi-selection menu: mark unread, archive behind the confirm setting', async () => {
    const { client, calls, dispatched } = fake([thread('a'), thread('b')], { picks: ['mark-unread', 'archive', 'archive'], settings: { confirmThreadArchive: true } });
    sidebarSession(client).selection = ['a', 'b'];
    await sidebarCommand(client, native, files, 'legacy-thread-menu', 'a', '');
    expect(calls[0]!.items).toEqual([{ type: 'item', id: 'mark-unread', label: 'Mark unread (2)', enabled: true, destructive: false },
      { type: 'item', id: 'archive', label: 'Archive (2)', enabled: true, destructive: false }, { type: 'separator' }, { type: 'item', id: 'delete', label: 'Delete (2)', enabled: true, destructive: true }]);
    expect(dispatched.map(payload => payload.type)).toEqual(['thread.mark-unread', 'thread.mark-unread']);
    expect(sidebarSession(client).selection).toEqual([]);
    sidebarSession(client).selection = ['a', 'b'];
    await sidebarCommand(client, native, files, 'legacy-thread-menu', 'b', '');
    expect(legacySidebarSnapshot(client, NOW, { projectIdentity: identity })).toMatchObject({ dialog: 'archive-many', dialogTitle: 'Archive 2 threads?' });
    await sidebarCommand(client, native, files, 'legacy-dialog-confirm', '', '');
    expect(dispatched.slice(2).map(payload => [payload.type, payload.threadId])).toEqual([['thread.archive', 'a'], ['thread.archive', 'b']]);
    expect(sidebarSession(client).selection).toEqual([]);
  });
  test('inline archive dispatches; a running thread refuses with the reference toast', async () => {
    const { client, dispatched } = fake([thread('a'), thread('r', { latestRunId: 'x', activeRunId: 'x', status: 'running' })]);
    await sidebarCommand(client, native, files, 'legacy-archive', 'a', '');
    expect(dispatched.map(payload => payload.type)).toEqual(['thread.archive']);
    await sidebarCommand(client, native, files, 'legacy-archive', 'r', '');
    expect(toasts(client).at(-1)).toMatchObject({ title: 'Failed to archive thread', description: 'Stop the running turn before archiving this thread.' });
  });
  test('project menu: Remove on a project with threads warns with Delete anyway, then confirms and removes', async () => {
    const { client, writes } = fake([thread('a')], { picks: ['delete:env:/alpha'] });
    await sidebarCommand(client, native, files, 'legacy-project-menu', 'env:/alpha', '');
    expect(toasts(client).at(-1)).toMatchObject({ kind: 'warning', title: 'Project is not empty', description: 'Delete all threads in this project before removing it.', action: { label: 'Delete anyway', op: 'sidebar:legacy-remove-anyway', id: 'p1' } });
    await sidebarCommand(client, native, files, 'legacy-remove-anyway', 'p1', '');
    const view = legacySidebarSnapshot(client, NOW, { projectIdentity: identity });
    expect(view).toMatchObject({ dialog: 'remove-project', dialogTitle: 'Remove project "Alpha" and delete its 1 thread?' });
    expect(view.dialogDescription.split('\n')).toEqual(['Path: /alpha', 'This permanently clears conversation history for those threads and any archived threads.', 'This removes only this project entry.', 'This action cannot be undone.']);
    await sidebarCommand(client, native, files, 'legacy-dialog-confirm', '', '');
    expect(writes.map(write => [write.method, obj(write.payload).type, obj(write.payload).projectId, obj(write.payload).force])).toEqual([['projects.mutate', 'project.delete', 'p1', true]]);
  });
  test('project menu: an empty project asks the plain removal; rename refuses an empty title and saves a new one', async () => {
    const { client, writes } = fake([], { picks: ['delete:env:/beta', 'rename:env:/beta', 'grouping:env:/beta'] });
    await sidebarCommand(client, native, files, 'legacy-project-menu', 'env:/beta', '');
    expect(legacySidebarSnapshot(client, NOW, { projectIdentity: identity })).toMatchObject({ dialog: 'remove-project', dialogTitle: 'Remove project "Beta"?',
      dialogDescription: 'Path: /beta\nThis permanently clears any archived conversation history.\nThis removes only this project entry.' });
    await sidebarLocal(client, native, 'legacy-dialog-close', '', '');
    await sidebarCommand(client, native, files, 'legacy-project-menu', 'env:/beta', '');
    expect(legacySidebarSnapshot(client, NOW, { projectIdentity: identity })).toMatchObject({ dialog: 'rename-project', dialogTitle: 'Rename project', dialogDescription: 'Update the title for /beta.', dialogField: 'Beta', dialogTarget: 'p2' });
    await sidebarCommand(client, native, files, 'legacy-rename-project', 'p2', '   ');
    expect(toasts(client).at(-1)).toMatchObject({ kind: 'warning', title: 'Project title cannot be empty' });
    expect(legacySession(client).dialog.kind).toBe('rename-project');
    await sidebarCommand(client, native, files, 'legacy-rename-project', 'p2', ' Gamma ');
    expect(writes.map(write => obj(write.payload))).toEqual([expect.objectContaining({ type: 'project.update', projectId: 'p2', title: 'Gamma' })]);
    expect(legacySession(client).dialog.kind).toBe('');
    await sidebarCommand(client, native, files, 'legacy-project-menu', 'env:/beta', '');
    expect(legacySidebarSnapshot(client, NOW, { projectIdentity: identity })).toMatchObject({ dialog: 'grouping', dialogGrouping: 'inherit', dialogGroupingLabel: 'Use global default (Group by repository)' });
    await sidebarLocal(client, native, 'legacy-dialog-grouping', '', 'separate');
    expect(legacySidebarSnapshot(client, NOW, { projectIdentity: identity })).toMatchObject({ dialogGroupingDescription: 'Every project path gets its own sidebar row.' });
    await sidebarCommand(client, native, files, 'legacy-grouping-save', '', '');
    expect(client.local.groupingOverrides).toEqual({ 'env:/beta': 'separate' });
  });
  test('a failed rename keeps the dialog and toasts', async () => {
    const { client } = fake([]);
    (client as unknown as { restAccess: () => Obj }).restAccess = () => ({ ids: async () => ['c'], write: async () => { throw new Error('Project title is taken.'); }, call: async () => ({}) });
    legacySession(client).dialog = { kind: 'rename-project', memberId: 'p1', title: 'Alpha', root: '/alpha', environment: '', grouping: 'inherit', threads: 0, ids: [] };
    await sidebarCommand(client, native, files, 'legacy-rename-project', 'p1', 'Other');
    expect(toasts(client).at(-1)).toMatchObject({ kind: 'error', title: 'Failed to rename project', description: 'Project title is taken.' });
    expect(legacySession(client).dialog.kind).toBe('rename-project');
  });
});

function obj(value: unknown): Obj { return value && typeof value === 'object' ? value as Obj : {}; }

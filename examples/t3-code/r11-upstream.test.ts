// Lane r11-upstream: the upstream commits f90b77d809..f870c419fc that change
// the client. 1826fb55cc (sweep a row action), 95edeb753b (draft context menu,
// discard behind Undo) and 737993303d (retry a failed workspace preparation).
import { describe, expect, test } from 'bun:test';
import './client';
import type { T3Client } from './client';
import type { Obj } from './domain';
import type { Files, Native } from './protocol';
import { toasts } from './toast';
import { nativeTemplate } from './sidebar-menu';
import { sidebarSnapshot } from './sidebar-view';
import { commitExpiredUndos, sidebarCommand, undoLatest } from './sidebar-commands';
import { adoptCommandTime, sidebarSession, wall } from './sidebar-state';
import { parseSweep, resolveSidebarSweepKeys } from './r11-upstream-sweep';
import { draftMenuItems } from './r11-upstream-drafts';
import { preparationFailureRunId, retryWorkspacePreparation, turnItemIsWorkspacePreparation, workspacePreparationRetryRunIds } from './r11-upstream-retry';
import { timelineEntries } from './timeline-rows';
import { availableScratchWorkspaceRoot, openScratchProject } from './r11-upstream-scratch';

const NOW = Date.parse('2026-10-04T12:00:00.000Z');
const iso = (offset: number) => new Date(NOW + offset).toISOString();
const CAPS = { threadSettlement: true, threadSnooze: true, threadPinning: true, threadPinReorder: true, threadActiveReorder: true, threadVisitedTracking: true };
const shell = (id: string, extra: Obj = {}): Obj => ({ id, projectId: 'p1', title: `Thread ${id}`, status: 'idle', latestRunId: null,
  activeProviderThreadId: null, pendingRuntimeRequest: null, createdAt: iso(-3_600_000), updatedAt: iso(-3_600_000), archivedAt: null,
  settledOverride: null, settledAt: null, modelSelection: { instanceId: 'codex', model: 'm' }, lineage: { relationshipToParent: null }, ...extra });

function fake(threads: Obj[], options: { caps?: Obj; picks?: string[]; threadId?: string } = {}) {
  const dispatched: Obj[] = [], calls: Obj[] = [], picks = [...(options.picks ?? [])];
  let ids = 0;
  const client = {
    shell: { projects: [{ id: 'p1', title: 'Parity fixture', workspaceRoot: '/fixture' }, { id: 'p2', title: 'Other', workspaceRoot: '/other' }], threads, sequence: 1 },
    config: { environment: { capabilities: { ...CAPS, ...(options.caps ?? {}) } }, providers: [], keybindings: [] },
    environmentId: 'env', threadId: options.threadId ?? '', projectId: 'p1', query: '', connection: 'connected', writable: true, ready: true,
    presentation: {}, local: { drafts: {}, snapshotDrafts: {}, snapshotReleases: [], composerControls: { contexts: {} }, deviceSettings: { timestampFormat: '24-hour' },
      clientSettings: { confirmThreadArchive: false, confirmThreadDelete: true, confirmThreadUnpin: false, sidebarWorkingShelfEnabled: false }, sidebarWidth: 256 },
    projection: { runs: [], turnItems: [] },
    projectGroups() { return [{ key: 'g1', name: 'Parity fixture', members: [{ id: 'p1' }] }, { key: 'g2', name: 'Other', members: [{ id: 'p2' }] }]; },
    restAccess: () => ({
      ids: async (count: number) => Array.from({ length: count }, () => `c${ids++}`),
      request: async (method: string, payload: Obj) => { if (method === 'orchestration.dispatchCommand') dispatched.push(payload); return {}; },
      dispatch: async (_storage: Files, payload: Obj, description: string) => { dispatched.push({ ...payload, description }); return {}; },
      call: async (request: Obj) => { calls.push(request); return request.op === 'sidebarMenu' ? { id: picks.shift() ?? null } : {}; },
    }),
    async openSelected(_native: Native, id: string) { client.threadId = id; },
    async openDraft(_native: Native, _projectId: string) { client.threadId = ''; },
  } as unknown as T3Client & { threadId: string };
  adoptCommandTime(client, NOW);
  return { client, dispatched, calls };
}
const native = { available: true, watch() {}, later: async () => ({}) } as unknown as Native;
const files = {} as Files;
const helpers = { projectIdentity: (name: string) => ({ projectMark: name.slice(0, 2).toUpperCase(), projectInk: 'ink', projectSurface: 'surface' }),
  providerBadge: () => ({ providerBadge: '', providerBadgeColor: '' }) };
const strip = (payloads: Obj[]) => payloads.map(({ commandId: _id, ...rest }) => rest);
const local = (client: T3Client) => client.local as unknown as { drafts: Record<string, string>; snapshotDrafts: Record<string, Obj[]>; snapshotReleases: string[]; composerControls: { contexts: Record<string, Obj> } };

describe('sweep sidebar row actions (1826fb55cc)', () => {
  test('resolveSidebarSweepKeys covers the rows between the pressed row and the pointer, in either direction', () => {
    const ordered = ['a', 'b', 'c', 'd', 'blocked'], canSettle = (key: string) => key !== 'blocked';
    expect(resolveSidebarSweepKeys(ordered, 'b', 'b', canSettle)).toEqual(['b']);
    expect(resolveSidebarSweepKeys(ordered, 'b', 'd', canSettle)).toEqual(['b', 'c', 'd']);
    expect(resolveSidebarSweepKeys(ordered, 'd', 'a', canSettle)).toEqual(['a', 'b', 'c', 'd']);
    expect(resolveSidebarSweepKeys(ordered, 'c', 'blocked', canSettle)).toEqual(['c', 'd']);
    expect(resolveSidebarSweepKeys(ordered, 'gone', 'a', canSettle)).toEqual([]);
  });

  test('the release names the action, the section, the pressed row and the armed rows', () => {
    expect(parseSweep('sweep|settle|active|a||a|b|')).toEqual({ action: 'settle', section: 'active', origin: 'a', keys: ['a', 'b'] });
    expect(parseSweep('sweep|unsnooze|snoozed|s1|')).toEqual({ action: 'unsnooze', section: 'snoozed', origin: 's1', keys: [] });
    expect(parseSweep('sweep|delete|active|a|')).toBeNull();
  });

  test('a settle sweep settles every armed row still in its section, under one undo notice', async () => {
    const { client, dispatched } = fake([shell('a', { updatedAt: iso(-1000) }), shell('b', { updatedAt: iso(-2000) }), shell('c', { updatedAt: iso(-3000) }),
      shell('p', { pinnedAt: iso(-1) })]);
    const epoch = sidebarSession(client).sweepEpoch;
    await sidebarCommand(client, native, files, 'drop', 'sweep|settle|active|a||a|b|p|', '|0|0|0|false', NOW);
    expect(strip(dispatched)).toEqual([{ type: 'thread.settle', threadId: 'a' }, { type: 'thread.settle', threadId: 'b' }]);
    const view = sidebarSnapshot(client, NOW, helpers).sidebar;
    expect([view.undoText, view.sweepEpoch]).toEqual(['Settled 2 threads,', epoch + 1]);
  });

  test('a pan that armed nothing is the button click; un-settle and wake sweeps act on each row', async () => {
    const { client, dispatched } = fake([shell('a'), shell('s1', { settledOverride: 'settled', settledAt: iso(-60_000) }), shell('s2', { settledOverride: 'settled', settledAt: iso(-90_000) }),
      shell('z', { snoozedUntil: iso(3_600_000) })]);
    await sidebarCommand(client, native, files, 'drop', 'sweep|settle|active|a|', '', NOW);
    await sidebarCommand(client, native, files, 'drop', 'sweep|unsettle|settled|s1||s1|s2|', '', NOW);
    await sidebarCommand(client, native, files, 'drop', 'sweep|unsnooze|snoozed|z||z|', '', NOW);
    expect(strip(dispatched)).toEqual([{ type: 'thread.settle', threadId: 'a' }, { type: 'thread.unsettle', threadId: 's1', reason: 'user' },
      { type: 'thread.unsettle', threadId: 's2', reason: 'user' }, { type: 'thread.unsnooze', threadId: 'z', reason: 'user' }]);
  });

  test('without settlement support nothing is swept', async () => {
    const { client, dispatched } = fake([shell('a'), shell('b')], { caps: { threadSettlement: false } });
    await sidebarCommand(client, native, files, 'drop', 'sweep|settle|active|a||a|b|', '', NOW);
    expect(dispatched).toEqual([]);
  });
});

describe('draft rows: context menu and discard behind Undo (95edeb753b)', () => {
  test('the menu is Copy (Path, Branch), Project settings and the destructive Discard draft', () => {
    expect(nativeTemplate(draftMenuItems({ hasPath: true, hasBranch: true, hasProject: true }))).toEqual([
      { type: 'submenu', id: 'copy', label: 'Copy', enabled: true, destructive: false, children: [
        { type: 'item', id: 'copy-path', label: 'Path', enabled: true, destructive: false }, { type: 'item', id: 'copy-branch', label: 'Branch', enabled: true, destructive: false }] },
      { type: 'item', id: 'project-settings', label: 'Project settings', enabled: true, destructive: false },
      { type: 'separator' },
      { type: 'item', id: 'discard', label: 'Discard draft', enabled: true, destructive: true }]);
    expect(draftMenuItems({ hasPath: false, hasBranch: false, hasProject: false }).map(item => [item.id, item.disabled === true])).toEqual([['copy', true], ['discard', false]]);
  });

  test('right click copies the draft workspace path and branch, opens project settings, or discards it', async () => {
    const { client, calls } = fake([shell('a')], { picks: ['copy-path', 'copy-branch', 'project-settings'] });
    local(client).drafts['env:new:p2'] = 'Sketch the parser';
    local(client).composerControls.contexts['env:new:p2'] = { envMode: 'local', branch: 'feature/x', worktreePath: '' };
    expect(await sidebarCommand(client, native, files, 'draft-menu', 'p2', '')).toBe('');
    expect(calls.filter(call => call.op === 'copyText').map(call => call.text)).toEqual(['/other']);
    expect(toasts(client).map(toast => [toast.title, toast.description])).toContainEqual(['Path copied', '/other']);
    await sidebarCommand(client, native, files, 'draft-menu', 'p2', '');
    expect(calls.filter(call => call.op === 'copyText').map(call => call.text)).toEqual(['/other', 'feature/x']);
    expect(await sidebarCommand(client, native, files, 'draft-menu', 'p2', '')).toBe('sidebar:navigate');
    expect(sidebarSession(client).navigate).toEqual({ kind: 'project-settings', projectId: 'p2' });
    const menu = calls.find(call => call.op === 'sidebarMenu')!.items as Obj[];
    expect(menu.map(item => item.label)).toEqual(['Copy', 'Project settings', undefined, 'Discard draft']);
  });

  test('Discard draft clears the session; Undo brings back the text, images and workspace choice', async () => {
    const { client } = fake([shell('a')], { picks: ['discard'] });
    local(client).drafts['env:new:p2'] = 'Sketch the parser';
    local(client).snapshotDrafts['env:new:p2'] = [{ id: 'img-1' }];
    local(client).composerControls.contexts['env:new:p2'] = { envMode: 'worktree', branch: 'main', worktreePath: '' };
    await sidebarCommand(client, native, files, 'draft-menu', 'p2', '');
    expect([local(client).drafts['env:new:p2'], local(client).snapshotDrafts['env:new:p2'], local(client).composerControls.contexts['env:new:p2']]).toEqual([undefined, undefined, undefined]);
    expect(sidebarSnapshot(client, NOW, helpers).sidebar.undoText).toBe('Discarded 1 draft,');
    expect(await undoLatest(client, native)).toBe(true);
    expect([local(client).drafts['env:new:p2'], local(client).snapshotDrafts['env:new:p2'], local(client).composerControls.contexts['env:new:p2']])
      .toEqual(['Sketch the parser', [{ id: 'img-1' }], { envMode: 'worktree', branch: 'main', worktreePath: '' }]);
    expect(local(client).snapshotReleases).toEqual([]);
  });

  test('Undo refuses a draft that has new content; an expired discard releases its uploads', async () => {
    const { client } = fake([shell('a')]);
    local(client).drafts['env:a'] = 'reply text';
    local(client).snapshotDrafts['env:a'] = [{ id: 'img-a' }];
    local(client).drafts['env:new:p2'] = 'idea';
    local(client).snapshotDrafts['env:new:p2'] = [{ id: 'img-b' }];
    await sidebarCommand(client, native, files, 'discard-draft', 'a', '');
    await sidebarCommand(client, native, files, 'discard-project-draft', 'p2', '');
    expect(sidebarSnapshot(client, NOW, helpers).sidebar.undoText).toBe('Discarded 2 drafts,');
    local(client).drafts['env:a'] = 'typed again';
    await undoLatest(client, native);
    expect(toasts(client).map(toast => [toast.title, toast.description])).toContainEqual(['Failed to restore draft', 'The draft has new content.']);
    expect([local(client).drafts['env:a'], local(client).drafts['env:new:p2']]).toEqual(['typed again', 'idea']);
    expect(local(client).snapshotReleases).toEqual(['img-a']);
    local(client).drafts['env:new:p2'] = 'idea 2';
    local(client).snapshotDrafts['env:new:p2'] = [{ id: 'img-c' }];
    await sidebarCommand(client, native, files, 'discard-project-draft', 'p2', '');
    adoptCommandTime(client, wall(client) + 6000);
    commitExpiredUndos(client);
    expect(local(client).snapshotReleases).toEqual(['img-a', 'img-c']);
  });
});

describe('retry a failed workspace preparation (737993303d)', () => {
  const failure = (status: string, code: string | null = 'workspace_preparation_failed'): Obj => ({ id: 'item-error', type: 'error', status, runId: 'run-1',
    title: 'Workspace preparation failed', failure: { class: 'validation_error', message: 'fetch failed', code, retryable: false } });
  const run = (status: string, workspacePreparation?: Obj): Obj => ({ id: 'run-1', status, ...(workspacePreparation ? { workspacePreparation } : {}) });
  const worktree = { type: 'worktree', baseRef: 'main' };

  test('a retry hides the failure it cancelled, and only that', () => {
    expect(turnItemIsWorkspacePreparation({ type: 'command_execution', input: 'Preparing workspace' })).toBe(true);
    expect(turnItemIsWorkspacePreparation({ type: 'command_execution', input: 'prepare workspace' })).toBe(false);
    expect(turnItemIsWorkspacePreparation(failure('failed'))).toBe(false);
    expect(turnItemIsWorkspacePreparation(failure('cancelled'))).toBe(true);
    expect(turnItemIsWorkspacePreparation(failure('cancelled', null))).toBe(false);
    const rows = [{ sourceThreadId: 't', sourceItemId: 'x', item: failure('cancelled') }];
    expect(timelineEntries({ rows, runs: [], attempts: [], nodes: [], checkpoints: [] } as never)).toEqual([]);
  });

  test('Retry is offered while the run still ends in its failed preparation', () => {
    const items = [failure('failed')];
    expect([...workspacePreparationRetryRunIds([run('failed', worktree)], items)]).toEqual(['run-1']);
    expect(workspacePreparationRetryRunIds([run('preparing', worktree)], items).size).toBe(0);
    expect(workspacePreparationRetryRunIds([run('failed')], items).size).toBe(0);
    expect(workspacePreparationRetryRunIds([run('failed', worktree)], [failure('failed', 'provider_crashed')]).size).toBe(0);
    expect([preparationFailureRunId(failure('failed')), preparationFailureRunId(failure('failed', 'provider_crashed'))]).toEqual(['run-1', '']);
  });

  test('Retry dispatches prepared-run.retry for the open thread', async () => {
    const { client, dispatched } = fake([shell('a')], { threadId: 'a' });
    await retryWorkspacePreparation(client, native, files, 'run-1');
    expect(strip(dispatched)).toEqual([{ type: 'prepared-run.retry', threadId: 'a', runId: 'run-1', description: 'Retry workspace preparation' }]);
  });
});

describe("one way to open a machine's No project folder (845ddd9354)", () => {
  const scratchClient = (shellAfter: Obj[], projectId = 'scratch') => {
    const calls: string[] = [];
    const client = { shell: { projects: [{ id: 'p1' }], threads: [], sequence: 1 },
      rpc: async (_native: Native, method: string) => { calls.push(method); return { projectId }; },
      restAccess: () => ({ http: async (path: string) => { calls.push(path); return { snapshotSequence: 2, projects: shellAfter, threads: [] }; } }) } as unknown as T3Client;
    return { client, calls };
  };

  test('the folder is usable once its project reaches the shell; otherwise the reference message', async () => {
    const ok = scratchClient([{ id: 'p1' }, { id: 'scratch', workspaceRoot: '/home/.t3/scratch' }]);
    expect(await openScratchProject(ok.client, native)).toBe('scratch');
    expect(ok.calls).toEqual(['projects.ensureScratch', '/api/orchestration/shell']);
    const missing = scratchClient([{ id: 'p1' }]);
    await expect(openScratchProject(missing.client, native)).rejects.toThrow('The folder for threads without a project has not reached this device yet. Try again.');
  });

  test('availableScratchWorkspaceRoot offers the folder only while connected', () => {
    expect(availableScratchWorkspaceRoot('connected', { scratchWorkspaceRoot: '/s' })).toBe('/s');
    expect(availableScratchWorkspaceRoot('connecting', { scratchWorkspaceRoot: '/s' })).toBeNull();
    expect(availableScratchWorkspaceRoot('connected', {})).toBeNull();
  });
});

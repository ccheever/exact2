// Independent integration regressions for the root-owned adapter, using isolated clients.
import { expect, test } from 'bun:test';
import { T3Client } from './shared/client';
import { obj } from './shared/domain';
import type { Native } from './shared/protocol';
import { mobileFilesRead } from './file-data';
import { mobileInspectorContext, mobileInspectorTransition, mobileInspectorPresentation } from './workspace-inspector-adapter';
import { workspaceInspectorSnapshot } from './workspace-inspector';

function fixture() {
  const client = new T3Client();
  Object.assign(client, { origin: 'https://example.test', environmentId: 'env', projectId: 'p', threadId: 'one', generation: 9,
    connection: 'connected', configLive: true, shellLive: true, threadLive: true });
  client.shell.projects = [{ id: 'p', title: 'Repo', workspaceRoot: '/repo' }];
  client.shell.threads = [{ id: 'one', projectId: 'p', branch: 'old-branch', worktreePath: null },
    { id: 'two', projectId: 'p', branch: 'new-branch', worktreePath: '/new' }];
  let granted = true;
  const native: Native = { available: true, watch() {}, async later(input) {
    const request = obj(input);
    return { ok: true, generation: 9, value: request.op === 'http' ? { authenticated: true, permissions: granted ? ['filesystem:read'] : [] }
      : request.method === 'projects.listEntries' ? { entries: [{ kind: 'file', path: client.threadId === 'one' ? 'old.ts' : 'new.ts' }] } : {} };
  } };
  const chat = { id: '7', name: 'thread', url: '/thread/env/one', params: { threadEnvironment: 'env', threadId: 'one' } };
  const overlay = { id: '8', name: 'settings', url: '/settings', params: {} };
  const context = (isOverlay = false, supported = true) => mobileInspectorContext(isOverlay ? [chat, overlay] : [chat], true,
    supported, supported ? 320 : 0, false, false, false, client).serialized;
  const step = (previous: { serialized: string; content: string }, ctx: string, now: number, kind = '', value = '') =>
    mobileInspectorTransition(previous.serialized, ctx, kind, value, now, previous.content, false, client);
  const open = (kind = 'files') => {
    const first = step({ serialized: '', content: '' }, context(), 0);
    return step(first, context(), 1, kind, workspaceInspectorSnapshot(first.serialized).focusOwner);
  };
  const presentation = (value: { content: string; serialized: string }) => mobileInspectorPresentation(value.content, value.serialized, true, '', 'light', 'default', client);
  return { client, native, context, step, open, presentation, deny() { granted = false; } };
}

test('missing captured Git content cannot display the currently selected thread', () => {
  const f = fixture(), opened = f.open('git');
  f.client.threadId = 'two';
  const absent = f.presentation({ ...opened, content: '' });
  expect(absent.showGit).toBe(false);
  expect(absent.git.owner).toBe('');
  expect(absent.git.branchLabel).not.toBe('new-branch');
  const wrongOwner = f.presentation({ ...opened, content: JSON.stringify({ ...JSON.parse(opened.content), owner: 'other' }) });
  expect(wrongOwner.showGit).toBe(false);
  expect(wrongOwner.git.owner).toBe('');
});

test('outgoing capture survives a new selected workspace and temporary unsupported column', async () => {
  const f = fixture(); await mobileFilesRead('', '', f.native, f.client);
  const opened = f.open(); expect(f.presentation(opened).files.rows.map(row => row.path)).toEqual(['old.ts']);
  f.client.threadId = 'two'; await mobileFilesRead('', '', f.native, f.client);
  const unsupported = f.step(opened, f.context(true, false), 100);
  expect(workspaceInspectorSnapshot(unsupported.serialized)).toMatchObject({ mounted: false, contentThreadId: 'one' });
  expect(unsupported.content).toBe(opened.content);
  const returned = f.step(unsupported, f.context(true), 340);
  expect(workspaceInspectorSnapshot(returned.serialized)).toMatchObject({ mounted: true, active: false, exitAt: 600 });
  expect(f.presentation(returned).files.rows.map(row => row.path)).toEqual(['old.ts']);
  expect(JSON.parse(f.presentation(returned).configuration).visible).toBe(false);
  const exit = workspaceInspectorSnapshot(returned.serialized);
  const closed = f.step(returned, f.context(true), 600, 'deadline', exit.exitToken);
  expect(closed.content).toBe(''); expect(f.presentation(closed).showFiles).toBe(false);
});

test('same-context permission revocation refreshes captured rows without changing focus owner', async () => {
  const f = fixture(); await mobileFilesRead('', '', f.native, f.client);
  const opened = f.open(), originalContext = f.context();
  expect(f.presentation(opened).files.rows.length).toBe(1);
  f.deny(); await mobileFilesRead('', '', f.native, f.client);
  expect(f.context()).toBe(originalContext);
  const refreshed = f.step(opened, originalContext, 100);
  expect(workspaceInspectorSnapshot(refreshed.serialized).contentOwner).toBe(workspaceInspectorSnapshot(opened.serialized).contentOwner);
  expect(f.presentation(refreshed).files.rows).toEqual([]);
  expect(f.presentation(refreshed).files.error).toContain('cannot read host files');
});

test('generation change cannot reuse a mismatched captured owner', () => {
  const f = fixture(), opened = f.open('git');
  f.client.generation = 10;
  const changed = f.step(opened, f.context(), 10);
  expect(workspaceInspectorSnapshot(changed.serialized).contentGeneration).toBe(10);
  const oldIntoNew = f.presentation({ ...changed, content: opened.content });
  expect(oldIntoNew.showGit).toBe(false);
  expect(oldIntoNew.git.owner).toBe('');
});

// The Diff panel's state across right-panel tab switches, reopening and threads (T3 Code 74541bab17: DiffPanel.tsx,
// diffPanelStore.ts useDiffPanelSessionStore, ChatView.tsx resolveExplicitDiffOpenOnActivate, diffFileTree.logic.ts).
import { describe, test, expect } from 'bun:test';
import { T3Client } from '../../src/client/client';
import { snapshot } from '../../src/client/presentation';
import { obj, type Obj } from '../../src/client/domain';
import type { Native, Files } from '../../src/client/protocol';
import { panelState, surfaceLocal } from '../../src/panel/panel-surfaces';
import { diffFileTreeEntries, diffTreeRows } from '../../src/diff/diff-tree';
import { gitCardView, gitLocal } from '../../src/details/git-actions';

const branchPatch = 'diff --git a/a.ts b/a.ts\n--- a/a.ts\n+++ b/a.ts\n@@ -1,1 +1,1 @@\n-one\n+two\ndiff --git a/b.ts b/b.ts\n--- a/b.ts\n+++ b/b.ts\n@@ -1,1 +1,1 @@\n-uno\n+dos\n';
const turnPatch = 'diff --git a/a.ts b/a.ts\n--- a/a.ts\n+++ b/a.ts\n@@ -1,1 +1,1 @@\n-one\n+two\ndiff --git a/src/c.ts b/src/c.ts\n--- a/src/c.ts\n+++ b/src/c.ts\n@@ -1,1 +1,1 @@\n-x\n+y\n';

function harness() {
  const calls: Obj[] = [];
  let branch = branchPatch, release: (() => void) | null = null, held = false;
  const native: Native = { available: true, watch() {}, async later(input) {
    const request = obj(input); calls.push(request);
    if (request.op === 'request' && request.method === 'review.getDiffPreview') {
      if (held) await new Promise<void>(resolve => { release = resolve; });
      return { ok: true, generation: 1, value: { cwd: '/repo', sources: [{ kind: 'working-tree', diff: '', truncated: false }, { kind: 'branch-range', diff: branch, truncated: false }] } };
    }
    if (request.op === 'request' && request.method === 'orchestration.getTurnDiff') return { ok: true, generation: 1, value: { diff: turnPatch } };
    return { ok: true, generation: 1, value: { entries: [], truncated: false } };
  } };
  const storage: Files = { fs: { async mkdir() {}, async readFile() { throw new Error('missing'); }, async atomicWriteFile() {} } };
  const client = new T3Client();
  Object.assign(client, { available: true, generation: 1, connection: 'connected', environmentId: 'env', projectId: 'p1', threadId: 't1',
    configLive: true, shellLive: true, threadLive: true, scopes: ['orchestration:read', 'orchestration:operate', 'filesystem:read'], config: { environment: { capabilities: {} } } });
  client.shell.projects = [{ id: 'p1', title: 'Fixture', workspaceRoot: '/repo' }];
  client.thread = { projection: { thread: { id: 't1' }, runtimeRequests: [], turnItems: [], runs: [{ id: 'r1', completedAt: '2026-10-03T14:15:00Z' }],
    checkpoints: [{ id: 'c1', status: 'ready', runId: 'r1', appRunOrdinal: 1 }] }, sequence: 0, historyCursor: null, hasMore: false, latestLocalTurnOrdinal: null };
  return {
    client, calls,
    command: (op: string, id = '', value = '', n = 0) => client.command(op, id, value, n, native, storage),
    panel: (op: string, id = '') => surfaceLocal(client, native, op, id, ''),
    serve: (patch: string) => { branch = patch; },
    hold: () => { held = true; },
    finish: () => { held = false; release?.(); release = null; },
  };
}
const expanded = (client: T3Client) => snapshot(client).diffItems.filter((item: Obj) => item.kind === 'file' && item.expanded).map((item: Obj) => item.path);
const previews = (calls: Obj[]) => calls.filter(call => call.op === 'request' && call.method === 'review.getDiffPreview').length;
/** Lets a command run up to its held request. */
async function untilAsked(calls: Obj[], count: number): Promise<void> {
  for (let turn = 0; turn < 50 && previews(calls) < count; turn++) await Promise.resolve();
  expect(previews(calls)).toBe(count);
}

describe('the Diff keeps its review across right-panel tabs', () => {
  test('opened files survive a switch to Files and back, and stay with their thread', async () => {
    const { client, command, panel } = harness();
    await panel('open', 'diff');
    await command('diff-view', 'file', 'a.ts');
    expect(expanded(client)).toEqual(['a.ts']);
    await panel('open', 'files');
    expect(client.diffOpen).toBe(false);
    await panel('activate', 'diff');
    expect(expanded(client)).toEqual(['a.ts']);
    // Another thread's Changes lists the same path folded: the folds belong to collapseScopeKey (thread and section).
    client.threadId = 't2';
    client.diffOpen = false;
    await command('diff');
    expect(expanded(client)).toEqual([]);
    client.threadId = 't1';
    await command('diff');
    expect(expanded(client)).toEqual(['a.ts']);
  });

  test('a press on the Diff tab resumes its scope; ⌘D, the launcher or "+" starts on Changes', async () => {
    const { client, command, panel } = harness();
    await panel('open', 'diff');
    await command('diff-scope', '', 'unstaged');
    expect(snapshot(client).diffScopeLabel).toBe('Uncommitted');
    await panel('open', 'files');
    await panel('activate', 'diff');
    expect(snapshot(client)).toMatchObject({ diffOpen: true, diffScopeLabel: 'Uncommitted' });
    await panel('open', 'files');
    await panel('open', 'diff');
    expect(snapshot(client)).toMatchObject({ diffOpen: true, diffScopeLabel: 'Changes' });
    // A turn chosen in the panel is kept through a tab switch too.
    await command('diff-scope', '', 'turn:r1');
    await panel('open', 'files');
    await panel('activate', 'diff');
    expect(snapshot(client).diffScopeLabel).toBe('Latest turn');
    expect(panelState(client).active).toBe('diff');
  });

  test('reopening Changes asks again and keeps the answer on show while it loads', async () => {
    const { client, calls, panel, serve, hold, finish } = harness();
    await panel('open', 'diff');
    expect(previews(calls)).toBe(1);
    await panel('open', 'files');
    serve('diff --git a/a.ts b/a.ts\n--- a/a.ts\n+++ b/a.ts\n@@ -1,1 +1,1 @@\n-one\n+three\n');
    hold();
    const reopening = panel('activate', 'diff');
    await untilAsked(calls, 2);
    expect(snapshot(client)).toMatchObject({ diffLoading: true, diffPending: false });
    expect(snapshot(client).diffFiles.map((file: Obj) => file.path)).toEqual(['a.ts', 'b.ts']);
    finish();
    await reopening;
    expect(snapshot(client).diffFiles.map((file: Obj) => file.path)).toEqual(['a.ts']);
  });

  test('an empty diff refreshes without the loading skeleton; a new selection shows it', async () => {
    const { client, calls, command, serve, hold, finish } = harness();
    serve('');
    await command('diff');
    expect(snapshot(client)).toMatchObject({ diffEmpty: true, diffPending: false, diffEmptyLabel: 'No net changes in this selection.' });
    hold();
    const refreshing = command('diff-refresh');
    await untilAsked(calls, 2);
    expect(snapshot(client)).toMatchObject({ diffLoading: true, diffPending: false, diffEmpty: true });
    finish();
    await refreshing;
    // Uncommitted has no answer yet: loading shows the skeleton until it arrives.
    hold();
    const scoping = command('diff-scope', '', 'unstaged');
    await untilAsked(calls, 3);
    expect(snapshot(client)).toMatchObject({ diffLoading: true, diffPending: true, diffEmpty: false });
    finish();
    await scoping;
    expect(snapshot(client)).toMatchObject({ diffPending: false, diffEmpty: true });
  });
});

describe('returning to a thread', () => {
  test('a thread whose Diff was showing shows it again, on Changes; a thread without one stays closed', async () => {
    const { client, command, panel } = harness();
    client.shell.threads = [{ id: 't1', projectId: 'p1', title: 'One' }, { id: 't2', projectId: 'p1', title: 'Two' }] as Obj[];
    const native: Native = { available: true, watch() {}, async later(input) {
      const request = obj(input);
      if (request.method === 'review.getDiffPreview') return { ok: true, generation: 1, value: { cwd: '/repo', sources: [{ kind: 'branch-range', diff: branchPatch, truncated: false }] } };
      return { ok: true, generation: 1, value: {} };
    } };
    // The thread's detail and stream are not under test: each thread shows the same projection.
    const projection = client.thread;
    client.openThread = async () => { client.thread = projection; };
    await panel('open', 'diff');
    await command('diff-scope', '', 'unstaged');
    await client.openSelected(native, 't2');
    expect(client.diffOpen).toBe(false);
    expect(panelState(client).visible).toBe(false);
    await client.openSelected(native, 't1');
    expect(panelState(client)).toMatchObject({ active: 'diff', visible: true });
    expect(snapshot(client)).toMatchObject({ diffOpen: true, diffScopeLabel: 'Changes', diffPending: false });
    expect(snapshot(client).diffFiles.map((file: Obj) => file.path)).toEqual(['a.ts', 'b.ts']);
  });

  test('a reply for a scope chosen away from meanwhile does not replace the newer one', async () => {
    const { client, command, panel, hold, finish } = harness();
    await panel('open', 'diff');
    await panel('open', 'files');
    hold();
    const reopening = panel('activate', 'diff');
    await Promise.resolve(); await Promise.resolve(); await Promise.resolve();
    await command('diff-scope', '', 'turn:r1');
    finish();
    await reopening;
    expect(snapshot(client)).toMatchObject({ diffScopeLabel: 'Latest turn' });
    expect(snapshot(client).diffFiles.map((file: Obj) => file.path)).toEqual(['a.ts', 'src/c.ts']);
  });
});

describe('a turn diff opened on a file reveals it', () => {
  test('the file opens, the tree selects it with its folders open, and each open asks the list to scroll again', async () => {
    const { client, command, panel } = harness();
    await command('diff-view', 'tree');
    await command('turn-diff', 'src/c.ts', 'r1');
    let view = snapshot(client);
    expect(expanded(client)).toEqual(['src/c.ts']);
    expect(view).toMatchObject({ diffReveal: 1, diffRevealTarget: 'file:src/c.ts' });
    expect(view.diffTreeRows.find((row: Obj) => row.path === 'src/c.ts')).toMatchObject({ selected: true });
    // Folded again by the reader, then opened from the timeline again: it unfolds and scrolls once more.
    await command('diff-view', 'file', 'src/c.ts');
    expect(expanded(client)).toEqual([]);
    expect(snapshot(client).diffReveal).toBe(1);
    await command('turn-diff', 'src/c.ts', 'r1');
    view = snapshot(client);
    expect(expanded(client)).toEqual(['src/c.ts']);
    expect(view.diffReveal).toBe(2);
    // Back from another tab the Diff reveals its named file again, as the reference's remounted panel does.
    await command('diff-view', 'file', 'src/c.ts');
    await panel('open', 'files');
    await panel('activate', 'diff');
    expect(expanded(client)).toEqual(['src/c.ts']);
    expect(snapshot(client).diffReveal).toBe(3);
    // The checkpoint's Open diff names no file: nothing is revealed.
    await command('checkpoint-diff', '', '', 1);
    expect(snapshot(client)).toMatchObject({ diffReveal: 3, diffRevealTarget: '' });
  });
});

describe('colliding paths in the file tree', () => {
  test('a file and a folder at the same path are both listed and the file stays selectable', () => {
    const entries = diffFileTreeEntries([{ path: 'docs/guide', status: 'deleted' }, { path: 'docs/guide/index.md', status: 'added' }]);
    const rows = diffTreeRows(entries, new Set(), 'docs/guide');
    expect(rows.map(row => [row.kind, row.path, row.name, row.selected])).toEqual([
      ['folder', 'docs/', 'docs', false], ['file', 'docs/guide', 'guide', true], ['folder', 'docs/guide/', 'guide', false], ['file', 'docs/guide/index.md', 'index.md', false]]);
    expect(new Set(rows.map(row => row.id)).size).toBe(rows.length);
  });
});

describe('changed files open against the worktree root', () => {
  const status = (rootPath?: string) => ({ isRepo: true, hasPrimaryRemote: false, isDefaultRef: false, refName: 'feature',
    workingTree: { ...(rootPath ? { rootPath } : {}), files: [{ path: 'src/ü.ts', insertions: 1, deletions: 0 }], insertions: 1, deletions: 0 } });
  test('the path resolves against workingTree.rootPath, not the project cwd; without it the row is disabled', async () => {
    const calls: Obj[] = [];
    const native: Native = { available: true, watch() {}, async later(input) { calls.push(obj(input)); return { ok: true, generation: 1, value: {} }; } };
    const client = new T3Client();
    Object.assign(client, { generation: 1, connection: 'connected', configLive: true, shellLive: true, threadLive: true, environmentId: 'env', scopes: ['orchestration:read', 'orchestration:operate'] });
    expect(gitCardView(client, status('/repo/.t3/worktrees/feature'), '', '/repo', 0, 'vscode').filesOpenable).toBe(true);
    await gitLocal(client, native, 'open-file', 'src/ü.ts', '');
    expect(calls.find(call => call.method === 'shell.openInEditor')?.payload).toEqual({ cwd: '/repo/.t3/worktrees/feature/src/ü.ts', editor: 'vscode' });
    expect(gitCardView(client, status(), '', '/repo', 0, 'vscode').filesOpenable).toBe(false);
    client.scopes = ['orchestration:read'];
    expect(gitCardView(client, status('/repo'), '', '/repo', 0, 'vscode').filesOpenable).toBe(false);
  });
});
